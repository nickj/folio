//! One-way source → preview positioning. Invisible source-range highlights let Rune give us
//! the actual text geometry on both sides (including wrapped lines, lists and code), rather
//! than assuming two scrollbars have the same percentage or document height.
use rune::{Color, Cx, ScrollPos, TextHighlight, Tx};
use rune_kit::markdown::Highlight;
use std::{cell::RefCell, ops::Range, rc::Rc};

#[derive(Clone)]
pub struct Line {
    pub range: Range<usize>,
    at: usize,
}
impl Line {
    fn source_key(&self) -> String {
        format!("sync-source-{}", self.at)
    }
    fn preview_key(&self) -> String {
        format!("sync-preview-{}", self.at)
    }
    pub fn source_highlight(&self) -> TextHighlight {
        TextHighlight::new(self.range.clone(), Color::TRANSPARENT)
            .pad(0.0)
            .key(self.source_key())
    }
    pub fn preview_highlight(&self) -> (Range<usize>, Highlight) {
        (
            self.range.clone(),
            Highlight::new()
                .bg(Color::TRANSPARENT)
                .key(self.preview_key()),
        )
    }
}

pub fn lines(source: &str, caret: usize) -> Vec<Line> {
    // Bound the extra geometry on large files, but always keep the line being edited.
    let stride = source.lines().count().div_ceil(128).max(1);
    let mut at = 0;
    let mut lines: Vec<_> = source
        .split_inclusive('\n')
        .enumerate()
        .filter_map(|(i, line)| {
            let start = at;
            at += line.len();
            if i % stride != 0 && !(start..=at).contains(&caret) {
                return None;
            }
            // One visible character is sufficient to locate a line. Highlighting the entire
            // line makes geometry lookups scan every glyph of a long, wrapped paragraph.
            let (byte, ch) = line.char_indices().find(|(_, ch)| ch.is_alphanumeric())?;
            Some(Line {
                range: start + byte..start + byte + ch.len_utf8(),
                at: start,
            })
        })
        .collect();
    // Include the actual caret's character as well as line starts. This keeps edits near
    // the end of a wrapped paragraph aligned with that visual line, not its first line.
    let caret = caret.min(source.len());
    if source.is_char_boundary(caret)
        && let Some((byte, ch)) = source[..caret]
            .char_indices()
            .rev()
            .find(|(_, ch)| ch.is_alphanumeric())
        && !lines.iter().any(|line| line.at == caret)
    {
        lines.push(Line {
            range: byte..byte + ch.len_utf8(),
            at: caret,
        });
        lines.sort_by_key(|line| line.range.start);
    }
    lines
}

struct Anchor {
    range: Range<usize>,
    source_y: f32,
    preview_y: f32,
}
#[derive(Clone, Copy)]
enum Follow {
    Scroll,
    Edit { byte: usize, laid_out: bool },
}
#[derive(Default)]
struct Position {
    source: Option<ScrollPos>,
    preview: Option<ScrollPos>,
    anchors: Vec<Anchor>,
    pending: Option<Follow>,
}

/// Kept per document, so switching tabs cannot apply another document's pending scroll.
#[derive(Clone, Default)]
pub struct SplitSync(Rc<RefCell<Position>>);
impl SplitSync {
    pub fn source_scrolled(&self, at: ScrollPos) -> bool {
        let mut p = self.0.borrow_mut();
        let moved = p
            .source
            .is_some_and(|old| (old.offset[1] - at.offset[1]).abs() > 0.01);
        p.source = Some(at);
        if moved && !matches!(p.pending, Some(Follow::Edit { .. })) {
            p.pending = Some(Follow::Scroll);
        }
        moved
    }
    pub fn preview_scrolled(&self, at: ScrollPos) {
        // Observe its extent/offset only. Preview input never schedules a source scroll.
        self.0.borrow_mut().preview = Some(at);
    }
    pub fn preview_navigated(&self) {
        self.0.borrow_mut().pending = None;
    }
    pub fn edited(&self, byte: usize) {
        self.0.borrow_mut().pending = Some(Follow::Edit {
            byte,
            laid_out: false,
        });
    }
    pub fn measure<S>(&self, lines: &[Line], cx: &Cx<S>) {
        let (Some(source), Some(preview)) =
            (cx.layout("source-scroll"), cx.layout("preview-scroll"))
        else {
            return;
        };
        let anchors = lines
            .iter()
            .filter_map(|line| {
                let a = cx.layout(&line.source_key())?;
                let b = cx.layout(&line.preview_key())?;
                Some(Anchor {
                    range: line.range.clone(),
                    source_y: a[1] - source[1],
                    preview_y: b[1] - preview[1],
                })
            })
            .collect();
        let mut p = self.0.borrow_mut();
        p.anchors = anchors;
        if let Some(Follow::Edit { laid_out, .. }) = &mut p.pending {
            *laid_out = true;
        }
    }
    pub fn apply(&self, tx: &mut Tx) {
        let mut p = self.0.borrow_mut();
        let (Some(source), Some(preview), Some(follow)) = (p.source, p.preview, p.pending) else {
            return;
        };
        if matches!(
            follow,
            Follow::Edit {
                laid_out: false,
                ..
            }
        ) {
            return;
        }
        p.pending = None;
        let target = match follow {
            Follow::Edit { byte, .. } => {
                // On an edit, put the edited line at the same vertical position as in the
                // source. Blank lines/fences fall back to the closest preceding text anchor.
                let a = p
                    .anchors
                    .iter()
                    .rev()
                    .find(|a| a.range.start <= byte)
                    .or(p.anchors.first());
                a.map_or(0.0, |a| a.preview_y - (a.source_y - source.offset[1]))
            }
            Follow::Scroll if source.offset[1] <= 0.0 => 0.0,
            Follow::Scroll if source.max[1] > 0.0 && source.offset[1] >= source.max[1] - 0.5 => {
                preview.max[1]
            }
            Follow::Scroll => {
                let y = source.offset[1];
                let mut before = (0.0, 0.0);
                let mut after = None;
                for a in &p.anchors {
                    if a.source_y <= y {
                        before = (a.source_y, a.preview_y);
                    } else {
                        after = Some((a.source_y, a.preview_y));
                        break;
                    }
                }
                let after = after.unwrap_or((
                    source.max[1] + source.viewport[1],
                    preview.max[1] + preview.viewport[1],
                ));
                let fraction = ((y - before.0) / (after.0 - before.0).max(1.0)).clamp(0.0, 1.0);
                before.1 + (after.1 - before.1) * fraction
            }
        }
        .clamp(0.0, preview.max[1]);
        if (target - preview.offset[1]).abs() > 0.5 {
            tx.scroll_to_y("preview-scroll", target);
            tx.live_only();
        }
    }
}
