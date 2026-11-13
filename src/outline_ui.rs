use crate::{
    app::State,
    outline::{self, Heading, ROW_HEIGHT},
};
use rune::{Color, Cursor, Cx, El, GrowthSpec, ScrollSpec, Spring, TextStyle, col, el, row, text};
use rune_kit::{Kit, ScrollBar};
use std::time::Duration;

pub fn build(s: &State, cx: &Cx<State>, k: &Kit) -> Option<El> {
    let doc = s.doc();
    let outline = doc.outline.clone();
    let mut snap = outline.snapshot(&doc.edit.text);
    if snap.headings.is_empty() {
        return None;
    }
    snap.active = outline.measure(cx);
    let id = doc.id;
    let generation = snap.generation;
    let expanded = snap.expanded;
    let reduced_motion = cx.is_reduced_motion();
    let pane = cx.layout_size("preview-pane").unwrap_or([530.0, 600.0]);
    let wide = (pane[0] * 0.7)
        .clamp(180.0, 260.0)
        .min((pane[0] - 24.0).max(0.0));
    let width = if expanded { wide } else { 110.0_f32.min(wide) };
    let cap = (pane[1] * 0.6)
        .clamp(72.0, 340.0)
        .min((pane[1] - 24.0).max(0.0));
    let visible_rows = ((cap - 16.0) / ROW_HEIGHT).floor().max(1.0);
    let height = (snap.headings.len() as f32)
        .min(visible_rows)
        .mul_add(ROW_HEIGHT, 16.0)
        .min(cap);
    let headings = snap.headings.clone();
    let kit = k.clone();
    let active = snap.active;
    let selected = snap.selected;
    let rows = col()
        .key(format!("outline-rows-{id}"))
        .w_full()
        .growth(GrowthSpec {
            spring: Spring::NONE,
            ..GrowthSpec::default()
        })
        .virtual_rows(
            headings.len(),
            ROW_HEIGHT,
            move |_: &State, i| outline::row_key(id, generation, i).into(),
            move |s: &State, i| {
                heading_row(
                    s,
                    &headings[i],
                    (id, generation, i),
                    &kit,
                    width - 16.0,
                    expanded,
                    reduced_motion,
                    active == Some(i),
                    selected == Some(i),
                )
            },
        )
        .virtual_rows_version((id, generation));
    let scroll_outline = outline.clone();
    let pointer_outline = outline.clone();
    let hover_outline = outline.clone();
    let mut viewport = col()
        .key(outline::scroll_key(id))
        .w_full()
        .h_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .scroll_edge_fade(6.0)
        .focusable(true)
        .preserve_focus_on_press(true)
        .role("listbox")
        .label("Document outline")
        .on_key(outline::key)
        .on_scroll_offset(move |at| scroll_outline.list_scrolled(at))
        .on_pointer_move_local(move |_: &mut State, pointer, tx| {
            pointer_outline.pointer(pointer.y);
            // The lens only moves Live bindings; don't rebuild the Markdown on mouse moves.
            tx.live_only();
        })
        .child(rows);
    if let Some(selected) = selected {
        viewport = viewport.active_descendant(outline::row_key(id, generation, selected));
    }
    Some(
        col()
            .key(outline::pane_key(id))
            .absolute(12.0, 12.0)
            .w(width)
            .h(height)
            .padding(8.0)
            .radius(9.0)
            .clip_rounded(9.0)
            .fill(if s.reduce_transparency {
                k.c.bg_base
            } else if expanded {
                k.c.bg_base.alpha(0.97)
            } else {
                Color::TRANSPARENT
            })
            .shadow(
                4.0,
                16.0,
                Color::BLACK.alpha(if expanded { 0.1 } else { 0.0 }),
            )
            .motion(Spring::new(if expanded { 0.18 } else { 0.25 }, 0.0))
            .on_hover_change(move |_: &mut State, inside, _| hover_outline.hover(inside))
            .hover_delays(Duration::from_millis(35), Duration::from_millis(180))
            .child(viewport.scroll_bar(k)),
    )
}

#[allow(clippy::too_many_arguments)]
fn heading_row(
    s: &State,
    heading: &Heading,
    identity: (u64, u64, usize),
    k: &Kit,
    width: f32,
    expanded: bool,
    reduced_motion: bool,
    active: bool,
    selected: bool,
) -> El {
    let (id, generation, i) = identity;
    let indent = f32::from(heading.level.saturating_sub(1)) * 7.0;
    let ink = if active || selected {
        k.c.text_accent
    } else {
        k.c.text_base
    };
    let scale = s.doc().outline.scale(i, reduced_motion);
    // Only inert artwork magnifies. Its unscaled parent owns a fixed-height click band.
    let artwork = row()
        .key("artwork")
        .w((width - indent - 10.0).max(24.0))
        .h(ROW_HEIGHT)
        .gap(6.0)
        .center_y()
        .origin(0.0, 0.5)
        .scale(if expanded { 1.0 } else { 0.78 })
        .live_scale(scale.smooth(Spring::new(0.12, 0.0)))
        .opacity(if expanded || s.reduce_transparency {
            1.0
        } else if active {
            0.6
        } else {
            0.25
        })
        .child(
            el().w(if heading.level == 1 {
                12.0
            } else if heading.level == 2 {
                9.0
            } else {
                5.0
            })
            .h(if heading.level <= 2 { 2.0 } else { 1.5 })
            .radius(1.0)
            .fill(ink),
        )
        .child(
            text(&heading.text)
                .font(TextStyle::new(11.0, if heading.level <= 2 { 550.0 } else { 400.0 }).lh(16.0))
                .color(ink)
                .grow(1.0)
                .min_w(0.0)
                .truncate(),
        )
        .inert()
        .a11y_hidden();
    row()
        .key(outline::row_key(id, generation, i))
        .role("option")
        .label(if heading.text.is_empty() {
            "Untitled heading"
        } else {
            &heading.text
        })
        .level(u32::from(heading.level))
        .focus_selected(selected)
        .w_full()
        .h(ROW_HEIGHT)
        .min_h(ROW_HEIGHT)
        .pl(indent + 4.0)
        .radius(4.0)
        .fill(if selected {
            k.c.layer_02
        } else {
            Color::TRANSPARENT
        })
        .hover(if expanded {
            k.c.hover
        } else {
            Color::TRANSPARENT
        })
        .cursor(Cursor::Pointer)
        .focusable(false)
        .preserve_focus_on_press(true)
        .on_press_tx(move |s: &mut State, tx| outline::choose(s, id, generation, i, tx))
        .child(artwork)
}
