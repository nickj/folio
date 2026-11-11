use super::*;
use crate::ui_tests::{click, runtime};
use rune::{Key, KeyEvent, Mods, Runtime};

fn key(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(0.0);
    rt.frame(0.0);
}
fn open(rt: &mut Runtime<State>, c: char) {
    key(
        rt,
        Key::Command(c),
        Mods {
            cmd: true,
            logo: true,
            shift: true,
            ..Default::default()
        },
    );
}
fn query(rt: &mut Runtime<State>, text: &str) {
    rt.paste(text);
    rt.frame(0.0);
}
fn enter(rt: &mut Runtime<State>) {
    key(rt, Key::Enter, Mods::default());
}

#[test]
fn command_palette_filters_from_the_editor_and_runs_the_real_view_action() {
    let mut rt = runtime("# Keep this text\n", app::Mode::Edit);
    rt.update(|_, tx| tx.focus("editor", false));
    rt.frame(0.0);
    open(&mut rt, 'p');
    assert!(rt.find("palette-panel").is_some());
    query(&mut rt, "splvw");
    let rows = palette::results(&rt.state);
    assert_eq!(rows[0].id, "command:split");
    assert_eq!(rt.state.doc().edit.text, "# Keep this text\n");
    enter(&mut rt);
    assert!(rt.state.palette.open().is_none());
    assert!(rt.state.mode == app::Mode::Split);
    assert!(rt.find("source-scroll").is_some() && rt.find("preview-scroll").is_some());
}

#[test]
fn escape_restores_editor_focus_and_undo_targets_document_not_palette_query() {
    let mut rt = runtime("original", app::Mode::Edit);
    rt.update(|_, tx| tx.focus("editor", false));
    rt.frame(0.0);
    rt.paste("added ");
    rt.frame(0.0);
    open(&mut rt, 'p');
    query(&mut rt, "undo");
    enter(&mut rt);
    assert_eq!(
        rt.state.doc().edit.text,
        "original",
        "Undo must be delivered after closing the palette to the source editor"
    );
    open(&mut rt, 'p');
    query(&mut rt, "discard this query");
    key(&mut rt, Key::Escape, Mods::default());
    rt.paste("typing ");
    rt.frame(0.0);
    assert_eq!(rt.state.doc().edit.text, "typing original");
}

#[test]
fn paste_reads_the_clipboard_into_the_source_selection_and_is_undoable() {
    let mut rt = runtime("one two", app::Mode::Edit);
    let board = rune::clipboard::memory();
    board.set_content(rune::ClipboardContent::new("pasted"));
    rt.use_clipboard(&board);
    rt.update(|s, tx| {
        s.doc_mut().edit.set_selection(0, 3);
        tx.focus("editor", false);
    });
    rt.frame(0.0);
    open(&mut rt, 'p');
    query(&mut rt, "paste");
    enter(&mut rt);
    rt.frame(0.0);
    rt.frame(0.0);
    assert_eq!(rt.state.doc().edit.text, "pasted two");
    assert!(rt.state.palette.open().is_none());
    open(&mut rt, 'p');
    query(&mut rt, "undo");
    enter(&mut rt);
    assert_eq!(rt.state.doc().edit.text, "one two");
}

#[test]
fn disabled_commands_and_empty_results_do_not_run_or_dismiss_the_palette() {
    let mut rt = runtime("", app::Mode::Preview);
    open(&mut rt, 'p');
    query(&mut rt, "revert");
    assert!(!palette::results(&rt.state)[0].enabled);
    enter(&mut rt);
    assert!(rt.state.palette.open().is_some());
    assert!(rt.state.confirm.is_none());
    // Select the query, replace it through the real focused control, and retry Enter.
    key(
        &mut rt,
        Key::Command('a'),
        Mods {
            cmd: true,
            logo: true,
            ..Default::default()
        },
    );
    query(&mut rt, "zzzz-no-such-command");
    enter(&mut rt);
    assert!(palette::results(&rt.state).is_empty());
    assert!(rt.state.palette.open().is_some());
    assert_eq!(rt.state.docs.len(), 1);
}

#[test]
fn keyboard_navigation_and_mouse_choice_use_the_same_actions() {
    let mut rt = runtime("# Content", app::Mode::Preview);
    open(&mut rt, 'p');
    query(&mut rt, "appearance");
    key(&mut rt, Key::End, Mods::default());
    let rows = palette::results(&rt.state);
    let chosen = &rows[palette::active(&rt.state, &rows).unwrap()];
    assert_eq!(chosen.id, "command:dark");
    enter(&mut rt);
    assert_eq!(rt.state.appearance, Some(true));
    open(&mut rt, 'p');
    query(&mut rt, "light appearance");
    click(&mut rt, &palette::row_key("command:light"));
    assert_eq!(rt.state.appearance, Some(false));
}

#[test]
fn fuzzy_recents_match_unicode_and_paths_and_reopening_an_open_file_deduplicates_tabs() {
    let dir = tempfile::tempdir().unwrap();
    let one = dir.path().join("project/日本語 résumé.md");
    let two = dir.path().join("other/readme.md");
    std::fs::create_dir_all(one.parent().unwrap()).unwrap();
    std::fs::create_dir_all(two.parent().unwrap()).unwrap();
    std::fs::write(&one, "# First").unwrap();
    std::fs::write(&two, "# Second").unwrap();
    let mut rt = runtime("", app::Mode::Preview);
    rt.update(|s, _| {
        s.open(&one);
        s.open(&two);
    });
    rt.frame(0.0);
    open(&mut rt, 'o');
    query(&mut rt, "日本 project");
    assert_eq!(palette::results(&rt.state).len(), 1);
    enter(&mut rt);
    assert_eq!(
        rt.state.doc().path.as_deref(),
        Some(one.canonicalize().unwrap().as_path())
    );
    assert_eq!(rt.state.docs.len(), 2);
    assert_eq!(rt.state.recent.paths()[0], one.canonicalize().unwrap());
}

#[test]
fn missing_recent_files_and_failed_opens_never_replace_unsaved_work() {
    let dir = tempfile::tempdir().unwrap();
    let existing = dir.path().join("existing.md");
    let deleted = dir.path().join("deleted.md");
    std::fs::write(&existing, "existing").unwrap();
    std::fs::write(&deleted, "deleted").unwrap();
    let mut rt = runtime("", app::Mode::Preview);
    rt.update(|s, _| {
        s.open(&deleted);
        s.open(&existing);
        s.doc_mut().edit.insert("unsaved ");
    });
    rt.frame(0.0);
    std::fs::remove_file(&deleted).unwrap();
    // It isn't already open now, so selecting its history entry must try (and fail) to read it.
    let id = rt.state.docs[0].id;
    rt.update(|s, tx| s.close(id, tx));
    rt.frame(0.0);
    open(&mut rt, 'o');
    query(&mut rt, "deleted");
    enter(&mut rt);
    assert_eq!(rt.state.doc().edit.text, "unsaved existing");
    assert_eq!(rt.state.docs.len(), 1);
    assert!(rt.state.error.as_ref().unwrap().contains("Couldn’t open"));
    let before = rt.state.recent.paths().to_vec();
    rt.update(|s, _| s.open(&dir.path().join("never-opened.md")));
    rt.frame(0.0);
    assert_eq!(rt.state.recent.paths(), before);
}

#[test]
fn save_as_from_palette_updates_persisted_history_only_after_a_successful_save() {
    let dir = tempfile::tempdir().unwrap();
    let storage = dir.path().join("history/recents.json");
    let target = dir.path().join("new note 日本語.md");
    let mut rt = runtime("# Draft", app::Mode::Edit);
    rt.state.recent = recent::History::load(storage.clone()).unwrap();
    open(&mut rt, 'p');
    query(&mut rt, "save as");
    enter(&mut rt);
    assert!(matches!(
        rt.file_dialog(),
        Some((rune::FileDialogKind::Save, _))
    ));
    rt.resolve_file_dialog(None).unwrap();
    rt.frame(0.0);
    assert!(
        recent::History::load(storage.clone())
            .unwrap()
            .paths()
            .is_empty()
    );
    open(&mut rt, 'p');
    query(&mut rt, "save as");
    enter(&mut rt);
    rt.resolve_file_dialog(Some(target.clone())).unwrap();
    rt.frame(0.0);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "# Draft");
    let restarted = recent::History::load(storage).unwrap();
    assert_eq!(restarted.paths(), &[target.canonicalize().unwrap()]);
    rt.update(|s, _| s.open(&target));
    rt.frame(0.0);
    assert_eq!(
        rt.state.docs.len(),
        1,
        "reopening a newly saved file must reuse its tab, including through directory aliases"
    );
}

#[test]
fn switching_palettes_and_clear_recent_history_preserve_documents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("one.md");
    std::fs::write(&path, "# One").unwrap();
    let mut rt = runtime("", app::Mode::Preview);
    rt.update(|s, _| s.open(&path));
    rt.frame(0.0);
    open(&mut rt, 'p');
    query(&mut rt, "open recent");
    enter(&mut rt);
    assert!(rt.state.palette.open().unwrap().kind == palette::Kind::Recent);
    open(&mut rt, 'p');
    query(&mut rt, "clear recent");
    enter(&mut rt);
    assert!(rt.state.recent.paths().is_empty());
    assert_eq!(rt.state.doc().edit.text, "# One");
    assert!(path.exists());
}
