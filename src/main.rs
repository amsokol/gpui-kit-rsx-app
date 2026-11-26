// Tests may panic. clippy.toml covers `unwrap`, `expect`, `panic!` and indexing;
// the rest of the no-panic lints (Cargo.toml) are allowed here, in test builds only.
#![cfg_attr(
    test,
    allow(
        clippy::disallowed_macros,
        clippy::string_slice,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable,
        reason = "tests may panic"
    )
)]

mod accent;
mod app_window;
mod platform;
mod system_font;
mod theme;
mod window_state;

use gpui::prelude::*;
use gpui::*;
use mimalloc::MiMalloc;

use crate::app_window::{AppWindow, window_options};
use crate::window_state::WindowStateFile;

/// All Rust allocations go through mimalloc.
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    // `Assets` bundles the icons components draw; without it they render blank.
    application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            // Must be called before using any GPUI Component features.
            gpui::init(cx);

            // Quit with the last window, as a desktop application does.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            // Read the system's look before opening the window, so that it starts with it.
            cx.spawn(async move |cx| {
                let dark = theme::system_is_dark(cx).await;
                let fonts = system_font::read(cx).await;
                cx.update(|cx| {
                    accent::init(cx);
                    system_font::set(cx, &fonts);
                    theme::apply(dark, cx);
                    open_main_window(cx);
                    accent::watch(cx, theme::reapply);
                    system_font::watch(cx, theme::reapply);
                });
            })
            .detach();
        });
}

fn open_main_window(cx: &mut App) {
    let state_file = WindowStateFile::system();
    let placement = state_file
        .as_ref()
        .and_then(WindowStateFile::load)
        .map(|state| state.placement(cx));
    let bounds = placement.map(|placement| placement.bounds);
    // Opens a window with a `Root` wrapping the view, so dialogs, sheets,
    // notifications and menus work in it.
    if let Err(error) = gpui::open_window(window_options(placement), cx, |window, cx| {
        cx.new(|cx| AppWindow::new(state_file, bounds, window, cx))
    }) {
        // Without a window the app is useless: report and leave, no panic.
        eprintln!("failed to open window: {error:?}");
        std::process::exit(1);
    }
    cx.activate(true);
}
