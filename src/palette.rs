use crate::{
    app::{Mode, State},
    commands::{self, Command},
    fuzzy, tab_switch,
};
use rune::{Key, KeyEvent, TextEdit, Tx};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Commands,
    Recent,
}
#[derive(Default)]
pub enum Palette {
    #[default]
    Closed,
    Open(Box<OpenPalette>),
}
pub struct OpenPalette {
    pub kind: Kind,
    pub query: TextEdit,
    pub selected: Option<String>,
    pub(crate) previous_focus: Option<String>,
}
#[derive(Clone)]
pub enum Action {
    Command(&'static Command),
    Document(u64),
    Recent(PathBuf),
}
pub struct Result {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub shortcut: String,
    pub enabled: bool,
    pub action: Action,
}
impl Palette {
    pub fn open(&self) -> Option<&OpenPalette> {
        match self {
            Self::Open(p) => Some(p),
            Self::Closed => None,
        }
    }
    pub fn open_mut(&mut self) -> Option<&mut OpenPalette> {
        match self {
            Self::Open(p) => Some(p),
            Self::Closed => None,
        }
    }
}

pub fn open_commands(s: &mut State, tx: &mut Tx) {
    open(s, Kind::Commands, tx);
}
pub fn open_recent(s: &mut State, tx: &mut Tx) {
    open(s, Kind::Recent, tx);
}
fn open(s: &mut State, kind: Kind, tx: &mut Tx) {
    if s.confirm.is_some() || s.tab_switch.is_open() {
        return;
    }
    if s.palette.open().is_some_and(|p| p.kind == kind) {
        close(s, tx);
        return;
    }
    let previous_focus = match std::mem::take(&mut s.palette) {
        Palette::Open(p) => p.previous_focus,
        Palette::Closed => tx.focused().map(str::to_owned),
    };
    s.palette = Palette::Open(Box::new(OpenPalette {
        kind,
        query: TextEdit::new(""),
        selected: None,
        previous_focus,
    }));
    tx.focus("palette-query/input", false);
    tx.instant();
}
pub fn close(s: &mut State, tx: &mut Tx) {
    if let Palette::Open(p) = std::mem::take(&mut s.palette)
        && let Some(focus) = p.previous_focus
    {
        tx.focus(focus, false);
    }
    tx.instant();
}

pub fn results(s: &State) -> Vec<Result> {
    let Some(p) = s.palette.open() else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    match p.kind {
        Kind::Commands => {
            for command in commands::ALL {
                candidates.push((
                    Result {
                        id: format!("command:{}", command.id),
                        label: command.label.into(),
                        detail: command.category.into(),
                        shortcut: command.key_hint(),
                        enabled: command.enabled(s),
                        action: Action::Command(command),
                    },
                    format!("{} {}", command.category, command.keywords),
                ));
            }
            for doc in &s.docs {
                candidates.push((
                    Result {
                        id: format!("document:{}", doc.id),
                        label: format!("Switch to {}", doc.title()),
                        detail: doc
                            .path
                            .as_ref()
                            .map(|p| display_path(p))
                            .unwrap_or_else(|| "Unsaved document".into()),
                        shortcut: String::new(),
                        enabled: true,
                        action: Action::Document(doc.id),
                    },
                    "switch tab document".into(),
                ));
            }
        }
        Kind::Recent => {
            for path in s.recent.paths() {
                candidates.push((
                    Result {
                        id: format!("recent:{}", path.display()),
                        label: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        detail: display_path(path),
                        shortcut: String::new(),
                        enabled: true,
                        action: Action::Recent(path.clone()),
                    },
                    path.to_string_lossy().into_owned(),
                ));
            }
        }
    }
    let mut matches: Vec<_> = candidates
        .into_iter()
        .enumerate()
        .filter_map(|(i, (result, extra))| {
            fuzzy::rank(
                &p.query.text,
                &result.label,
                &format!("{} {extra}", result.detail),
            )
            .map(|score| (i, score, result))
        })
        .collect();
    matches.sort_by_key(|(i, score, _)| (std::cmp::Reverse(*score), *i));
    matches.into_iter().map(|(_, _, r)| r).collect()
}
pub fn active(s: &State, rows: &[Result]) -> Option<usize> {
    let chosen = s.palette.open()?.selected.as_deref();
    rows.iter()
        .position(|r| r.enabled && Some(r.id.as_str()) == chosen)
        .or_else(|| rows.iter().position(|r| r.enabled))
}
pub fn query_changed(s: &mut State, tx: &mut Tx) {
    if let Some(p) = s.palette.open_mut() {
        p.selected = None;
    }
    tx.scroll_to_y("palette-results", 0.0);
    tx.instant();
}
pub fn choose(s: &mut State, id: &str, tx: &mut Tx) {
    let Some(row) = results(s).into_iter().find(|r| r.id == id && r.enabled) else {
        return;
    };
    if let Action::Command(c) = &row.action {
        if c.id == "recent" {
            open(s, Kind::Recent, tx);
            return;
        }
        if c.id == "commands" {
            close(s, tx);
            return;
        }
    }
    close(s, tx);
    match row.action {
        Action::Command(command) => (command.action)(s, tx),
        Action::Document(id) => {
            if let Some(i) = s.docs.iter().position(|d| d.id == id) {
                s.select(i, tx);
            }
            if s.mode != Mode::Preview {
                tx.focus("editor", false);
            }
        }
        Action::Recent(path) => {
            s.open(&path);
            s.title(tx);
        }
    }
}
pub fn shortcut(s: &mut State, e: &KeyEvent, tx: &mut Tx) -> bool {
    if !e.mods.shift || e.mods.ctrl || e.mods.alt {
        return false;
    }
    if e.is_command('p') {
        if !tx.key_repeat() {
            open_commands(s, tx);
        }
        return true;
    }
    if e.is_command('o') {
        if !tx.key_repeat() {
            open_recent(s, tx);
        }
        return true;
    }
    false
}
pub fn key(s: &mut State, e: &KeyEvent, tx: &mut Tx) -> bool {
    if tab_switch::key(s, e, tx) || shortcut(s, e, tx) {
        return true;
    }
    let Some(p) = s.palette.open() else {
        return false;
    };
    if p.query.preedit().is_some() {
        return false;
    }
    if e.key == Key::Escape {
        if !tx.key_repeat() {
            close(s, tx);
        }
        return true;
    }
    if e.mods.cmd || e.mods.alt {
        return false;
    }
    let rows = results(s);
    let enabled: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.enabled)
        .map(|(i, _)| i)
        .collect();
    let current = active(s, &rows)
        .and_then(|i| enabled.iter().position(|e| *e == i))
        .unwrap_or(0);
    if e.key == Key::Enter {
        if !tx.key_repeat()
            && let Some(i) = active(s, &rows)
        {
            choose(s, &rows[i].id, tx);
        }
        return true;
    }
    let next = match &e.key {
        Key::Down => Some((current + 1) % enabled.len().max(1)),
        Key::Up => Some((current + enabled.len().max(1) - 1) % enabled.len().max(1)),
        Key::Home => Some(0),
        Key::End => Some(enabled.len().saturating_sub(1)),
        Key::PageDown => Some((current + 6).min(enabled.len().saturating_sub(1))),
        Key::PageUp => Some(current.saturating_sub(6)),
        Key::Text(t) if e.mods.ctrl && t == "n" => Some((current + 1) % enabled.len().max(1)),
        Key::Text(t) if e.mods.ctrl && t == "p" => {
            Some((current + enabled.len().max(1) - 1) % enabled.len().max(1))
        }
        _ => None,
    };
    if let Some(next) = next {
        if let Some(i) = enabled.get(next)
            && let Some(p) = s.palette.open_mut()
        {
            p.selected = Some(rows[*i].id.clone());
            tx.scroll_into_view(row_key(&rows[*i].id));
            tx.instant();
        }
        return true;
    }
    false
}
pub fn row_key(id: &str) -> String {
    format!("palette-option-{id}")
}
pub fn display_path(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    if let Ok(home) = std::env::var("HOME")
        && let Some(rest) = path.strip_prefix(&home)
    {
        return format!("~{rest}");
    }
    path.into_owned()
}
