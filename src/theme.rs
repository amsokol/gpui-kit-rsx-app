//! GPUI Kit's default theme in the system's light or dark mode, tinted with the system accent
//! color and set in the system UI font.
//!
//! The theme follows the system and is never saved.

use std::{
    env,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::component::{ActiveTheme as _, Theme, ThemeConfig, ThemeMode, ThemeRegistry};
use gpui::{App, AsyncApp, Window};

use crate::platform::Os;
use crate::{accent, system_font};

/// Forces `light` or `dark`, to try a mode without changing the system.
const OVERRIDE_ENV: &str = "GPUI_KIT_RSX_APP_THEME";

/// How long to wait for the system's light/dark events to settle before applying the theme.
pub(crate) const APPEARANCE_SETTLE: Duration = Duration::from_millis(50);

/// On Linux the system's light/dark mode arrives shortly after startup, and GPUI reports "light"
/// until then. Wait up to this long for "dark" before opening the window.
const APPEARANCE_WAIT: Duration = Duration::from_millis(100);
const APPEARANCE_POLL: Duration = Duration::from_millis(5);

/// Applies the light or the dark theme with the current accent color and font.
/// Call [`gpui::init`] first.
pub(crate) fn apply(dark: bool, cx: &mut App) {
    let registry = ThemeRegistry::global(cx);
    let base = if dark {
        registry.default_dark_theme()
    } else {
        registry.default_light_theme()
    };
    let mut config = ThemeConfig::clone(base);
    if let Some(accent) = accent::current(cx) {
        accent::tint(&mut config, accent);
    }
    config.font_family = Some(system_font::family(cx));
    let config = Rc::new(config);
    Theme::update(cx, |theme| theme.apply_config(&config));
}

/// Applies the theme again, after the accent color or the font changed.
pub(crate) fn reapply(cx: &mut App) {
    apply(cx.theme().mode.is_dark(), cx);
}

/// Applies the theme for the window's current system setting.
pub(crate) fn apply_system(window: &mut Window, cx: &mut App) {
    let dark = forced().unwrap_or_else(|| ThemeMode::from(window.appearance()).is_dark());
    apply(dark, cx);
}

fn parse_mode(text: &str) -> Option<bool> {
    match text.trim() {
        "dark" => Some(true),
        "light" => Some(false),
        _ => None,
    }
}

/// Whether [`OVERRIDE_ENV`] forces the dark mode, if it forces one.
fn forced() -> Option<bool> {
    env::var(OVERRIDE_ENV)
        .ok()
        .and_then(|text| parse_mode(&text))
}

/// Reads the system's light/dark mode. On Linux "light" cannot be told apart from "not yet
/// known", so a light system always waits the full [`APPEARANCE_WAIT`].
pub(crate) async fn system_is_dark(cx: &mut AsyncApp) -> bool {
    if let Some(dark) = forced() {
        return dark;
    }
    let is_dark =
        |cx: &mut AsyncApp| ThemeMode::from(cx.update(|cx| cx.window_appearance())).is_dark();

    if Os::current() == Os::Linux {
        let deadline = Instant::now() + APPEARANCE_WAIT;
        while Instant::now() < deadline {
            if is_dark(cx) {
                return true;
            }
            cx.background_executor().timer(APPEARANCE_POLL).await;
        }
    }
    is_dark(cx)
}

#[cfg(test)]
mod tests {
    use gpui::component::{ActiveTheme as _, ThemeRegistry};
    use gpui::{Hsla, TestAppContext, rgb};

    use super::{apply, parse_mode, reapply};
    use crate::accent::{self, Rgb};
    use crate::system_font;

    #[test]
    fn parses_the_forced_mode() {
        assert_eq!(parse_mode("dark"), Some(true));
        assert_eq!(parse_mode(" light "), Some(false));
        assert_eq!(parse_mode("auto"), None);
    }

    #[gpui::test]
    fn applies_both_modes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            apply(true, cx);
            assert!(cx.theme().mode.is_dark());
            apply(false, cx);
            assert!(!cx.theme().mode.is_dark());
        });
    }

    #[gpui::test]
    fn the_accent_tints_the_controls_in_both_modes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            accent::set(cx, Some(Rgb(0xE9, 0x54, 0x20)));
            let orange: Hsla = rgb(0x00E9_5420).into();
            for dark in [true, false, true] {
                apply(dark, cx);
                assert_eq!(cx.theme().primary, orange, "dark: {dark}");
                assert_eq!(cx.theme().button_primary, orange, "dark: {dark}");
                assert_eq!(cx.theme().ring, orange, "dark: {dark}");
            }
        });
    }

    #[gpui::test]
    fn losing_the_accent_restores_the_kits_colors(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            apply(false, cx);
            let own = cx.theme().primary;
            accent::set(cx, Some(Rgb(0xE9, 0x54, 0x20)));
            reapply(cx);
            assert_ne!(cx.theme().primary, own);
            accent::set(cx, None);
            reapply(cx);
            assert_eq!(cx.theme().primary, own);
        });
    }

    #[gpui::test]
    fn the_kits_registry_is_left_untinted(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            let before = ThemeRegistry::global(cx)
                .default_light_theme()
                .colors
                .primary
                .clone();
            accent::set(cx, Some(Rgb(0xE9, 0x54, 0x20)));
            apply(false, cx);
            let after = &ThemeRegistry::global(cx)
                .default_light_theme()
                .colors
                .primary;
            assert_eq!(*after, before);
        });
    }

    #[gpui::test]
    fn the_system_font_stays_through_both_modes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            system_font::set_installed(cx, "Ubuntu Sans");
            for dark in [true, false, true] {
                apply(dark, cx);
                assert_eq!(cx.theme().font_family.as_ref(), "Ubuntu Sans");
            }
        });
    }

    #[gpui::test]
    fn without_a_system_font_the_theme_uses_gpuis_choice(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            for dark in [true, false] {
                apply(dark, cx);
                assert_eq!(cx.theme().font_family.as_ref(), ".SystemUIFont");
            }
        });
    }
}
