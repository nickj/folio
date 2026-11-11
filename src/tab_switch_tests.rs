use super::*;
use crate::ui_tests::runtime;
use rune::{Key, KeyEvent, Modifier, Mods, Runtime};

fn key(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(0.0);
    rt.frame(0.0);
}
fn cycle(rt: &mut Runtime<State>, backwards: bool) {
    key(
        rt,
        Key::Tab,
        Mods {
            ctrl: true,
            shift: backwards,
            ..Default::default()
        },
    );
}
fn release(rt: &mut Runtime<State>) {
    key(
        rt,
        Key::Modifier {
            modifier: Modifier::ControlLeft,
            pressed: false,
        },
        Mods::default(),
    );
}

#[test]
fn control_tab_previews_mru_targets_cycles_both_ways_and_commits_on_release() {
    let mut rt = runtime("# First unsaved document", app::Mode::Edit);
    let a = rt.state.doc().id;
    rt.update(|s, tx| {
        s.blank();
        s.doc_mut().edit.insert("# Second unsaved document");
        s.blank();
        s.doc_mut().edit.insert("# Third unsaved document");
        tx.focus("editor", false);
    });
    rt.frame(0.0);
    let b = rt.state.docs[1].id;
    let c = rt.state.doc().id;
    cycle(&mut rt, false);
    assert_eq!(
        rt.state.doc().id,
        c,
        "preview must not change the active document yet"
    );
    assert_eq!(
        rt.state.tab_switch.open().unwrap().ids[rt.state.tab_switch.open().unwrap().selected],
        b
    );
    assert!(rt.find(&format!("tab-preview-{b}")).is_some());
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("Second unsaved document")),
        "overlay renders the target Markdown"
    );
    cycle(&mut rt, false);
    assert!(rt.find(&format!("tab-preview-{a}")).is_some());
    cycle(&mut rt, true);
    release(&mut rt);
    assert!(rt.state.tab_switch.open().is_none());
    assert_eq!(rt.state.doc().id, b);
    assert_eq!(
        rt.state.doc().edit.text,
        "# Second unsaved document",
        "Ctrl+Tab must not insert a tab into the editor"
    );
    cycle(&mut rt, false);
    release(&mut rt);
    assert_eq!(
        rt.state.doc().id,
        c,
        "the next press toggles back to the most recently used document"
    );
}

#[test]
fn cancelling_a_tab_preview_preserves_active_document_and_mru_order() {
    let mut rt = runtime("# A", app::Mode::Edit);
    rt.update(|s, tx| {
        s.blank();
        s.doc_mut().edit.insert("# B");
        tx.focus("editor", false);
    });
    rt.frame(0.0);
    let before = rt.state.doc().id;
    let order = rt.state.tab_order.clone();
    cycle(&mut rt, true);
    key(
        &mut rt,
        Key::Escape,
        Mods {
            ctrl: true,
            ..Default::default()
        },
    );
    release(&mut rt);
    assert_eq!(rt.state.doc().id, before);
    assert_eq!(rt.state.tab_order, order);
    assert!(rt.state.tab_switch.open().is_none());
    rt.paste("Typing ");
    rt.frame(0.0);
    assert_eq!(rt.state.doc().edit.text, "# BTyping ");
}

#[test]
fn losing_window_focus_cancels_the_preview_without_switching() {
    let mut rt = runtime("# A", app::Mode::Preview);
    rt.window_focus(true);
    rt.physical_input(rune::PhysicalInput::Key {
        key: rune::PhysicalKeyCode::ControlLeft,
        pressed: true,
        repeat: false,
        mods: Mods {
            ctrl: true,
            ..Default::default()
        },
    });
    rt.frame(0.0);
    rt.update(|s, _| {
        s.blank();
        s.doc_mut().edit.insert("# B");
    });
    rt.frame(0.0);
    let before = rt.state.doc().id;
    cycle(&mut rt, false);
    assert!(rt.state.tab_switch.is_open());
    rt.window_focus(false);
    rt.frame(0.0);
    assert!(!rt.state.tab_switch.is_open());
    assert_eq!(rt.state.doc().id, before);
}

#[test]
fn tab_preview_opened_from_a_palette_restores_the_original_editor_on_cancel() {
    let mut rt = runtime("# A", app::Mode::Edit);
    rt.update(|s, tx| {
        s.blank();
        s.doc_mut().edit.insert("# B");
        tx.focus("editor", false);
    });
    rt.frame(0.0);
    key(
        &mut rt,
        Key::Command('p'),
        Mods {
            cmd: true,
            logo: true,
            shift: true,
            ..Default::default()
        },
    );
    cycle(&mut rt, false);
    assert!(rt.state.palette.open().is_none());
    assert!(rt.state.tab_switch.is_open());
    key(&mut rt, Key::Escape, Mods::default());
    rt.paste(" more");
    rt.frame(0.0);
    assert_eq!(rt.state.doc().edit.text, "# B more");
}

#[test]
fn control_tab_with_one_document_is_a_noop_not_an_editor_tab() {
    let mut rt = runtime("# Only one", app::Mode::Edit);
    rt.update(|_, tx| tx.focus("editor", false));
    rt.frame(0.0);
    cycle(&mut rt, false);
    release(&mut rt);
    assert!(rt.state.tab_switch.open().is_none());
    assert_eq!(rt.state.doc().edit.text, "# Only one");
}
