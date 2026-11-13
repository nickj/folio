//! Preview-only heading navigation. Targets are rendered block paths, never title slugs.
use crate::app::{Mode, State};
use rune::{Cx, Key, KeyEvent, Live, ScrollPos, Tx};
use rune_kit::markdown::{Block, parse_cached};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

pub const ROW_HEIGHT: f32 = 24.0;
pub const LANDING_INSET: f32 = 28.0;

#[derive(Clone, Debug)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub target: String,
}

#[derive(Default)]
struct Position {
    parsed: Option<Rc<Vec<Block>>>,
    headings: Rc<Vec<Heading>>,
    generation: u64,
    ys: Vec<Option<f32>>,
    anchors: Vec<(usize, f32)>,
    preview: ScrollPos,
    list: ScrollPos,
    active: Option<usize>,
    dirty: bool,
    hovered: bool,
    focused: bool,
    selected: Option<usize>,
    pointer_y: Option<f32>,
    reduced_motion: bool,
    scales: BTreeMap<usize, Live<f32>>,
}

#[derive(Clone, Default)]
pub struct Outline(Rc<RefCell<Position>>);

pub struct Snapshot {
    pub headings: Rc<Vec<Heading>>,
    pub generation: u64,
    pub expanded: bool,
    pub active: Option<usize>,
    pub selected: Option<usize>,
}

fn collect(blocks: &[Block], parent: &str, out: &mut Vec<Heading>) {
    for (i, block) in blocks.iter().enumerate() {
        // These paths mirror the pinned Rune renderer, including quote/list descendants.
        let target = format!("{parent}/b{i}");
        match block {
            Block::Heading(level, inline) => out.push(Heading {
                level: *level,
                // Inline code padding is layout-only, not part of the heading's name.
                text: inline
                    .text
                    .replace("\u{202f}\u{202f}\u{2066}", "")
                    .replace("\u{2069}\u{202f}\u{202f}", "")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                target,
            }),
            Block::Quote(inner) => collect(inner, &target, out),
            Block::List { items, .. } => {
                for (j, item) in items.iter().enumerate() {
                    collect(item, &format!("{target}/i{j}"), out);
                }
            }
            _ => {}
        }
    }
}

impl Outline {
    pub fn snapshot(&self, source: &str) -> Snapshot {
        let parsed = parse_cached(source);
        let mut p = self.0.borrow_mut();
        if p.parsed
            .as_ref()
            .is_none_or(|old| !Rc::ptr_eq(old, &parsed))
        {
            let mut headings = Vec::new();
            collect(&parsed, "markdown", &mut headings);
            p.ys = vec![None; headings.len()];
            p.anchors.clear();
            p.headings = Rc::new(headings);
            p.parsed = Some(parsed);
            p.generation += 1;
            p.scales.clear();
            p.active = None;
            p.selected = None;
            p.pointer_y = None;
        }
        Snapshot {
            headings: p.headings.clone(),
            generation: p.generation,
            expanded: p.hovered || p.focused,
            active: p.active,
            selected: p.focused.then_some(p.selected).flatten(),
        }
    }

    pub fn measure<S>(&self, cx: &Cx<S>) -> Option<usize> {
        let preview = cx.layout("preview-scroll")?;
        let mut p = self.0.borrow_mut();
        p.ys = p
            .headings
            .iter()
            .map(|h| cx.layout(&h.target).map(|r| r[1] - preview[1]))
            .collect();
        p.anchors =
            p.ys.iter()
                .enumerate()
                .filter_map(|(i, y)| y.map(|y| (i, y)))
                .collect();
        update_active(&mut p);
        p.active
    }

    /// Only crossing a heading asks for a commit; ordinary scroll frames just record offset.
    pub fn preview_scrolled(&self, at: ScrollPos) -> bool {
        let mut p = self.0.borrow_mut();
        p.preview = at;
        let changed = update_active(&mut p);
        p.dirty |= changed;
        changed
    }

    pub fn list_scrolled(&self, at: ScrollPos) {
        let mut p = self.0.borrow_mut();
        // Keep only bindings near the mounted window, even after traversing a huge outline.
        p.scales.retain(|i, _| {
            let y = *i as f32 * ROW_HEIGHT;
            y >= at.offset[1] - at.viewport[1] * 2.0 && y <= at.offset[1] + at.viewport[1] * 3.0
        });
        p.list = at;
        update_scales(&p);
    }

    pub fn hover(&self, inside: bool) {
        let mut p = self.0.borrow_mut();
        p.hovered = inside;
        if !inside {
            p.pointer_y = None;
        }
        update_scales(&p);
    }

    pub fn pointer(&self, y: f32) {
        let mut p = self.0.borrow_mut();
        p.pointer_y = Some(y);
        update_scales(&p);
    }

    pub fn scale(&self, i: usize, reduced_motion: bool) -> Live<f32> {
        let mut p = self.0.borrow_mut();
        p.reduced_motion = reduced_motion;
        let value = magnification(&p, i);
        let scale = p
            .scales
            .entry(i)
            .or_insert_with(|| Live::new(value))
            .clone();
        scale.set(value);
        scale
    }

    fn poll(&self, visible: bool, focused: bool) -> bool {
        let mut p = self.0.borrow_mut();
        let changed = p.focused != focused || (!visible && p.hovered) || p.dirty;
        if focused && !p.focused {
            p.selected = p.active.or((!p.headings.is_empty()).then_some(0));
        }
        p.focused = focused;
        if !visible {
            p.hovered = false;
            p.pointer_y = None;
            p.ys.fill(None);
            p.anchors.clear();
            p.scales.clear();
        }
        p.dirty = false;
        update_scales(&p);
        changed
    }
}

fn update_active(p: &mut Position) -> bool {
    let active = if p.preview.max[1] > 0.0 && p.preview.offset[1] >= p.preview.max[1] - 0.5 {
        // The last short section cannot always reach the top of its viewport.
        p.anchors.last().map(|(i, _)| *i)
    } else {
        let n = p
            .anchors
            .partition_point(|(_, y)| *y <= p.preview.offset[1] + LANDING_INSET + 1.0);
        n.checked_sub(1).map(|n| p.anchors[n].0)
    };
    let changed = p.active != active;
    p.active = active;
    changed
}

fn magnification(p: &Position, i: usize) -> f32 {
    if p.reduced_motion {
        return 1.0;
    }
    let center = i as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0;
    let lens = if p.hovered {
        p.pointer_y.map(|y| y + p.list.offset[1])
    } else if p.focused {
        p.selected.map(|i| i as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0)
    } else {
        None
    };
    let strength = lens.map_or(0.0, |y| {
        (1.0 - (center - y).abs() / (ROW_HEIGHT * 2.5)).max(0.0)
    });
    1.0 + 0.22 * strength * strength
}
fn update_scales(p: &Position) {
    for (i, scale) in &p.scales {
        scale.set(magnification(p, *i));
    }
}

pub fn pane_key(id: u64) -> String {
    format!("outline-{id}")
}
pub fn scroll_key(id: u64) -> String {
    format!("outline-scroll-{id}")
}
pub fn row_key(id: u64, generation: u64, i: usize) -> String {
    format!("outline-heading-{id}-{generation}-{i}")
}

pub fn poll(s: &mut State, tx: &mut Tx) -> bool {
    let mut changed = false;
    for doc in &s.docs {
        let visible = doc.id == s.doc().id && s.mode != Mode::Edit;
        // Pointer focus lets the list receive keys, but must not pin a hover overlay open.
        // Only explicit keyboard navigation keeps it expanded after the pointer leaves.
        let focused =
            visible && tx.focus_visible() && tx.focused() == Some(scroll_key(doc.id).as_str());
        {
            let p = doc.outline.0.borrow();
            if visible
                && p.dirty
                && !p.hovered
                && !focused
                && let Some(i) = p.active
            {
                tx.scroll_into_view_in(scroll_key(doc.id), row_key(doc.id, p.generation, i));
            }
        }
        changed |= doc.outline.poll(visible, focused);
    }
    changed
}

pub fn focus(s: &mut State, tx: &mut Tx) {
    let snap = s.doc().outline.snapshot(&s.doc().edit.text);
    if snap.headings.is_empty() {
        return;
    }
    if s.mode == Mode::Edit {
        s.mode = Mode::Split;
    }
    let id = s.doc().id;
    let outline = s.doc().outline.clone();
    outline.poll(true, true);
    let selected = outline.0.borrow().selected.unwrap_or(0);
    tx.focus(scroll_key(id), true);
    tx.scroll_into_view_in(scroll_key(id), row_key(id, snap.generation, selected));
}

pub fn choose(s: &mut State, id: u64, generation: u64, i: usize, tx: &mut Tx) {
    if s.doc().id != id || s.mode == Mode::Edit {
        return;
    }
    let snap = s.doc().outline.snapshot(&s.doc().edit.text);
    if snap.generation != generation {
        return;
    }
    let Some(heading) = snap.headings.get(i) else {
        return;
    };
    // Explicit preview navigation wins over an already queued source-follow, but a later
    // source scroll or edit is still free to take control again.
    s.doc().split_sync.preview_navigated();
    let mut p = s.doc().outline.0.borrow_mut();
    p.selected = Some(i);
    if let Some(y) = p.ys.get(i).copied().flatten() {
        let y = (y - LANDING_INSET).clamp(0.0, p.preview.max[1]);
        // Travel time is bounded even for a jump across thousands of document lines.
        tx.scroll_to_with("preview-scroll", 0.0, y, rune::Spring::new(0.22, 0.0));
    } else {
        tx.scroll_key_to(&heading.target, LANDING_INSET);
    }
    update_scales(&p);
}

pub fn key(s: &mut State, e: &KeyEvent, tx: &mut Tx) -> bool {
    let snap = s.doc().outline.snapshot(&s.doc().edit.text);
    if snap.headings.is_empty() {
        return false;
    }
    let id = s.doc().id;
    let mut p = s.doc().outline.0.borrow_mut();
    let selected = p
        .selected
        .or(p.active)
        .unwrap_or(0)
        .min(snap.headings.len() - 1);
    if e.mods.cmd || e.mods.ctrl || e.mods.alt {
        return false;
    }
    let next = match &e.key {
        Key::Up => selected.saturating_sub(1),
        Key::Down => (selected + 1).min(snap.headings.len() - 1),
        Key::Home => 0,
        Key::End => snap.headings.len() - 1,
        Key::Enter => {
            drop(p);
            choose(s, id, snap.generation, selected, tx);
            return true;
        }
        Key::Text(t) if t == " " => {
            drop(p);
            choose(s, id, snap.generation, selected, tx);
            return true;
        }
        Key::Escape => {
            p.focused = false;
            p.hovered = false;
            p.pointer_y = None;
            update_scales(&p);
            tx.blur();
            if s.mode == Mode::Split {
                tx.focus("editor", false);
            }
            return true;
        }
        _ => return false,
    };
    p.focused = true;
    p.selected = Some(next);
    update_scales(&p);
    tx.scroll_into_view_in(scroll_key(id), row_key(id, snap.generation, next));
    true
}
