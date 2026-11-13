use crate::app::{self, Confirm, Mode, State};
use crate::document::local_link;
use rune::{Color, Cx, El, Family, ScrollSpec, TextStyle, col, el, icon, row, text, text_input};
use rune_kit::{
    Button, ButtonVariant, Dialog, Kit, Markdown, MarkdownStyle, PaletteSeed, ScrollBar,
    SegmentedControl, TextInput, markdown::Highlight,
};

fn color(r: u8, g: u8, b: u8) -> Color {
    let decode = |v: u8| {
        let x = v as f32 / 255.0;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    Color {
        r: decode(r),
        g: decode(g),
        b: decode(b),
        a: 1.0,
    }
}
fn type_(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight)
}
fn rule(k: &Kit) -> El {
    el().w_full().h(1.0).fill(k.c.border_base).a11y_hidden()
}

pub fn kit(dark: bool) -> Kit {
    Kit::from_palette(PaletteSeed {
        bg: if dark {
            color(27, 30, 30)
        } else {
            color(252, 251, 248)
        },
        fg: if dark {
            color(227, 231, 227)
        } else {
            color(37, 46, 44)
        },
        accent: if dark {
            color(102, 207, 183)
        } else {
            color(28, 116, 100)
        },
    })
}

fn button(label: &str, action: fn(&mut State, &mut rune::Tx), k: &Kit) -> El {
    Button::new(label)
        .variant(ButtonVariant::Ghost)
        .on_press_tx(action)
        .build(k)
}

fn brand(side: f32, k: &Kit) -> El {
    el().size(side, side)
        .radius(side * 0.24)
        .gradient(color(43, 139, 119), color(17, 74, 67))
        .center()
        .child(icon(&rune_kit::glyphs::FILE_TEXT, side * 0.57).color(color(248, 246, 232)))
        .border(1.0, k.c.border_base)
}

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let dark = s.appearance.unwrap_or(cx.system_dark());
    let k = kit(dark);
    let c = &k.c;
    let d = s.doc();
    let title = col()
        .min_w(40.0)
        .grow(1.0)
        .gap(3.0)
        .window_drag()
        .child(
            text(d.title())
                .font(type_(13.0, 600.0))
                .color(c.text_base)
                .truncate(),
        )
        .child(
            text(
                d.path
                    .as_ref()
                    .and_then(|p| p.parent())
                    .map(|p| short_path(&p.to_string_lossy()))
                    .unwrap_or_else(|| "Folio".into()),
            )
            .font(type_(10.0, 400.0))
            .color(c.text_muted)
            .truncate(),
        );
    let toolbar = row()
        .key("toolbar")
        .w_full()
        .h(66.0)
        .pl(cx.titlebar_inset() + 18.0)
        .pr(18.0)
        .gap(16.0)
        .center_y()
        .window_drag()
        .child(title)
        .child(
            SegmentedControl::new(
                &["Preview", "Edit", "Split"],
                Some(match s.mode {
                    Mode::Preview => 0,
                    Mode::Edit => 1,
                    Mode::Split => 2,
                }),
            )
            .label("Document view")
            .width(Some(246.0))
            .on_pick(|s: &mut State, i, tx| {
                s.mode = [Mode::Preview, Mode::Edit, Mode::Split][i];
                if s.mode == Mode::Edit {
                    tx.focus("editor", false);
                }
                tx.instant();
            })
            .build(&k)
            .key("view-mode"),
        )
        .child(
            row()
                .gap(4.0)
                .center_y()
                .child(
                    button("Open…", app::open_dialog, &k)
                        .key("open")
                        .help("Open Markdown · ⌘O"),
                )
                .child(
                    Button::new("Save")
                        .variant(ButtonVariant::Neutral)
                        .disabled(d.path.is_some() && !d.dirty())
                        .on_press_tx(app::save)
                        .build(&k)
                        .key("save")
                        .help("Save · ⌘S"),
                ),
        );
    let mut body = col()
        .key("app")
        .w_full()
        .h_full()
        .min_h(0.0)
        .fill(c.bg_base)
        .on_key(app::on_key)
        .on_drop_files(|s: &mut State, paths, tx| {
            for p in paths {
                s.open(p);
            }
            s.title(tx);
            tx.instant();
        })
        .child(toolbar)
        .child(rule(&k));
    if s.docs.len() > 1 {
        body = body.child(tabs(s, &k)).child(rule(&k));
    }
    if s.finding {
        body = body.child(findbar(s, &k)).child(rule(&k));
    }
    if let Some(error) = &s.error {
        body = body.child(
            row()
                .w_full()
                .px(20.0)
                .py(10.0)
                .gap(12.0)
                .center_y()
                .fill(c.danger.bg)
                .child(
                    text(error)
                        .font(type_(12.0, 400.0).lh(18.0))
                        .color(c.danger.fg)
                        .wrap()
                        .grow(1.0),
                )
                .child(
                    Button::new("Dismiss")
                        .variant(ButtonVariant::Ghost)
                        .on_press(|s: &mut State| s.error = None)
                        .build(&k),
                ),
        );
    }
    let empty = d.path.is_none() && d.edit.text.is_empty() && s.mode == Mode::Preview;
    let content = if empty {
        welcome(&k)
    } else {
        let mut panes = row()
            .key(format!("document-{}", d.id))
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .min_w(0.0);
        let sync_lines = if s.mode == Mode::Split {
            crate::split_sync::lines(&d.edit.text, d.edit.focus)
        } else {
            Vec::new()
        };
        if s.mode != Mode::Preview {
            panes = panes.child(editor(s, &k, &sync_lines));
        }
        if s.mode == Mode::Split {
            panes = panes.child(el().w(1.0).h_full().fill(c.border_base));
        }
        if s.mode != Mode::Edit {
            panes = panes.child(preview(s, cx, &k, &sync_lines));
        }
        if s.mode == Mode::Split {
            d.split_sync.measure(&sync_lines, cx);
        }
        panes
    };
    let count = d.edit.text.split_whitespace().count();
    let lines = d.edit.text.lines().count().max(1);
    let status = row()
        .w_full()
        .h(30.0)
        .px(20.0)
        .gap(16.0)
        .center_y()
        .child(
            text(format!("{count} words  ·  {lines} lines"))
                .font(type_(10.0, 400.0))
                .color(c.text_muted),
        )
        .child(el().grow(1.0))
        .child(
            text(if d.dirty() { "Edited" } else { "Markdown" })
                .font(type_(10.0, 500.0))
                .color(if d.dirty() {
                    c.text_accent
                } else {
                    c.text_muted
                }),
        )
        .child(
            text(format!("{}%", (s.zoom * 100.0).round() as u32))
                .font(type_(10.0, 400.0))
                .color(c.text_muted),
        );
    body = body.child(content).child(rule(&k)).child(status);
    let mut layer = Vec::new();
    if let Some(confirm) = s.confirm {
        let (id, reverting) = match confirm {
            Confirm::Close(id) => (id, false),
            Confirm::Revert(id) => (id, true),
        };
        let name = s
            .docs
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.title())
            .unwrap_or_default();
        let mut actions = row().w_full().padding(16.0).gap(8.0).center_y();
        if reverting {
            actions = actions
                .child(el().grow(1.0))
                .child(
                    Button::new("Cancel")
                        .on_press(|s: &mut State| s.confirm = None)
                        .build(&k)
                        .autofocus(),
                )
                .child(
                    Button::new("Revert")
                        .variant(ButtonVariant::Danger)
                        .on_press_tx(app::revert)
                        .build(&k),
                );
        } else {
            actions = actions
                .child(
                    Button::new("Don’t Save")
                        .variant(ButtonVariant::Ghost)
                        .on_press_tx(move |s: &mut State, tx| s.close(id, tx))
                        .build(&k),
                )
                .child(el().grow(1.0))
                .child(
                    Button::new("Cancel")
                        .on_press(|s: &mut State| s.confirm = None)
                        .build(&k)
                        .autofocus(),
                )
                .child(
                    Button::new("Save")
                        .variant(ButtonVariant::Contrast)
                        .on_press_tx(move |s: &mut State, tx| {
                            if let Some(i) = s.docs.iter().position(|d| d.id == id) {
                                s.active = i;
                                app::save_document(s, tx, false, true);
                            }
                        })
                        .build(&k),
                );
        }
        Dialog::new(
            "confirm",
            if reverting {
                format!("Revert “{name}”?")
            } else {
                format!("Save changes to “{name}”?")
            },
        )
        .description(if reverting {
            "Your edits will be replaced by the file on disk."
        } else {
            "Unsaved changes will be lost if you don’t save."
        })
        .open(true)
        .fit()
        .click_outside(false)
        .on_close(|s: &mut State| s.confirm = None)
        .child(actions)
        .build(&k, &mut layer);
    }
    let height = cx.layout_size("app").map(|size| size[1]).unwrap_or(780.0);
    if s.confirm.is_none() {
        crate::palette_ui::build(s, &k, height, &mut layer);
        crate::tab_switch_ui::build(s, &k, height, &mut layer);
    }
    rune_kit::root(body, layer).on_window_focus(|s: &mut State, focused, tx| {
        if !focused && s.tab_switch.is_open() {
            crate::tab_switch::cancel(s, tx);
        }
    })
}

fn tabs(s: &State, k: &Kit) -> El {
    let mut tabs = row().w_full().min_h(36.0).scroll_x();
    for (i, d) in s.docs.iter().enumerate() {
        let id = d.id;
        let tab = row()
            .key(format!("tab-{id}"))
            .h(36.0)
            .px(12.0)
            .gap(12.0)
            .center_y()
            .fill(if s.active == i {
                k.c.layer_01
            } else {
                k.c.bg_base
            })
            .border_sides(
                [0.0, 1.0, if s.active == i { 2.0 } else { 0.0 }, 0.0],
                if s.active == i {
                    k.c.text_accent
                } else {
                    k.c.border_base
                },
            )
            .child(
                el().role("tab")
                    .label(d.title())
                    .focus_selected(s.active == i)
                    .focusable(true)
                    .hit()
                    .on_press_tx(move |s: &mut State, tx| s.select(i, tx))
                    .child(
                        text(format!(
                            "{}{}",
                            d.title(),
                            if d.dirty() { " •" } else { "" }
                        ))
                        .font(type_(11.0, 500.0))
                        .color(k.c.text_base),
                    ),
            )
            .child(
                Button::new("×")
                    .variant(ButtonVariant::Ghost)
                    .on_press_tx(move |s: &mut State, tx| {
                        s.active = i;
                        if s.doc().dirty() {
                            s.confirm = Some(Confirm::Close(id));
                        } else {
                            s.close(id, tx);
                        }
                    })
                    .build(k)
                    .help("Close document"),
            );
        tabs = tabs.child(tab);
    }
    tabs
}

fn editor(s: &State, k: &Kit, sync_lines: &[crate::split_sync::Line]) -> El {
    let sync = s.doc().split_sync.clone();
    let wake = s.wake.clone();
    let split = s.mode == Mode::Split;
    let font = TextStyle::new(13.0 * s.zoom, 400.0)
        .lh(23.0 * s.zoom)
        .family(Family::Mono);
    let field = text_input(&s.doc().edit, font)
        .key("editor")
        .label("Markdown source")
        .w_full()
        .min_h(120.0)
        .color(k.c.text_base)
        .caret(k.c.text_accent, k.selection())
        .placeholder("# Start writing…", k.c.text_faint)
        .wrap_anywhere()
        .text_highlights(sync_lines.iter().map(|line| line.source_highlight()))
        .on_edit(|s: &mut State| &mut s.doc_mut().edit)
        .on_input_key(|s: &mut State, e, tx| {
            crate::tab_switch::key(s, e, tx) || crate::palette::shortcut(s, e, tx)
        })
        .on_input_change(|s: &mut State, tx| {
            s.changed(tx);
            if s.mode == Mode::Split {
                s.doc().split_sync.edited(s.doc().edit.focus);
                tx.wake_after(std::time::Duration::ZERO);
            }
        });
    col()
        .key("source-pane")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .h_full()
        .fill(k.c.layer_01)
        .child(
            col()
                .key("source-scroll")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .on_scroll_offset(move |at| {
                    if split
                        && sync.source_scrolled(at)
                        && let Some(wake) = &wake
                    {
                        wake();
                    }
                })
                .child(col().w_full().padding(32.0).child(field))
                .scroll_bar(k),
        )
}

fn preview(s: &State, cx: &mut Cx<State>, k: &Kit, sync_lines: &[crate::split_sync::Line]) -> El {
    let sync = s.doc().split_sync.clone();
    let split = s.mode == Mode::Split;
    let width = cx
        .layout_size("preview-scroll")
        .map(|v| v[0] - 80.0)
        .unwrap_or(720.0)
        .clamp(200.0, 760.0);
    let z = s.zoom;
    let style = MarkdownStyle::from_kit(k)
        .body_font(type_(15.0 * z, 400.0).lh(26.0 * z))
        .heading_font(1, type_(32.0 * z, 650.0).lh(42.0 * z))
        .heading_font(2, type_(23.0 * z, 600.0).lh(34.0 * z))
        .heading_font(3, type_(18.0 * z, 600.0).lh(28.0 * z))
        .code_block_font(type_(12.0 * z, 400.0).lh(21.0 * z).family(Family::Mono))
        .text_width(width)
        .link(k.c.text_accent)
        .code_block(k.c.layer_01, k.c.border_base)
        .code_block_padding(18.0, 16.0);
    let task_source = s.doc().edit.text.clone();
    let task_id = s.doc().id;
    let mut md = Markdown::new(&s.doc().edit.text)
        .selectable(true)
        .scroll_wide(true)
        .style(style)
        .on_link_tx(|s: &mut State, url, tx| s.link(url, tx))
        .on_task_tx(move |s: &mut State, _, offset, checked, tx| {
            if s.doc().id == task_id
                && s.doc().edit.text == task_source
                && task_source.as_bytes().get(offset) == Some(&b'[')
            {
                s.doc_mut().edit.set_selection(offset + 1, offset + 2);
                s.doc_mut().edit.insert(if checked { "x" } else { " " });
                s.changed(tx);
            }
        });
    let mut highlights: Vec<_> = if s.finding {
        s.matches
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, ranges)| {
                (
                    ranges,
                    if i == s.match_index {
                        Highlight::current()
                    } else {
                        Highlight::new()
                    },
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    highlights.extend(sync_lines.iter().map(|line| {
        let (range, highlight) = line.preview_highlight();
        (vec![range], highlight)
    }));
    md = md.highlights(highlights);
    if let Some(cache) = s.cache.clone() {
        let base = s.doc().path.clone();
        let images = s.images.clone();
        let scale = cx.env().scale;
        md = md.images(move |src| {
            let path = local_link(base.as_deref(), src)?;
            let mut images = images.borrow_mut();
            if images.len() >= 64 && !images.contains_key(&path) {
                return None;
            }
            Some(
                images
                    .entry(path.clone())
                    .or_insert_with(|| cache.file(path, 760.0, scale))
                    .clone(),
            )
        });
    }
    let outline = s.doc().outline.clone();
    let wake = s.wake.clone();
    let mut pane = col()
        .key("preview-pane")
        .grow(1.0)
        .h_full()
        .min_h(0.0)
        .min_w(0.0)
        .child(
            col()
                .key("preview-scroll")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .scroll_anchor()
                .on_scroll_offset(move |at| {
                    if split {
                        sync.preview_scrolled(at);
                    }
                    if outline.preview_scrolled(at)
                        && let Some(wake) = &wake
                    {
                        wake();
                    }
                })
                .child(
                    col().w_full().px(40.0).pt(44.0).pb(90.0).child(
                        md.build(k)
                            .key("markdown")
                            .w_full()
                            .max_w(760.0)
                            .self_center(),
                    ),
                )
                .scroll_bar(k),
        );
    if let Some(outline) = crate::outline_ui::build(s, cx, k) {
        pane = pane.child(outline);
    }
    pane.clip_rounded(0.0)
}

fn findbar(s: &State, k: &Kit) -> El {
    row()
        .w_full()
        .h(44.0)
        .px(16.0)
        .gap(10.0)
        .center_y()
        .child(
            TextInput::new(&s.search)
                .label("Find in document")
                .placeholder("Find in document")
                .width(Some(260.0))
                .leading_icon(k.glyph.search)
                .on_edit(|s: &mut State| &mut s.search)
                .on_change(|s: &mut State, tx| {
                    s.match_index = 0;
                    s.refresh_search();
                    s.show_match(tx);
                })
                .on_submit(|s: &mut State, tx| s.find_step(false, tx))
                .build(k)
                .key("find-field"),
        )
        .child(
            text(if s.matches.is_empty() {
                if s.search.text.is_empty() {
                    String::new()
                } else {
                    "No matches".into()
                }
            } else {
                format!("{} of {}", s.match_index + 1, s.matches.len())
            })
            .font(type_(11.0, 400.0))
            .color(k.c.text_muted),
        )
        .child(button("Previous", |s, tx| s.find_step(true, tx), k))
        .child(button("Next", |s, tx| s.find_step(false, tx), k))
        .child(el().grow(1.0))
        .child(button(
            "Done",
            |s, _| {
                s.finding = false;
                s.search.replace_text("");
                s.refresh_search();
            },
            k,
        ))
}

fn welcome(k: &Kit) -> El {
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(18.0)
        .child(brand(76.0, k))
        .child(text("Folio").font(type_(30.0, 600.0)).color(k.c.text_base))
        .child(
            text("Open a Markdown file")
                .font(type_(14.0, 400.0))
                .color(k.c.text_muted),
        )
        .child(
            row()
                .gap(10.0)
                .mt(6.0)
                .child(
                    Button::new("New Document")
                        .variant(ButtonVariant::Neutral)
                        .on_press_tx(app::new_document)
                        .build(k)
                        .key("new-document"),
                )
                .child(
                    Button::new("Open…")
                        .variant(ButtonVariant::Contrast)
                        .on_press_tx(app::open_dialog)
                        .build(k),
                ),
        )
}

fn short_path(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME")
        && let Some(rest) = path.strip_prefix(&home)
    {
        return format!("~{rest}");
    }
    path.to_owned()
}
