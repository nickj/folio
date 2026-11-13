//! One catalog supplies the command palette and custom native menus, keeping both entry
//! points on the same save/close/confirmation and editing paths.
use crate::{
    app::{self, Mode, State},
    palette, tab_switch,
};
use rune::{Key, KeyEvent, MenuItem, Mods, Shortcut, Tx};

type Action = fn(&mut State, &mut Tx);
#[derive(Clone, Copy)]
pub enum Requires {
    Any,
    SavedFile,
    Editing,
    Selection,
    MultipleTabs,
    Recent,
    Matches,
    Headings,
}

pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub shortcut: Option<&'static str>,
    pub keywords: &'static str,
    pub action: Action,
    menu: bool,
    requires: Requires,
}
impl Command {
    const fn new(
        id: &'static str,
        label: &'static str,
        category: &'static str,
        shortcut: Option<&'static str>,
        action: Action,
    ) -> Self {
        Self {
            id,
            label,
            category,
            shortcut,
            keywords: "",
            action,
            menu: true,
            requires: Requires::Any,
        }
    }
    const fn only_palette(mut self) -> Self {
        self.menu = false;
        self
    }
    const fn requires(mut self, requires: Requires) -> Self {
        self.requires = requires;
        self
    }
    const fn keywords(mut self, keywords: &'static str) -> Self {
        self.keywords = keywords;
        self
    }
    pub fn enabled(&self, s: &State) -> bool {
        match self.requires {
            Requires::Any => true,
            Requires::SavedFile => s.doc().path.is_some(),
            Requires::Editing => s.mode != Mode::Preview,
            Requires::Selection => s.mode != Mode::Preview && !s.doc().edit.selected().is_empty(),
            Requires::MultipleTabs => s.docs.len() > 1,
            Requires::Recent => !s.recent.paths().is_empty(),
            Requires::Matches => !s.matches.is_empty(),
            Requires::Headings => !s
                .doc()
                .outline
                .snapshot(&s.doc().edit.text)
                .headings
                .is_empty(),
        }
    }
    pub fn key_hint(&self) -> String {
        self.shortcut
            .and_then(|s| s.parse::<Shortcut>().ok())
            .map(|s| s.menu_keycaps().join(""))
            .unwrap_or_default()
    }
}

pub static ALL: &[Command] = &[
    Command::new("new", "New", "File", Some("cmd+n"), app::new_document)
        .keywords("create untitled document"),
    Command::new("open", "Open…", "File", Some("cmd+o"), app::open_dialog)
        .keywords("file document"),
    Command::new(
        "recent",
        "Open Recent Document…",
        "File",
        Some("cmd+shift+o"),
        palette::open_recent,
    )
    .keywords("history quick open fuzzy"),
    Command::new("save", "Save", "File", Some("cmd+s"), app::save),
    Command::new(
        "save-as",
        "Save As…",
        "File",
        Some("cmd+shift+s"),
        app::save_as,
    )
    .keywords("rename copy file"),
    Command::new(
        "revert",
        "Revert to Saved…",
        "File",
        None,
        app::request_revert,
    )
    .requires(Requires::SavedFile)
    .keywords("reload discard edits"),
    Command::new(
        "close",
        "Close Document",
        "File",
        Some("cmd+w"),
        app::request_close,
    )
    .keywords("tab"),
    Command::new("copy-markdown", "Copy Markdown", "File", None, |s, tx| {
        tx.copy(&s.doc().edit.text)
    }),
    Command::new("copy-path", "Copy File Path", "File", None, copy_path)
        .requires(Requires::SavedFile),
    Command::new("reveal", "Reveal in Finder", "File", None, reveal).requires(Requires::SavedFile),
    Command::new(
        "clear-recents",
        "Clear Recent Documents",
        "File",
        None,
        clear_recents,
    )
    .requires(Requires::Recent)
    .keywords("forget history"),
    Command::new("undo", "Undo", "Edit", Some("cmd+z"), |s, tx| {
        edit_key(s, tx, 'z', false)
    })
    .only_palette()
    .requires(Requires::Editing),
    Command::new("redo", "Redo", "Edit", Some("cmd+shift+z"), |s, tx| {
        edit_key(s, tx, 'z', true)
    })
    .only_palette()
    .requires(Requires::Editing),
    Command::new("cut", "Cut", "Edit", Some("cmd+x"), |s, tx| {
        edit_key(s, tx, 'x', false)
    })
    .only_palette()
    .requires(Requires::Selection),
    Command::new("copy", "Copy", "Edit", Some("cmd+c"), |s, tx| {
        edit_key(s, tx, 'c', false)
    })
    .only_palette()
    .requires(Requires::Selection),
    Command::new("paste", "Paste", "Edit", Some("cmd+v"), paste_source)
        .only_palette()
        .requires(Requires::Editing),
    Command::new(
        "select-all",
        "Select All",
        "Edit",
        Some("cmd+a"),
        |s, tx| edit_key(s, tx, 'a', false),
    )
    .only_palette()
    .requires(Requires::Editing),
    Command::new("find", "Find…", "Edit", Some("cmd+f"), app::find).keywords("search document"),
    Command::new("find-next", "Find Next", "Edit", Some("cmd+g"), |s, tx| {
        s.find_step(false, tx)
    })
    .requires(Requires::Matches),
    Command::new(
        "find-previous",
        "Find Previous",
        "Edit",
        Some("cmd+shift+g"),
        |s, tx| s.find_step(true, tx),
    )
    .requires(Requires::Matches),
    Command::new(
        "commands",
        "Command Palette…",
        "View",
        Some("cmd+shift+p"),
        palette::open_commands,
    )
    .keywords("actions search"),
    Command::new("preview", "Preview", "View", Some("cmd+1"), app::preview)
        .keywords("read markdown"),
    Command::new("edit", "Edit Source", "View", Some("cmd+2"), app::edit)
        .keywords("write markdown"),
    Command::new("split", "Split View", "View", Some("cmd+3"), app::split)
        .keywords("side by side preview editor"),
    Command::new(
        "outline",
        "Focus Outline",
        "View",
        None,
        crate::outline::focus,
    )
    .requires(Requires::Headings)
    .keywords("headings sections table of contents navigation"),
    Command::new(
        "zoom-in",
        "Larger Text",
        "View",
        Some("cmd+="),
        app::zoom_in,
    )
    .keywords("zoom in increase font size"),
    Command::new(
        "zoom-out",
        "Smaller Text",
        "View",
        Some("cmd+-"),
        app::zoom_out,
    )
    .keywords("zoom out decrease font size"),
    Command::new(
        "zoom-reset",
        "Actual Size",
        "View",
        Some("cmd+0"),
        app::zoom_reset,
    )
    .keywords("reset zoom font"),
    Command::new("system", "System Appearance", "View", None, |s, _| {
        s.appearance = None
    })
    .keywords("theme automatic"),
    Command::new("light", "Light Appearance", "View", None, |s, _| {
        s.appearance = Some(false)
    })
    .keywords("theme"),
    Command::new("dark", "Dark Appearance", "View", None, |s, _| {
        s.appearance = Some(true)
    })
    .keywords("theme"),
    Command::new(
        "next-tab",
        "Next Document",
        "Window",
        Some("ctrl+tab"),
        tab_switch::next,
    )
    .requires(Requires::MultipleTabs)
    .keywords("switch tab"),
    Command::new(
        "previous-tab",
        "Previous Document",
        "Window",
        Some("ctrl+shift+tab"),
        tab_switch::previous,
    )
    .requires(Requires::MultipleTabs)
    .keywords("switch tab"),
    Command::new(
        "minimize",
        "Minimize Window",
        "Window",
        Some("cmd+m"),
        |_, tx| tx.minimize(),
    )
    .only_palette(),
    Command::new(
        "fullscreen",
        "Toggle Full Screen",
        "Window",
        Some("ctrl+cmd+f"),
        |_, tx| tx.toggle_full_screen(),
    ),
    Command::new("close-window", "Close Window", "Window", None, |_, tx| {
        tx.close_window()
    })
    .only_palette(),
    Command::new("quit", "Quit Folio", "App", Some("cmd+q"), |_, tx| {
        tx.quit()
    })
    .only_palette(),
];

pub fn menu_items() -> Vec<MenuItem<State>> {
    ALL.iter()
        .filter(|c| c.menu)
        .map(|c| {
            let mut item = MenuItem::new(c.label, c.action).id(c.id).menu(c.category);
            if let Some(key) = c.shortcut {
                item = item
                    .key_equivalent(key.parse().expect("valid command shortcut"))
                    .keys_first();
            }
            item
        })
        .collect()
}
fn copy_path(s: &mut State, tx: &mut Tx) {
    if let Some(path) = &s.doc().path {
        tx.copy(path.to_string_lossy());
    }
}
fn reveal(s: &mut State, _: &mut Tx) {
    #[cfg(target_os = "macos")]
    if !s.headless
        && let Some(path) = &s.doc().path
    {
        crate::macos::reveal_file(path);
    }
}
fn clear_recents(s: &mut State, _: &mut Tx) {
    if let Err(error) = s.recent.clear() {
        s.error = Some(error);
    }
}
fn paste_source(s: &mut State, tx: &mut Tx) {
    let id = s.doc().id;
    tx.focus("editor", false);
    // Clipboard reads are asynchronous. Target the document, not the palette field that
    // is still mounted until this frame ends, and never paste into a subsequently opened tab.
    tx.read_clipboard(move |s: &mut State, content, tx| {
        if content.text().is_empty() {
            return;
        }
        if let Some(i) = s.docs.iter().position(|d| d.id == id) {
            s.docs[i].edit.paste(content.text());
            s.persist();
            s.refresh_search();
            s.title(tx);
            if s.active == i && s.mode == Mode::Split {
                s.doc().split_sync.edited(s.doc().edit.focus);
                tx.wake_after(std::time::Duration::ZERO);
            }
            tx.instant();
        }
    });
}

fn edit_key(_: &mut State, tx: &mut Tx, c: char, shift: bool) {
    tx.focus("editor", false);
    tx.send_key(KeyEvent {
        key: Key::Command(c),
        mods: Mods {
            cmd: true,
            logo: true,
            shift,
            ..Default::default()
        },
    });
}
