use crate::app::{Mode, State};
use rune::{Key, KeyEvent, Modifier, Tx};

#[derive(Default)]
pub enum Switcher {
    #[default]
    Closed,
    Open(OpenSwitcher),
}
pub struct OpenSwitcher {
    pub ids: Vec<u64>,
    pub selected: usize,
    previous_focus: Option<String>,
}
impl Switcher {
    pub fn is_open(&self) -> bool {
        matches!(self, Self::Open(_))
    }
    pub fn open(&self) -> Option<&OpenSwitcher> {
        match self {
            Self::Open(s) => Some(s),
            Self::Closed => None,
        }
    }
}
fn order(s: &State) -> Vec<u64> {
    let mut ids = vec![s.doc().id];
    for id in s
        .tab_order
        .iter()
        .copied()
        .chain(s.docs.iter().map(|d| d.id))
    {
        if !ids.contains(&id) && s.docs.iter().any(|d| d.id == id) {
            ids.push(id);
        }
    }
    ids
}
fn cycle(s: &mut State, backwards: bool, tx: &mut Tx) {
    if let Switcher::Open(switcher) = &mut s.tab_switch {
        let n = switcher.ids.len();
        switcher.selected = if backwards {
            (switcher.selected + n - 1) % n
        } else {
            (switcher.selected + 1) % n
        };
        tx.scroll_into_view(format!(
            "tab-switch-option-{}",
            switcher.ids[switcher.selected]
        ));
        tx.scroll_to_y("tab-switch-preview", 0.0);
        tx.instant();
    }
}
fn begin(s: &mut State, backwards: bool, tx: &mut Tx) {
    if s.docs.len() < 2 || s.confirm.is_some() {
        return;
    }
    if s.tab_switch.is_open() {
        cycle(s, backwards, tx);
        return;
    }
    let previous_focus = match s.palette.open() {
        Some(p) => p.previous_focus.clone(),
        None => tx.focused().map(str::to_owned),
    };
    crate::palette::close(s, tx);
    let ids = order(s);
    let selected = if backwards { ids.len() - 1 } else { 1 };
    s.tab_switch = Switcher::Open(OpenSwitcher {
        ids,
        selected,
        previous_focus,
    });
    tx.focus("tab-switch-panel", false);
    tx.instant();
}
pub fn cancel(s: &mut State, tx: &mut Tx) {
    if let Switcher::Open(switcher) = std::mem::take(&mut s.tab_switch)
        && let Some(focus) = switcher.previous_focus
    {
        tx.focus(focus, false);
    }
    tx.instant();
}
pub fn commit(s: &mut State, tx: &mut Tx) {
    let Switcher::Open(switcher) = std::mem::take(&mut s.tab_switch) else {
        return;
    };
    if let Some(id) = switcher.ids.get(switcher.selected)
        && let Some(i) = s.docs.iter().position(|d| d.id == *id)
    {
        s.select(i, tx);
        if s.mode != Mode::Preview {
            tx.focus("editor", false);
        }
    }
    tx.instant();
}
pub fn choose(s: &mut State, id: u64, tx: &mut Tx) {
    if let Switcher::Open(switcher) = &mut s.tab_switch
        && let Some(i) = switcher.ids.iter().position(|d| *d == id)
    {
        switcher.selected = i;
    }
    commit(s, tx);
}
pub fn next(s: &mut State, tx: &mut Tx) {
    immediate(s, false, tx);
}
pub fn previous(s: &mut State, tx: &mut Tx) {
    immediate(s, true, tx);
}
fn immediate(s: &mut State, backwards: bool, tx: &mut Tx) {
    begin(s, backwards, tx);
    commit(s, tx);
}
pub fn modifiers(s: &mut State, mods: rune::Mods, tx: &mut Tx) {
    // Also cover aggregate flags changing without a physical Control press in this
    // window (Control was held before activation, or a headless/native flags event).
    if s.tab_switch.is_open() && !mods.ctrl {
        commit(s, tx);
    }
}

pub fn key(s: &mut State, e: &KeyEvent, tx: &mut Tx) -> bool {
    if e.key == Key::Tab && e.mods.ctrl && !e.mods.alt && !e.mods.cmd && !e.mods.logo {
        begin(s, e.mods.shift, tx);
        return true;
    }
    if !s.tab_switch.is_open() {
        return false;
    }
    match &e.key {
        Key::Modifier {
            modifier: Modifier::ControlLeft | Modifier::ControlRight,
            pressed: false,
        } if !e.mods.ctrl => {
            commit(s, tx);
            true
        }
        Key::Escape => {
            cancel(s, tx);
            true
        }
        Key::Enter => {
            commit(s, tx);
            true
        }
        Key::Up | Key::Left => {
            cycle(s, true, tx);
            true
        }
        Key::Down | Key::Right => {
            cycle(s, false, tx);
            true
        }
        _ => false,
    }
}
