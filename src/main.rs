mod app;
mod commands;
mod document;
mod fuzzy;
#[cfg(target_os = "macos")]
mod macos;
mod outline;
mod outline_ui;
mod palette;
mod palette_ui;
mod recent;
mod session;
mod split_sync;
mod tab_switch;
mod tab_switch_ui;
mod ui;

use app::State;

fn demo_document(s: &mut State, source: &str, path: &str) {
    s.docs.clear();
    s.blank();
    let d = s.doc_mut();
    d.edit = rune::TextEdit::new(source);
    d.edit.set_caret(0);
    d.original = d.edit.text.clone();
    d.path = Some(std::path::PathBuf::from(path));
    s.mode = app::Mode::Preview;
}
fn demo(s: &mut State) {
    demo_document(
        s,
        include_str!("../examples/Welcome.md"),
        "/Documents/Welcome.md",
    );
}
fn showcase(s: &mut State) {
    demo_document(
        s,
        include_str!("../examples/Fieldnotes.md"),
        "/Documents/Fieldnotes.md",
    );
}
fn main() {
    rune::prewarm_gpu();
    rune::set_app_id("com.nickjensen.folio");
    let spec = rune::AppSpec {
        min_size: Some((720.0, 440.0)),
        families: ("Inter", "ui-monospace"),
        setup: Some(app::setup),
        poll: Some(app::poll),
        on_key: Some(app::on_key),
        scenes: &[
            ("preview", demo),
            ("split", |s| {
                demo(s);
                s.mode = app::Mode::Split;
            }),
            ("edit", |s| {
                demo(s);
                s.mode = app::Mode::Edit;
            }),
            ("showcase", showcase),
            ("showcase-split", |s| {
                showcase(s);
                s.mode = app::Mode::Split;
            }),
        ],
        ..rune::AppSpec::new("Folio", (1060.0, 780.0), State::new, ui::view)
    };
    let mut application = rune::App::new(spec)
        .chrome(rune::Chrome {
            full_size_content: true,
            traffic_lights: Some((18.0, 26.0)),
            drag_regions: true,
        })
        .keep_running()
        .modifier_keys()
        .on_modifiers(tab_switch::modifiers)
        .standard_menus()
        .on_close(|s, _| s.persist())
        .on_quit(|s, _| s.persist());
    for item in app::menu_items() {
        application = application.menu_item(item);
    }
    let handle = application.handle();
    let ready = handle.clone();
    application = application.on_ready(move |_| {
        ready.update(|s, tx| {
            s.title(tx);
            if !s.headless {
                tx.wake_after(std::time::Duration::from_secs(1));
            }
        });
    });
    #[cfg(target_os = "macos")]
    macos::install_open_handler(move |paths| {
        handle.update(move |s, tx| {
            for p in paths {
                s.open(&p);
            }
            s.title(tx);
            tx.instant();
        });
        handle.open();
    });
    application.run();
}

#[cfg(test)]
mod outline_tests;
#[cfg(test)]
mod palette_tests;
#[cfg(test)]
mod split_sync_tests;
#[cfg(test)]
mod tab_switch_tests;
#[cfg(test)]
mod ui_tests;
