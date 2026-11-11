use super::*;
use crate::ui_tests::runtime;
use rune::Runtime;

fn settle(rt: &mut Runtime<State>) {
    // The test runtime uses the native app’s actual poll and key hooks.
    for _ in 0..5 {
        rt.frame(1.0 / 60.0);
    }
    rt.frame(0.0);
}
fn scroll(rt: &Runtime<State>, key: &str) -> f64 {
    fn find<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
        if v["key"] == key && v["scroll"].is_array() {
            return Some(v);
        }
        v["children"]
            .as_array()?
            .iter()
            .find_map(|child| find(child, key))
    }
    let tree = rt.accessibility_json();
    find(&tree, key).unwrap_or_else(|| panic!("missing {key} in {tree}"))["scroll"][1]
        .as_f64()
        .unwrap()
}
fn long_markdown() -> String {
    (0..35).map(|i| format!("## Section {i}\n\nSome **formatted words** and a paragraph that wraps on the source side with enough text to occupy several lines while the preview lays it out differently.\n\n```rust\nlet section = {i};\nprintln!(\"{{section}}\");\n```\n\n")).collect()
}

#[test]
fn scrolling_source_aligns_content_but_preview_scroll_never_moves_source() {
    let source = long_markdown();
    let mut rt = runtime(&source, app::Mode::Split);
    settle(&mut rt);
    let byte = source.find("## Section 12").unwrap();
    let source_key = format!("sync-source-{byte}");
    let preview_key = format!("sync-preview-{byte}");
    rt.update(|_, tx| tx.scroll_key_to(source_key.clone(), 20.0));
    settle(&mut rt);
    assert!(scroll(&rt, "source-scroll") > 500.0);
    assert!(scroll(&rt, "preview-scroll") > 500.0);
    let source_y = rt.find(&source_key).unwrap().1;
    let preview_y = rt.find(&preview_key).unwrap().1;
    assert!(
        (source_y - preview_y).abs() < 15.0,
        "corresponding content stays aligned: {source_y} vs {preview_y}"
    );
    let source_before = scroll(&rt, "source-scroll");
    let preview_before = scroll(&rt, "preview-scroll");
    let (x, y) = rt.find("preview-scroll").unwrap();
    rt.wheel_xy(x, y, 0.0, -250.0);
    settle(&mut rt);
    assert!((scroll(&rt, "preview-scroll") - preview_before).abs() > 100.0);
    assert!((scroll(&rt, "source-scroll") - source_before).abs() < 0.1);
    // A later source scroll takes control again, but doesn't fight preview-only input.
    let (x, y) = rt.find("source-scroll").unwrap();
    rt.wheel_xy(x, y, 0.0, -180.0);
    settle(&mut rt);
    assert!(scroll(&rt, "source-scroll") > source_before + 100.0);
}

#[test]
fn editing_inside_a_wrapped_paragraph_follows_the_visual_caret_line() {
    let mut source = long_markdown();
    let paragraph_start = source.len();
    source.push_str(&"A paragraph with **formatting** and Unicode café 日本語 that wraps differently on each side. ".repeat(25));
    source.push_str("\n\n## After\n\nMore text.\n");
    let byte = paragraph_start + 1000;
    let byte = (byte..source.len())
        .find(|b| source.is_char_boundary(*b))
        .unwrap();
    let mut rt = runtime(&source, app::Mode::Split);
    rt.update(|s, tx| {
        s.doc_mut().edit.set_caret(byte);
        tx.focus("editor", false);
        tx.scroll_key_to(format!("sync-source-{byte}"), 150.0);
    });
    settle(&mut rt);
    rt.paste("more words ");
    settle(&mut rt);
    let caret = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("editor"))
        .unwrap()
        .caret
        .unwrap();
    let preview_y = rt
        .find(&format!("sync-preview-{}", rt.state.doc().edit.focus))
        .unwrap()
        .1;
    assert!(
        (preview_y - (caret[1] + caret[3] / 2.0)).abs() < 20.0,
        "preview follows the edited visual line, not the paragraph start: preview={preview_y}, caret={caret:?}"
    );
}

#[test]
fn typing_realigns_preview_to_the_edited_line_after_independent_preview_scroll() {
    let source = long_markdown();
    let mut rt = runtime(&source, app::Mode::Split);
    let byte = source.find("## Section 16").unwrap();
    let source_key = format!("sync-source-{byte}");
    let preview_key = format!("sync-preview-{byte}");
    rt.update(|s, tx| {
        s.doc_mut().edit.set_caret(byte + "## Section 16".len());
        tx.focus("editor", false);
        tx.scroll_key_to(source_key.clone(), 80.0);
    });
    settle(&mut rt);
    rt.update(|_, tx| tx.scroll_to_y("preview-scroll", 0.0));
    settle(&mut rt);
    let source_before = scroll(&rt, "source-scroll");
    assert!(source_before > 500.0);
    assert_eq!(scroll(&rt, "preview-scroll"), 0.0);
    rt.paste(" revised");
    settle(&mut rt);
    assert!(rt.state.doc().edit.text.contains("## Section 16 revised"));
    assert!(scroll(&rt, "preview-scroll") > 500.0);
    let source_y = rt.find(&source_key).unwrap().1;
    let preview_y = rt.find(&preview_key).unwrap().1;
    assert!(
        (source_y - preview_y).abs() < 15.0,
        "edited line aligns after relayout: {source_y} vs {preview_y}"
    );
    assert!((scroll(&rt, "source-scroll") - source_before).abs() < 0.1);
}
