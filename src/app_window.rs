//! The main window: the title bar over the content.

use std::time::Duration;

use gpui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, TITLE_BAR_HEIGHT, TitleBar,
    button::{Button, ButtonVariants as _},
    label::Label,
};
use gpui::prelude::*;
use gpui::*;
use gpui_rsx::{rsx, rsx_listener};

use crate::platform::{self, Os, TitleAlignment};
use crate::theme;
use crate::window_state::{
    MIN_SIZE, Placement, Unapplied, WindowState, WindowStateFile, display_key, remember, unapplied,
};

const TITLE: &str = "GPUI Kit RSX App";

/// The most window buttons a title bar has on one side: minimize, maximize and close. Each
/// is as wide as the title bar is high.
const MAX_WINDOW_BUTTONS: f32 = 3.;

/// How long after opening to check that the window got the requested size and state.
const RESTORE_SETTLE: Duration = Duration::from_millis(300);

/// The id desktops know the app by: on Linux it ties the window to its launcher.
const APP_ID: &str = "gpui-kit-rsx-app";

/// The options of the main window, opened at `placement` when there is a saved one.
pub(crate) fn window_options(placement: Option<Placement>) -> WindowOptions {
    WindowOptions {
        window_bounds: placement.map(|placement| placement.bounds),
        display_id: placement.and_then(|placement| placement.display),
        window_min_size: Some(MIN_SIZE),
        titlebar: Some(TitlebarOptions {
            // Shown by the task bar, the Dock and the window switcher.
            title: Some(TITLE.into()),
            ..TitleBar::title_bar_options()
        }),
        // Linux only. GPUI asks for the server's decorations otherwise, which KDE grants.
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some(APP_ID.into()),
        ..TitleBar::window_options()
    }
}

pub(crate) struct AppWindow {
    clicks: usize,
    window_bounds: WindowBounds,
    /// The saved key of the display the window is on.
    display: Option<String>,
    /// While the window is being restored, what it reports is not what the user chose.
    restoring: bool,
    appearance_event: u64,
    state_file: Option<WindowStateFile>,
    _subscriptions: Vec<Subscription>,
}

impl AppWindow {
    /// Follows the system's light/dark mode and, with a `state_file`, saves the window geometry
    /// to it when the window closes or the app quits. `requested` are the bounds the window
    /// was opened with.
    pub(crate) fn new(
        state_file: Option<WindowStateFile>,
        requested: Option<WindowBounds>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // GPUI may report a wrong "light" first and the real mode a moment later, and applying
        // the wrong one would flash the window. Wait for the events to settle.
        let appearance = cx.observe_window_appearance(window, |this, window, cx| {
            this.appearance_event = this.appearance_event.wrapping_add(1);
            let event = this.appearance_event;
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(theme::APPEARANCE_SETTLE)
                    .await;
                // This fails only when the window has closed, and then there is no theme to set.
                this.update_in(cx, |this, window, cx| {
                    if this.appearance_event == event {
                        theme::apply_system(window, cx);
                    }
                })
                .ok();
            })
            .detach();
        });

        // Keep the latest bounds: the window is gone by the time they are saved.
        let bounds = cx.observe_window_bounds(window, |this, window, cx| {
            this.record_bounds(window, cx);
        });
        let release = cx.on_release(|this, _| this.save_window_state());
        // Quitting does not close the windows first (the Dock's "Quit" on macOS).
        let quit = cx.on_app_quit(|this, _| {
            this.save_window_state();
            async {}
        });

        // Only Linux needs this: elsewhere the window opens as asked.
        let restore = requested.filter(|_| Os::current() == Os::Linux);
        if let Some(requested) = restore {
            Self::finish_restore(requested, window, cx);
        }

        Self {
            clicks: 0,
            window_bounds: requested.unwrap_or_else(|| window.window_bounds()),
            display: current_display(window, cx),
            restoring: restore.is_some(),
            appearance_event: 0,
            state_file,
            _subscriptions: vec![appearance, bounds, release, quit],
        }
    }

    /// Asks again, once the window is shown, for what the system did not apply. Bounds are
    /// recorded again after that has had the time to take effect too.
    fn finish_restore(requested: WindowBounds, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(RESTORE_SETTLE).await;
            // Both updates fail only when the window has closed already.
            cx.update(|window, _| {
                match unapplied(requested, window.is_maximized(), window.bounds().size) {
                    Some(Unapplied::Maximize) => window.zoom_window(),
                    Some(Unapplied::Resize(size)) => window.resize(size),
                    None => {}
                }
            })
            .ok();
            cx.background_executor().timer(RESTORE_SETTLE).await;
            this.update_in(cx, |this, window, cx| {
                this.restoring = false;
                this.record_bounds(window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn record_bounds(&mut self, window: &Window, cx: &App) {
        if !self.restoring {
            self.window_bounds = remember(self.window_bounds, window.window_bounds());
            self.display = current_display(window, cx);
        }
    }

    fn save_window_state(&self) {
        if let Some(file) = &self.state_file {
            file.save(&WindowState::from_window_bounds(
                self.window_bounds,
                self.display.clone(),
            ));
        }
    }
}

/// The saved key of the display the window is on.
fn current_display(window: &Window, cx: &App) -> Option<String> {
    window
        .display(cx)
        .and_then(|display| display_key(display.as_ref()))
}

/// The name of the app as the system's title bars show it, with the icon where they have one.
fn title(os: Os, cx: &App) -> Div {
    let icon = platform::title_has_icon(os).then(|| title_icon(cx));
    let weight = if platform::title_is_bold(os) {
        FontWeight::SEMIBOLD
    } else {
        FontWeight::NORMAL
    };
    rsx! {
        <div class="flex items-center min-w-0 text-sm" gap_2
            debug_selector={|| TITLE_SELECTOR.to_owned()}>
            {...icon}
            <div class="min-w-0 truncate" font_weight={weight}>{TITLE}</div>
        </div>
    }
}

/// The app's icon, in the accent color.
fn title_icon(cx: &App) -> Div {
    let icon = Icon::new(IconName::GalleryVerticalEnd)
        .small()
        .text_color(cx.theme().primary);
    rsx! {
        <div class="flex-shrink-0" debug_selector={|| ICON_SELECTOR.to_owned()}>{icon}</div>
    }
}

/// How tests find [`title`] and [`title_icon`].
const TITLE_SELECTOR: &str = "window-title";
const ICON_SELECTOR: &str = "window-title-icon";

/// GPUI Kit's title bar with the title as the system shows it.
fn title_bar(os: Os, cx: &App) -> AnyElement {
    match platform::title_alignment(os) {
        TitleAlignment::Start => TitleBar::new()
            .child(
                rsx! { <div class="flex-1 h-full flex items-center min-w-0">{title(os, cx)}</div> },
            )
            .into_any_element(),
        // A child of the title bar only gets the room between its padding and the window
        // buttons, which is not centered in the window. So the title is laid over the whole
        // bar. The layer takes no mouse events: dragging the title still moves the window.
        // Both sides keep clear of the window buttons, whichever side the desktop uses.
        TitleAlignment::Center => {
            let buttons = TITLE_BAR_HEIGHT * MAX_WINDOW_BUTTONS;
            rsx! {
                <div class="relative flex-shrink-0">
                    {TitleBar::new()}
                    <div class="absolute top-0 left-0 w-full flex items-center justify-center"
                        h={TITLE_BAR_HEIGHT} px={buttons}>
                        {title(os, cx)}
                    </div>
                </div>
            }
            .into_any_element()
        }
    }
}

impl Render for AppWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let os = Os::current();
        let title_bar = platform::draws_own_title_bar(os, window.window_decorations())
            .then(|| title_bar(os, cx));
        rsx! {
            <div class="size-full flex flex-col"
                bg={cx.theme().background} text_color={cx.theme().foreground}>
                {...title_bar}
                <div class="flex-1 flex flex-col items-center justify-center" gap_2>
                    <Label(format!("Clicked {} times", self.clicks)) />
                    <Button("ok") primary icon={IconName::Check} label="Let's Go!"
                        onClick={rsx_listener!(cx, notify, |view, _, _, _| {
                            view.clicks += 1;
                        })} />
                </div>
            </div>
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::component::Root;
    use gpui::{
        AppContext as _, Context, IntoElement, Modifiers, MouseButton, ParentElement as _, Pixels,
        Render, Styled as _, TestAppContext, VisualTestContext, Window, WindowBounds,
        WindowOptions, div, point, px, size,
    };

    use super::{ICON_SELECTOR, TITLE_SELECTOR, title_bar};
    use crate::platform::Os;

    struct Bar(Os);

    impl Render for Bar {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(title_bar(self.0, cx))
        }
    }

    /// A drawn window of this width with only the title bar of `os` in it.
    fn open(cx: &mut TestAppContext, os: Os, width: Pixels) -> VisualTestContext {
        cx.update(gpui::init);
        let window = cx.update(|cx| {
            let bounds = gpui::Bounds::new(point(px(0.), px(0.)), size(width, px(300.)));
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            };
            cx.open_window(options, |window, cx| {
                let view = cx.new(|_| Bar(os));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .unwrap()
        });
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx
    }

    #[gpui::test]
    fn a_centered_title_is_centered_in_the_window(cx: &mut TestAppContext) {
        let mut cx = open(cx, Os::Linux, px(700.));
        let window_width = cx.update(|window, _| window.viewport_size().width);
        let title = cx.debug_bounds(TITLE_SELECTOR).expect("the title is drawn");
        let off_center = f32::from(title.center().x - window_width / 2.);
        assert!(off_center.abs() <= 1., "off center by {off_center}px");
    }

    #[gpui::test]
    fn a_narrow_window_keeps_the_title_clear_of_the_buttons(cx: &mut TestAppContext) {
        let mut cx = open(cx, Os::Linux, px(320.));
        let title = cx.debug_bounds(TITLE_SELECTOR).expect("the title is drawn");
        assert!(title.left() >= px(102.), "starts at {:?}", title.left());
        assert!(title.right() <= px(218.), "ends at {:?}", title.right());
    }

    #[gpui::test]
    fn the_windows_title_has_the_icon(cx: &mut TestAppContext) {
        let mut cx = open(cx, Os::Windows, px(700.));
        assert!(cx.debug_bounds(ICON_SELECTOR).is_some());
    }

    #[gpui::test]
    fn the_linux_title_has_no_icon(cx: &mut TestAppContext) {
        let mut cx = open(cx, Os::Linux, px(700.));
        assert!(cx.debug_bounds(ICON_SELECTOR).is_none());
    }

    #[gpui::test]
    fn the_macos_title_has_no_icon(cx: &mut TestAppContext) {
        let mut cx = open(cx, Os::Mac, px(700.));
        assert!(cx.debug_bounds(ICON_SELECTOR).is_none());
    }

    #[gpui::test]
    fn a_title_at_the_start_stays_there(cx: &mut TestAppContext) {
        let mut cx = open(cx, Os::Windows, px(700.));
        let title = cx.debug_bounds(TITLE_SELECTOR).expect("the title is drawn");
        assert!(title.left() < px(100.), "starts at {:?}", title.left());
    }

    /// The test platform cannot move windows: reaching `start_window_move` panics, which
    /// shows that a drag on the title gets through to the title bar.
    #[gpui::test]
    #[should_panic(expected = "not implemented")]
    fn dragging_the_centered_title_moves_the_window(cx: &mut TestAppContext) {
        let mut cx = open(cx, Os::Linux, px(700.));
        let at = cx
            .debug_bounds(TITLE_SELECTOR)
            .expect("the title is drawn")
            .center();
        let moved = point(at.x + px(1.), at.y);
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(moved, Some(MouseButton::Left), Modifiers::default());
    }
}
