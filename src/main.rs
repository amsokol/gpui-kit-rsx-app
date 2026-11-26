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

use gpui::component::{
    ActiveTheme as _, IconName,
    button::{Button, ButtonVariants as _},
    label::Label,
};
use gpui::prelude::*;
use gpui::*;
use gpui_rsx::{rsx, rsx_listener};

#[derive(Default)]
struct HelloWorld {
    clicks: usize,
}

impl Render for HelloWorld {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        rsx! {
            <div class="size-full flex flex-col gap-[8px] items-center justify-center"
                bg={cx.theme().background} text_color={cx.theme().foreground}>
                <Label(format!("Clicked {} times", self.clicks)) />
                <Button("ok") primary icon={IconName::Check} label="Let's Go!"
                    onClick={rsx_listener!(cx, notify, |view, _, _, _| {
                        view.clicks += 1;
                    })} />
            </div>
        }
    }
}

fn main() {
    // `Assets` bundles the icons components draw; without it they render blank.
    application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            // Must be called before using any GPUI Component features.
            gpui::init(cx);

            // Opens a window with a `Root` wrapping the view, so dialogs, sheets,
            // notifications and menus work in it.
            if let Err(error) = gpui::open_window(WindowOptions::default(), cx, |_, cx| {
                cx.new(|_| HelloWorld::default())
            }) {
                // Without a window the app is useless: report and leave, no panic.
                eprintln!("failed to open window: {error:?}");
                std::process::exit(1);
            }
            cx.activate(true);
        });
}
