use crate::{
    document::{Document, local_link},
    session,
};
use rune::{FileDialogOptions, Image, ImageCache, Key, KeyEvent, MenuItem, TextEdit, Tx};
use rune_kit::markdown::{self, Case};
use std::{
    cell::RefCell,
    collections::HashMap,
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Preview,
    Edit,
    Split,
}
#[derive(Clone, Copy)]
pub enum Confirm {
    Close(u64),
    Revert(u64),
}

pub struct State {
    pub docs: Vec<Document>,
    pub active: usize,
    pub next_id: u64,
    pub mode: Mode,
    pub search: TextEdit,
    pub finding: bool,
    pub matches: Vec<Vec<Range<usize>>>,
    pub match_index: usize,
    pub error: Option<String>,
    pub confirm: Option<Confirm>,
    pub zoom: f32,
    pub appearance: Option<bool>,
    pub reduce_transparency: bool,
    pub cache: Option<Arc<ImageCache>>,
    pub images: Rc<RefCell<HashMap<PathBuf, Image>>>,
    image_stamp: usize,
    last_disk_check: f64,
    pub headless: bool,
    pub wake: Option<rune::Waker>,
    pub recent: crate::recent::History,
    pub palette: crate::palette::Palette,
    pub tab_switch: crate::tab_switch::Switcher,
    pub tab_order: Vec<u64>,
}

impl State {
    pub fn new() -> Self {
        let headless = cfg!(test)
            || std::env::args()
                .any(|a| matches!(a.as_str(), "--snap" | "--tree" | "--script" | "--trace"));
        let restored = if headless {
            Ok(Vec::new())
        } else {
            session::restore()
        };
        let (docs, error) = match restored {
            Ok(d) => (d, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        let (recent, recent_error) = if headless {
            (crate::recent::History::default(), None)
        } else {
            match crate::recent::History::load(session::directory().join("recents.json")) {
                Ok(history) => (history, None),
                Err(e) => (crate::recent::History::default(), Some(e)),
            }
        };
        let error = error.or(recent_error);
        let tab_order = docs.iter().map(|d| d.id).collect();
        let next_id = docs.len() as u64 + 1;
        let mut s = Self {
            docs,
            active: 0,
            next_id,
            mode: Mode::Preview,
            search: TextEdit::new(""),
            finding: false,
            matches: Vec::new(),
            match_index: 0,
            error,
            confirm: None,
            zoom: 1.0,
            appearance: None,
            reduce_transparency: false,
            cache: None,
            images: Rc::new(RefCell::new(HashMap::new())),
            image_stamp: 0,
            last_disk_check: 0.0,
            headless,
            wake: None,
            recent,
            palette: Default::default(),
            tab_switch: Default::default(),
            tab_order,
        };
        for path in rune::extra_args()
            .iter()
            .filter(|a| !a.starts_with("-psn_"))
        {
            s.open(Path::new(path));
        }
        if s.docs.is_empty() {
            s.blank();
            s.mode = Mode::Preview;
        }
        s
    }
    pub fn doc(&self) -> &Document {
        &self.docs[self.active]
    }
    pub fn doc_mut(&mut self) -> &mut Document {
        &mut self.docs[self.active]
    }
    pub fn blank(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        self.docs.push(Document::blank(id));
        self.active = self.docs.len() - 1;
        self.touch_tab();
        self.mode = Mode::Edit;
        self.refresh_search();
    }
    pub fn open(&mut self, path: &Path) {
        if let Ok(canonical) = path.canonicalize()
            && let Some(i) = self
                .docs
                .iter()
                .position(|d| d.path.as_ref() == Some(&canonical))
        {
            self.active = i;
            self.touch_tab();
            self.error = None;
            self.remember_current();
            self.refresh_search();
            return;
        }
        match Document::open(self.next_id, path) {
            Ok(doc) => {
                self.next_id += 1;
                if self.docs.len() == 1 && self.doc().path.is_none() && !self.doc().dirty() {
                    self.docs.clear();
                }
                self.docs.push(doc);
                self.active = self.docs.len() - 1;
                self.mode = Mode::Preview;
                self.touch_tab();
                self.error = None;
                self.remember_current();
                self.images.borrow_mut().clear();
                self.refresh_search();
            }
            Err(e) => self.error = Some(e),
        }
    }
    pub fn remember(&mut self, path: PathBuf) {
        if let Err(error) = self.recent.record(&path) {
            self.error = Some(error);
        }
    }
    pub fn remember_current(&mut self) {
        if let Some(path) = self.doc().path.clone() {
            self.remember(path);
        }
    }
    fn touch_tab(&mut self) {
        let id = self.doc().id;
        self.tab_order
            .retain(|d| *d != id && self.docs.iter().any(|doc| doc.id == *d));
        self.tab_order.insert(0, id);
    }
    pub fn persist(&mut self) {
        if !self.headless
            && let Err(e) = session::save(&self.docs)
        {
            self.error = Some(e);
        }
    }
    pub fn changed(&mut self, tx: &mut Tx) {
        self.refresh_search();
        self.persist();
        self.title(tx);
        tx.instant();
    }
    pub fn title(&self, tx: &mut Tx) {
        tx.set_title(format!(
            "{}{} — Folio",
            self.doc().title(),
            if self.doc().dirty() { " •" } else { "" }
        ));
    }
    pub fn refresh_search(&mut self) {
        self.matches = if self.search.text.is_empty() {
            Vec::new()
        } else {
            markdown::find(&self.doc().edit.text, &self.search.text, Case::Insensitive)
        };
        self.match_index = self.match_index.min(self.matches.len().saturating_sub(1));
    }
    pub fn find_step(&mut self, backwards: bool, tx: &mut Tx) {
        let n = self.matches.len();
        if n == 0 {
            return;
        }
        self.match_index = if backwards {
            (self.match_index + n - 1) % n
        } else {
            (self.match_index + 1) % n
        };
        self.show_match(tx);
    }
    pub fn show_match(&mut self, tx: &mut Tx) {
        if let Some(ranges) = self.matches.get(self.match_index) {
            if self.mode != Mode::Preview
                && let (Some(first), Some(last)) = (ranges.first(), ranges.last())
            {
                let (start, end) = (first.start, last.end);
                self.doc_mut().edit.set_selection(start, end);
            }
            tx.scroll_into_view(rune_kit::Markdown::highlight_key(self.match_index));
        }
        tx.instant();
    }
    pub fn select(&mut self, index: usize, tx: &mut Tx) {
        self.active = index;
        self.touch_tab();
        self.refresh_search();
        self.title(tx);
        tx.instant();
    }
    pub fn close(&mut self, id: u64, tx: &mut Tx) {
        if let Some(i) = self.docs.iter().position(|d| d.id == id) {
            self.docs.remove(i);
            self.active = if i < self.active {
                self.active - 1
            } else {
                self.active.min(self.docs.len().saturating_sub(1))
            };
            if self.docs.is_empty() {
                self.blank();
            }
            self.touch_tab();
            self.confirm = None;
            self.persist();
            self.refresh_search();
            self.title(tx);
            tx.instant();
        }
    }
    pub fn link(&mut self, link: &str, tx: &mut Tx) {
        if let Some(fragment) = link.strip_prefix('#') {
            // Headings are matched against their GitHub-style slug, then scrolled by block key.
            for (block, b) in rune_kit::markdown::parse(&self.doc().edit.text)
                .into_iter()
                .enumerate()
            {
                if let rune_kit::markdown::Block::Heading(_, inline) = b {
                    let slug: String = inline
                        .text
                        .to_lowercase()
                        .chars()
                        .filter_map(|c| {
                            if c.is_alphanumeric() || c == '_' || c == '-' {
                                Some(c)
                            } else if c.is_whitespace() {
                                Some('-')
                            } else {
                                None
                            }
                        })
                        .collect();
                    if slug == fragment {
                        tx.scroll_into_view(format!("markdown/b{block}"));
                        return;
                    }
                }
            }
            return;
        }
        if link.starts_with("https:") || link.starts_with("http:") || link.starts_with("mailto:") {
            if let Err(e) = tx.open_url(link) {
                self.error = Some(e.to_string());
            }
        } else if let Some(path) = local_link(self.doc().path.as_deref(), link) {
            if is_markdown(&path) {
                self.open(&path);
                self.title(tx);
            } else {
                self.error = Some("Only Markdown file links open inside Folio.".into());
            }
        } else {
            self.error = Some("This link can’t be opened.".into());
        }
    }
}

pub fn is_markdown(p: &Path) -> bool {
    p.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        ["md", "markdown", "mdown", "mkd", "mkdn"]
            .iter()
            .any(|e| s.eq_ignore_ascii_case(e))
    })
}

pub fn setup(s: &mut State, wake: rune::Waker) {
    s.wake = Some(wake.clone());
    s.cache = Some(Arc::new(ImageCache::new(wake)));
    #[cfg(target_os = "macos")]
    if !s.headless {
        s.reduce_transparency = crate::macos::reduce_transparency();
    }
}
pub fn poll(s: &mut State, clock: f64, tx: &mut Tx) -> bool {
    let outline_changed = crate::outline::poll(s, tx);
    if s.mode == Mode::Split {
        s.doc().split_sync.apply(tx);
    }
    let stamp = s.images.borrow().values().filter(|i| i.is_ready()).count();
    let mut changed = stamp != s.image_stamp || outline_changed;
    s.image_stamp = stamp;
    if !s.headless && clock - s.last_disk_check >= 1.0 {
        s.last_disk_check = clock;
        #[cfg(target_os = "macos")]
        {
            let reduce = crate::macos::reduce_transparency();
            changed |= reduce != s.reduce_transparency;
            s.reduce_transparency = reduce;
        }
        let mut disk_changed = false;
        for doc in &mut s.docs {
            match doc.reload_if_changed() {
                Ok(reloaded) => disk_changed |= reloaded,
                Err(error) if s.error.as_ref() != Some(&error) => {
                    s.error = Some(error);
                    disk_changed = true;
                }
                Err(_) => {}
            }
        }
        if disk_changed {
            s.refresh_search();
            s.title(tx);
            tx.instant();
        }
        changed |= disk_changed;
    }
    if !s.headless {
        tx.wake_after(std::time::Duration::from_secs_f64(
            (1.0 - (clock - s.last_disk_check)).clamp(0.01, 1.0),
        ));
    }
    changed
}
pub fn new_document(s: &mut State, tx: &mut Tx) {
    // The welcome screen already owns a blank document; New should begin editing it,
    // rather than exposing the placeholder as a second Untitled tab.
    if s.doc().path.is_none() && s.doc().edit.text.is_empty() {
        s.mode = Mode::Edit;
    } else {
        s.blank();
    }
    s.title(tx);
    tx.focus("editor", false);
    tx.instant();
}
pub fn open_dialog(s: &mut State, tx: &mut Tx) {
    let options = FileDialogOptions {
        title: "Open Markdown".into(),
        directory: s
            .doc()
            .path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf)),
        ..Default::default()
    }
    .filter("Markdown", &["md", "markdown", "mdown", "mkd", "mkdn"]);
    tx.open_file(options, |s: &mut State, result, tx| match result {
        Ok(Some(path)) => {
            s.open(&path);
            s.title(tx);
            tx.instant();
        }
        Ok(None) => {}
        Err(e) => s.error = Some(e.to_string()),
    });
}
pub fn save(s: &mut State, tx: &mut Tx) {
    save_document(s, tx, false, false);
}
pub fn save_as(s: &mut State, tx: &mut Tx) {
    save_document(s, tx, true, false);
}
pub fn save_document(s: &mut State, tx: &mut Tx, force_as: bool, close_after: bool) {
    let id = s.doc().id;
    if !force_as && let Some(path) = s.doc().path.clone() {
        match s.doc_mut().save(&path) {
            Ok(()) => {
                s.error = None;
                s.remember_current();
                if close_after {
                    s.close(id, tx);
                } else {
                    s.persist();
                    s.title(tx);
                }
            }
            Err(e) => {
                s.confirm = None;
                s.error = Some(e);
            }
        }
        return;
    }
    let options = FileDialogOptions {
        title: "Save Markdown".into(),
        file_name: Some(s.doc().title()),
        directory: s
            .doc()
            .path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf)),
        ..Default::default()
    }
    .filter("Markdown", &["md", "markdown"]);
    tx.save_file(options, move |s: &mut State, result, tx| match result {
        Ok(Some(mut path)) => {
            if path.extension().is_none() {
                path.set_extension("md");
            }
            if let Some(index) = s.docs.iter().position(|d| d.id == id) {
                let target = path.canonicalize().unwrap_or_else(|_| path.clone());
                if s.docs
                    .iter()
                    .any(|d| d.id != id && d.path.as_ref() == Some(&target))
                {
                    s.error = Some("That file is already open in another tab.".into());
                    s.confirm = None;
                    return;
                }
                match s.docs[index].save(&path) {
                    Ok(()) => {
                        s.error = None;
                        if let Some(path) = s.docs[index].path.clone() {
                            s.remember(path);
                        }
                        if close_after {
                            s.close(id, tx);
                        } else {
                            s.confirm = None;
                            s.persist();
                            s.title(tx);
                        }
                    }
                    Err(e) => {
                        s.confirm = None;
                        s.error = Some(e);
                    }
                }
            }
        }
        Ok(None) => s.confirm = None,
        Err(e) => {
            s.confirm = None;
            s.error = Some(e.to_string());
        }
    });
}
pub fn request_close(s: &mut State, tx: &mut Tx) {
    if s.doc().dirty() {
        s.confirm = Some(Confirm::Close(s.doc().id));
    } else {
        s.close(s.doc().id, tx);
    }
}
pub fn request_revert(s: &mut State, _: &mut Tx) {
    if s.doc().path.is_some() {
        s.confirm = Some(Confirm::Revert(s.doc().id));
    }
}
pub fn revert(s: &mut State, tx: &mut Tx) {
    if let Some(path) = s.doc().path.clone() {
        match Document::open(s.doc().id, &path) {
            Ok(d) => {
                let index = s.active;
                s.docs[index] = d;
                s.persist();
                s.refresh_search();
                s.title(tx);
            }
            Err(e) => s.error = Some(e),
        }
    }
    s.confirm = None;
    tx.instant();
}
pub fn find(s: &mut State, tx: &mut Tx) {
    s.finding = true;
    tx.focus("find-field/input", false);
    tx.instant();
}
pub fn preview(s: &mut State, tx: &mut Tx) {
    s.mode = Mode::Preview;
    tx.instant();
}
pub fn edit(s: &mut State, tx: &mut Tx) {
    s.mode = Mode::Edit;
    tx.focus("editor", false);
    tx.instant();
}
pub fn split(s: &mut State, tx: &mut Tx) {
    s.mode = Mode::Split;
    tx.instant();
}
pub fn zoom_in(s: &mut State, tx: &mut Tx) {
    s.zoom = (s.zoom + 0.1).min(1.8);
    tx.instant();
}
pub fn zoom_out(s: &mut State, tx: &mut Tx) {
    s.zoom = (s.zoom - 0.1).max(0.7);
    tx.instant();
}
pub fn zoom_reset(s: &mut State, tx: &mut Tx) {
    s.zoom = 1.0;
    tx.instant();
}
pub fn on_key(s: &mut State, e: &KeyEvent, tx: &mut Tx) -> bool {
    if crate::tab_switch::key(s, e, tx) || crate::palette::shortcut(s, e, tx) {
        return true;
    }
    if e.key == Key::Escape && s.finding {
        s.finding = false;
        s.search.replace_text("");
        s.refresh_search();
        tx.instant();
        return true;
    }
    if e.is_command('g') {
        s.find_step(e.mods.shift, tx);
        return true;
    }
    if e.is_command('w') {
        request_close(s, tx);
        return true;
    }
    false
}
pub fn menu_items() -> Vec<MenuItem<State>> {
    crate::commands::menu_items()
}
