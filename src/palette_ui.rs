use crate::{
    app::State,
    palette::{self, Kind},
};
use rune::{El, ScrollSpec, TextStyle, col, el, row, text};
use rune_kit::{Button, ButtonVariant, Kit, ScrollBar, TextInput};

pub fn build(s: &State, k: &Kit, height: f32, layer: &mut Vec<El>) {
    let Some(p) = s.palette.open() else { return };
    let rows = palette::results(s);
    let active = palette::active(s, &rows);
    let title = match p.kind {
        Kind::Commands => "Commands",
        Kind::Recent => "Recent Documents",
    };
    let hint = match p.kind {
        Kind::Commands => "Run a command…",
        Kind::Recent => "Find recent documents…",
    };
    let mut field = TextInput::new(&p.query)
        .plain()
        .width(None)
        .height(50.0)
        .text_style(TextStyle::new(15.0, 400.0).lh(22.0))
        .leading_icon(k.glyph.search)
        .placeholder(hint)
        .label(title)
        .on_edit(|s: &mut State| &mut s.palette.open_mut().expect("palette is open").query)
        .on_change(palette::query_changed)
        .on_key(palette::key);
    if let Some(i) = active {
        field = field.active_descendant(Some(palette::row_key(&rows[i].id)));
    }
    let field = field.build(k).key("palette-query").autofocus();
    let mut list = col().w_full().gap(2.0).padding(6.0);
    for (i, result) in rows.iter().enumerate() {
        let id = result.id.clone();
        let selected = active == Some(i);
        let label = col()
            .grow(1.0)
            .min_w(0.0)
            .gap(3.0)
            .child(
                text(&result.label)
                    .font(TextStyle::new(13.0, 500.0))
                    .color(k.c.text_base)
                    .truncate(),
            )
            .child(
                text(&result.detail)
                    .font(TextStyle::new(10.0, 400.0))
                    .color(k.c.text_muted)
                    .truncate(),
            );
        let mut item = row()
            .key(palette::row_key(&id))
            .role("option")
            .label(&result.label)
            .description(&result.detail)
            .position_in_set(i + 1, rows.len())
            .focus_selected(selected)
            .disabled(!result.enabled)
            .w_full()
            .h(48.0)
            .px(12.0)
            .gap(12.0)
            .center_y()
            .radius(5.0)
            .fill(if selected {
                k.c.layer_02
            } else {
                rune::Color::TRANSPARENT
            })
            .child(label)
            .child(
                text(&result.shortcut)
                    .font(TextStyle::new(11.0, 400.0))
                    .color(k.c.text_muted)
                    .a11y_hidden(),
            );
        item = if result.enabled {
            item.hover(k.c.hover)
                .on_press_tx(move |s: &mut State, tx| palette::choose(s, &id, tx))
        } else {
            item.layer_opacity(0.4)
        };
        list = list.child(item);
    }
    let body = if rows.is_empty() {
        let message = if p.kind == Kind::Recent && s.recent.paths().is_empty() {
            "No recent documents"
        } else if p.kind == Kind::Recent {
            "No matching documents"
        } else {
            "No matching commands"
        };
        let mut empty = col().w_full().h(120.0).center().gap(14.0).child(
            text(message)
                .font(TextStyle::new(13.0, 400.0))
                .color(k.c.text_muted),
        );
        if p.kind == Kind::Recent && s.recent.paths().is_empty() {
            empty = empty.child(
                Button::new("Open…")
                    .variant(ButtonVariant::Neutral)
                    .on_press_tx(|s: &mut State, tx| {
                        palette::close(s, tx);
                        crate::app::open_dialog(s, tx);
                    })
                    .build(k),
            );
        }
        empty
    } else {
        col()
            .key("palette-results")
            .w_full()
            .h((rows.len() as f32 * 50.0 + 12.0).min((height - 180.0).clamp(100.0, 360.0)))
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(list)
            .scroll_bar(k)
    };
    let panel = col()
        .key("palette-panel")
        .role("dialog")
        .label(title)
        .w_full()
        .max_w(600.0)
        .radius(10.0)
        .fill(k.c.bg_base)
        .border(1.0, k.c.border_strong)
        .shadow(14.0, 50.0, rune::Color::BLACK.alpha(0.22))
        .child(field)
        .child(el().w_full().h(1.0).fill(k.c.border_base))
        .child(body.role("listbox").label(format!("{title} results")))
        .modal_tx(true, palette::close)
        .instant_changes();
    layer.push(
        el().key("palette-scrim")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .fill(k.c.scrim)
            .a11y_hidden(),
    );
    layer.push(
        col()
            .key("palette-layer")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .px(20.0)
            .pt(82.0)
            .items_center()
            .child(panel),
    );
}
