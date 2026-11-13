use super::*;
use crate::ui_tests::{click, runtime};
use rune::{Key, KeyEvent, Runtime};

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..24 {
        rt.frame(1.0 / 60.0);
    }
    rt.frame(0.0);
}
fn scroll(rt: &Runtime<State>, key: &str) -> f64 {
    fn find<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
        if v["key"] == key && v["scroll"].is_array() {
            return Some(v);
        }
        v["children"].as_array()?.iter().find_map(|c| find(c, key))
    }
    let tree = rt.accessibility_json();
    find(&tree, key).unwrap_or_else(|| panic!("missing {key}"))["scroll"][1]
        .as_f64()
        .unwrap()
}
fn sections(n: usize) -> String {
    (0..n)
        .map(|i| {
            format!(
                "## Section {i}\n\n{}\n\n",
                "Paragraph with **formatted** words and Unicode café 日本語. ".repeat(12)
            )
        })
        .collect()
}
fn row_key(rt: &Runtime<State>, i: usize) -> String {
    let doc = rt.state.doc();
    let snap = doc.outline.snapshot(&doc.edit.text);
    outline::row_key(doc.id, snap.generation, i)
}
fn pane_rect(rt: &Runtime<State>) -> [f32; 4] {
    let key = outline::pane_key(rt.state.doc().id);
    rt.elements()
        .iter()
        .find(|e| e.key.as_deref() == Some(&key))
        .unwrap()
        .rect
}
fn key(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Default::default(),
    });
    settle(rt);
}
fn artwork_scale(rt: &Runtime<State>, i: usize) -> f32 {
    let key = row_key(rt, i);
    rt.elements()
        .iter()
        .find(|e| e.key.as_deref() == Some("artwork") && e.path.contains(&key))
        .unwrap()
        .scale
}
fn land(rt: &Runtime<State>, i: usize) {
    let snap = rt.state.doc().outline.snapshot(&rt.state.doc().edit.text);
    let heading = rt.find(&snap.headings[i].target).unwrap();
    let viewport = rt
        .elements()
        .iter()
        .find(|e| e.key.as_deref() == Some("preview-scroll"))
        .unwrap()
        .rect;
    let block = rt
        .elements()
        .iter()
        .find(|e| {
            e.path
                .split('/')
                .filter(|p| !p.starts_with('#'))
                .collect::<Vec<_>>()
                .join("/")
                .ends_with(&snap.headings[i].target)
        })
        .unwrap()
        .rect;
    assert!(
        (block[1] - viewport[1] - outline::LANDING_INSET).abs() < 2.0,
        "heading {i} at {heading:?}; block {block:?}, viewport {viewport:?}"
    );
}

#[test]
fn hover_expands_without_reflow_or_scrolling_and_lens_keeps_click_bands_stable() {
    let mut rt = runtime(&sections(12), app::Mode::Split);
    settle(&mut rt);
    let compact = pane_rect(&rt);
    let markdown_before = rt.find("markdown/b0").unwrap();
    let editor_before = scroll(&rt, "source-scroll");
    let preview_before = scroll(&rt, "preview-scroll");
    let target_key = row_key(&rt, 3);
    let (x, y) = rt.find(&target_key).unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    assert!(pane_rect(&rt)[2] > compact[2] + 80.0);
    let bands = (0..8)
        .map(|i| rt.find(&row_key(&rt, i)).unwrap().1)
        .collect::<Vec<_>>();
    assert!(artwork_scale(&rt, 3) > artwork_scale(&rt, 4) + 0.05);
    // Rune's debug stale-view probe deliberately commits once per quarter second.
    rt.check_live_only(false);
    let commits = rt.stats.commits;
    let (_, next_y) = rt.find(&row_key(&rt, 6)).unwrap();
    rt.pointer_move(x, next_y);
    for _ in 0..24 {
        rt.frame(1.0 / 60.0);
    }
    assert!(artwork_scale(&rt, 6) > artwork_scale(&rt, 3) + 0.1);
    assert_eq!(
        rt.stats.commits, commits,
        "moving the lens must not rebuild the Markdown"
    );
    for (i, before) in bands.iter().enumerate() {
        assert!((rt.find(&row_key(&rt, i)).unwrap().1 - before).abs() < 0.1);
    }
    assert_eq!(rt.find("markdown/b0").unwrap(), markdown_before);
    assert_eq!(scroll(&rt, "source-scroll"), editor_before);
    assert_eq!(scroll(&rt, "preview-scroll"), preview_before);
    rt.pointer_move(1000.0, 700.0);
    rt.frame(0.1);
    assert!(
        pane_rect(&rt)[2] > compact[2] + 80.0,
        "exit grace prevents flicker"
    );
    settle(&mut rt);
    assert!(
        (pane_rect(&rt)[2] - compact[2]).abs() < 0.5,
        "compact={compact:?}, after={:?}, expanded={}, focus={:?}",
        pane_rect(&rt),
        rt.state
            .doc()
            .outline
            .snapshot(&rt.state.doc().edit.text)
            .expanded,
        rt.elements()
            .iter()
            .filter(|e| e.focused)
            .map(|e| &e.key)
            .collect::<Vec<_>>()
    );
}

#[test]
fn mouse_navigation_collapses_on_leave_even_when_the_outline_has_pointer_focus() {
    for (mode, keyboard_origin) in [
        (app::Mode::Preview, false),
        (app::Mode::Split, false),
        (app::Mode::Preview, true),
        (app::Mode::Split, true),
    ] {
        let mut rt = runtime(&sections(12), mode);
        settle(&mut rt);
        let compact = pane_rect(&rt);
        if keyboard_origin {
            rt.update(outline::focus);
            settle(&mut rt);
        }
        let target = row_key(&rt, 5);
        let (x, y) = rt.find(&target).unwrap();
        rt.pointer_move(x, y);
        settle(&mut rt);
        click(&mut rt, &target);
        settle(&mut rt);
        land(&rt, 5);
        assert!(pane_rect(&rt)[2] > compact[2] + 80.0);
        let landed = scroll(&rt, "preview-scroll");
        rt.pointer_move(1000.0, 700.0);
        rt.frame(0.2); // Allow the hover-exit grace before settling the collapse.
        settle(&mut rt);
        assert!(
            (pane_rect(&rt)[2] - compact[2]).abs() < 0.5,
            "a mouse click must not pin the outline open after mouse-out: compact={compact:?}, after={:?}, expanded={}, focused={:?}",
            pane_rect(&rt),
            rt.state
                .doc()
                .outline
                .snapshot(&rt.state.doc().edit.text)
                .expanded,
            rt.elements()
                .iter()
                .filter(|e| e.focused)
                .map(|e| (&e.key, e.focus_visible))
                .collect::<Vec<_>>()
        );
        assert!((scroll(&rt, "preview-scroll") - landed).abs() < 0.5);
        // Returning to the outline still expands it and allows another navigation.
        let target = row_key(&rt, 2);
        let (x, y) = rt.find(&target).unwrap();
        rt.pointer_move(x, y);
        settle(&mut rt);
        click(&mut rt, &target);
        settle(&mut rt);
        land(&rt, 2);
    }
}

#[test]
fn keyboard_navigation_stays_expanded_when_the_pointer_leaves() {
    let mut rt = runtime(&sections(12), app::Mode::Preview);
    settle(&mut rt);
    let compact = pane_rect(&rt);
    rt.update(outline::focus);
    settle(&mut rt);
    let (x, y) = rt.find(&row_key(&rt, 3)).unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    rt.pointer_move(1000.0, 700.0);
    settle(&mut rt);
    assert!(pane_rect(&rt)[2] > compact[2] + 80.0);
    key(&mut rt, Key::Down);
    key(&mut rt, Key::Enter);
    land(&rt, 1);
    assert!(pane_rect(&rt)[2] > compact[2] + 80.0);
    key(&mut rt, Key::Escape);
    assert!((pane_rect(&rt)[2] - compact[2]).abs() < 0.5);
}

#[test]
fn outline_click_wins_over_pending_source_follow_and_preserves_source_selection() {
    let source = sections(18);
    let mut rt = runtime(&source, app::Mode::Split);
    settle(&mut rt);
    rt.update(|s, tx| {
        s.doc_mut().edit.set_selection(4, 11);
        tx.focus("editor", false);
    });
    settle(&mut rt);
    let selected = rt.state.doc().edit.selected().to_owned();
    let caret = rt.state.doc().edit.focus;
    let source_before = scroll(&rt, "source-scroll");
    // Deliberately leave work queued from the other side as the click arrives.
    rt.state.doc().split_sync.edited(0);
    let k = row_key(&rt, 7);
    click(&mut rt, &k);
    settle(&mut rt);
    assert!(scroll(&rt, "preview-scroll") > 700.0);
    land(&rt, 7);
    assert_eq!(scroll(&rt, "source-scroll"), source_before);
    assert_eq!(rt.state.doc().edit.focus, caret);
    assert_eq!(rt.state.doc().edit.selected(), selected);
    assert_eq!(rt.state.doc().edit.text, source);
    let (x, y) = rt.find("source-scroll").unwrap();
    rt.wheel_xy(x + 100.0, y, 0.0, -220.0);
    settle(&mut rt);
    assert!(scroll(&rt, "source-scroll") > source_before + 100.0);
    assert!(
        scroll(&rt, "preview-scroll") < 700.0,
        "later source input can take control again"
    );
}

#[test]
fn formatted_setext_duplicate_and_nested_headings_navigate_to_their_rendered_blocks() {
    let filler = format!(
        "\n\n{}\n\n",
        "A paragraph with enough words for some scrolling. ".repeat(40)
    );
    let source = format!(
        "# **Overview** and `code`{filler}Repeated\n--------{filler}```md\n# Not a heading\n```\n\n> ### Quoted\n>\n> Text\n\n{filler}- ## Listed\n\n{filler}###### Deep\n\n{filler}## Repeated{filler}"
    );
    let mut rt = runtime(&source, app::Mode::Preview);
    settle(&mut rt);
    let snap = rt.state.doc().outline.snapshot(&source);
    assert_eq!(
        snap.headings.len(),
        6,
        "code fences must not add outline targets"
    );
    // Exercise these targets through actual pointer hit testing, not a second slug parser.
    for i in [1, 2, 3, 4, 5] {
        let k = row_key(&rt, i);
        click(&mut rt, &k);
        settle(&mut rt);
        land(&rt, i);
    }
    assert_eq!(snap.headings[0].text, "Overview and code");
    assert_eq!(snap.headings[1].text, snap.headings[5].text);
    assert_ne!(snap.headings[1].target, snap.headings[5].target);
}

#[test]
fn long_outline_scrolls_independently_and_keyboard_reaches_unmounted_last_heading() {
    let mut rt = runtime(&sections(140), app::Mode::Preview);
    settle(&mut rt);
    let last = row_key(&rt, 139);
    assert!(
        rt.find(&last).is_none(),
        "far outline rows should not be laid out"
    );
    let first = row_key(&rt, 0);
    let (x, y) = rt.find(&first).unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    rt.wheel_xy(x, y, 0.0, -600.0);
    settle(&mut rt);
    let scroll_key = outline::scroll_key(rt.state.doc().id);
    assert!(scroll(&rt, &scroll_key) > 300.0);
    assert_eq!(scroll(&rt, "preview-scroll"), 0.0);
    rt.pointer_move(1000.0, 700.0);
    rt.update(outline::focus);
    settle(&mut rt);
    key(&mut rt, Key::End);
    assert!(rt.find(&last).is_some());
    assert_eq!(
        scroll(&rt, "preview-scroll"),
        0.0,
        "choosing with arrows never navigates the document"
    );
    key(&mut rt, Key::Enter);
    assert!(scroll(&rt, "preview-scroll") > 20_000.0);
    let snap = rt.state.doc().outline.snapshot(&rt.state.doc().edit.text);
    assert_eq!(snap.active, Some(139));
    key(&mut rt, Key::Escape);
    assert!(
        !rt.state
            .doc()
            .outline
            .snapshot(&rt.state.doc().edit.text)
            .expanded
    );
}

#[test]
fn stale_clicks_after_edit_or_tab_switch_do_not_navigate_a_different_heading() {
    let mut rt = runtime(&sections(12), app::Mode::Split);
    settle(&mut rt);
    let id = rt.state.doc().id;
    let snap = rt.state.doc().outline.snapshot(&rt.state.doc().edit.text);
    let (x, y) = rt.find(&row_key(&rt, 4)).unwrap();
    rt.pointer_down(x, y);
    rt.state
        .doc_mut()
        .edit
        .replace_text(format!("# Inserted\n\n{}", sections(12)));
    // Release on the old frame, before the new parse has been presented.
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert_eq!(scroll(&rt, "preview-scroll"), 0.0);
    let fresh = row_key(&rt, 4);
    click(&mut rt, &fresh);
    settle(&mut rt);
    land(&rt, 4);
    rt.update(|s, tx| {
        s.blank();
        s.doc_mut().edit.replace_text(sections(12));
        s.mode = app::Mode::Preview;
        outline::choose(s, id, snap.generation, 4, tx);
    });
    settle(&mut rt);
    assert_eq!(scroll(&rt, "preview-scroll"), 0.0);
    assert!(
        !rt.state
            .doc()
            .outline
            .snapshot(&rt.state.doc().edit.text)
            .expanded
    );
}

#[test]
fn focusing_from_edit_opens_only_the_preview_and_heading_removal_hides_the_overlay() {
    let mut rt = runtime(&sections(4), app::Mode::Edit);
    let id = rt.state.doc().id;
    assert!(rt.find(&outline::pane_key(id)).is_none());
    rt.update(outline::focus);
    settle(&mut rt);
    assert!(rt.state.mode == app::Mode::Split);
    assert!(pane_rect(&rt)[2] > 180.0);
    key(&mut rt, Key::Down);
    key(&mut rt, Key::Enter);
    land(&rt, 1);
    key(&mut rt, Key::Escape);
    assert!(rt.state.doc().edit.focused);
    rt.update(|s, tx| {
        s.doc_mut()
            .edit
            .replace_text("Plain text without sections.");
        s.changed(tx);
    });
    settle(&mut rt);
    assert!(rt.find(&outline::pane_key(id)).is_none());
}

#[test]
fn resizing_and_text_zoom_remeasure_the_landing_position() {
    let mut rt = runtime(&sections(15), app::Mode::Preview);
    settle(&mut rt);
    rt.set_viewport((720.0, 440.0), 1.0);
    rt.update(app::zoom_in);
    rt.update(app::zoom_in);
    settle(&mut rt);
    rt.update(outline::focus);
    key(&mut rt, Key::Down);
    key(&mut rt, Key::Down);
    key(&mut rt, Key::Enter);
    land(&rt, 2);
    let pane = pane_rect(&rt);
    let viewport = rt
        .elements()
        .iter()
        .find(|e| e.key.as_deref() == Some("preview-pane"))
        .unwrap()
        .rect;
    assert!(pane[0] >= viewport[0] && pane[1] >= viewport[1]);
    assert!(pane[0] + pane[2] <= viewport[0] + viewport[2]);
    assert!(pane[1] + pane[3] <= viewport[1] + viewport[3]);
}

#[test]
fn reduce_motion_stops_the_lens_without_disabling_heading_navigation() {
    let mut rt = runtime(&sections(12), app::Mode::Preview);
    settle(&mut rt);
    let (x, y) = rt.find(&row_key(&rt, 3)).unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    assert!(artwork_scale(&rt, 3) > artwork_scale(&rt, 5) + 0.1);
    // Change the host preference while a live magnifying binding is already mounted.
    rt.set_system_reduced_motion(true);
    settle(&mut rt);
    let (x, y) = rt.find(&row_key(&rt, 6)).unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    assert!((artwork_scale(&rt, 6) - artwork_scale(&rt, 3)).abs() < 0.01);
    let k = row_key(&rt, 6);
    click(&mut rt, &k);
    land(&rt, 6);
}

#[test]
fn passive_outline_tracks_the_current_section_but_hover_never_moves_it() {
    let mut rt = runtime(&sections(100), app::Mode::Preview);
    settle(&mut rt);
    let snap = rt.state.doc().outline.snapshot(&rt.state.doc().edit.text);
    let target = snap.headings[75].target.clone();
    rt.update(|_, tx| tx.scroll_key_to(target, outline::LANDING_INSET));
    settle(&mut rt);
    let snap = rt.state.doc().outline.snapshot(&rt.state.doc().edit.text);
    assert_eq!(snap.active, Some(75));
    let active = row_key(&rt, 75);
    assert!(
        rt.find(&active).is_some(),
        "the current section should be visible in the ghost outline"
    );
    let (x, y) = rt.find(&active).unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    let outline_key = outline::scroll_key(rt.state.doc().id);
    let before = scroll(&rt, &outline_key);
    let snap = rt.state.doc().outline.snapshot(&rt.state.doc().edit.text);
    let target = snap.headings[80].target.clone();
    rt.update(|_, tx| tx.scroll_key_to(target, outline::LANDING_INSET));
    settle(&mut rt);
    assert_eq!(
        scroll(&rt, &outline_key),
        before,
        "no auto-follow while the user is inspecting headings"
    );
}
