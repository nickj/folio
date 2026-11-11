use crate::{app::State, palette::display_path, tab_switch};
use rune::{Color, El, ScrollSpec, TextStyle, col, el, row, text};
use rune_kit::{Kit, Markdown, MarkdownStyle, ScrollBar};

pub fn build(s: &State, k: &Kit, height: f32, layer: &mut Vec<El>) {
    let Some(switcher) = s.tab_switch.open() else {
        return;
    };
    let Some(selected) = switcher
        .ids
        .get(switcher.selected)
        .and_then(|id| s.docs.iter().find(|d| d.id == *id))
    else {
        return;
    };
    let body_height = (height - 180.0).clamp(180.0, 340.0);
    let mut list = col().w_full().gap(3.0).padding(6.0);
    for id in &switcher.ids {
        let Some(doc) = s.docs.iter().find(|d| d.id == *id) else {
            continue;
        };
        let id = *id;
        let on = selected.id == id;
        list = list.child(
            col()
                .key(format!("tab-switch-option-{id}"))
                .role("option")
                .label(doc.title())
                .focus_selected(on)
                .w_full()
                .h(51.0)
                .px(10.0)
                .gap(4.0)
                .items_start()
                .justify_center()
                .radius(5.0)
                .fill(if on { k.c.layer_02 } else { Color::TRANSPARENT })
                .hover(k.c.hover)
                .on_press_tx(move |s: &mut State, tx| tab_switch::choose(s, id, tx))
                .child(
                    text(doc.title())
                        .font(TextStyle::new(12.0, 500.0))
                        .color(k.c.text_base)
                        .truncate(),
                )
                .child(
                    text(if doc.dirty() {
                        "Edited"
                    } else if doc.path.is_none() {
                        "Untitled"
                    } else {
                        "Markdown"
                    })
                    .font(TextStyle::new(10.0, 400.0))
                    .color(k.c.text_muted),
                ),
        );
    }
    let style = MarkdownStyle::from_kit(k)
        .body_font(TextStyle::new(12.0, 400.0).lh(20.0))
        .heading_font(1, TextStyle::new(23.0, 600.0).lh(31.0))
        .heading_font(2, TextStyle::new(18.0, 600.0).lh(25.0))
        .heading_font(3, TextStyle::new(14.0, 600.0).lh(22.0));
    let preview = if selected.edit.text.is_empty() {
        text("Empty document")
            .font(TextStyle::new(13.0, 400.0))
            .color(k.c.text_muted)
    } else {
        // Bound switcher work; this is a quick rendered preview, not a second full editor.
        Markdown::new(selected.edit.text.chars().take(16000).collect::<String>())
            .style(style)
            .tasks_read_only()
            .build(k)
    };
    let content = row()
        .w_full()
        .h(body_height)
        .min_h(0.0)
        .child(
            col()
                .key("tab-switch-list")
                .w(210.0)
                .h_full()
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(list)
                .role("listbox")
                .label("Open documents")
                .scroll_bar(k),
        )
        .child(el().w(1.0).h_full().fill(k.c.border_base))
        .child(
            col()
                .key("tab-switch-preview")
                .grow(1.0)
                .min_w(0.0)
                .h_full()
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(
                    col()
                        .key(format!("tab-preview-{}", selected.id))
                        .w_full()
                        .padding(24.0)
                        .child(preview),
                )
                .scroll_bar(k),
        );
    let header = col()
        .w_full()
        .padding(16.0)
        .gap(4.0)
        .child(
            text(selected.title())
                .font(TextStyle::new(14.0, 600.0))
                .color(k.c.text_base)
                .truncate(),
        )
        .child(
            text(
                selected
                    .path
                    .as_ref()
                    .map(|p| display_path(p))
                    .unwrap_or_else(|| "Unsaved document".into()),
            )
            .font(TextStyle::new(10.0, 400.0))
            .color(k.c.text_muted)
            .truncate(),
        );
    let panel = col()
        .key("tab-switch-panel")
        .role("dialog")
        .label("Switch Document")
        .focusable(true)
        .autofocus()
        .no_focus_ring()
        .on_key(tab_switch::key)
        .w_full()
        .max_w(850.0)
        .radius(10.0)
        .fill(k.c.bg_base)
        .border(1.0, k.c.border_strong)
        .shadow(14.0, 50.0, Color::BLACK.alpha(0.22))
        .child(header)
        .child(el().w_full().h(1.0).fill(k.c.border_base))
        .child(content)
        .modal_tx(true, tab_switch::cancel)
        .instant_changes();
    layer.push(
        el().key("tab-switch-scrim")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .fill(k.c.scrim)
            .a11y_hidden(),
    );
    layer.push(
        col()
            .key("tab-switch-layer")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .padding(24.0)
            .center()
            .child(panel),
    );
}
