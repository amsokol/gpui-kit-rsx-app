//! The system accent color, used to tint the controls.
//!
//! The color comes from the `mundy` crate: the XDG settings portal on Linux (with a Yaru theme
//! fallback on Ubuntu), `UISettings` on Windows and `NSColor.controlAccentColor` on macOS. When
//! the system reports none, the theme keeps its own colors.

use std::{env, time::Duration};

use futures_lite::StreamExt as _;
use gpui::component::ThemeConfig;
use gpui::{App, Global};
use mundy::{Interest, Preferences};

/// Forces an accent color, e.g. `#e95420`, to try the tinting without changing the system.
const OVERRIDE_ENV: &str = "GPUI_KIT_RSX_APP_ACCENT";

/// How long to wait for the system at startup. The window opens afterwards, so keep it short.
const READ_TIMEOUT: Duration = Duration::from_millis(200);

/// Below this contrast ratio against white, text on the accent color turns black.
const MIN_CONTRAST_WITH_WHITE: f64 = 3.0;

const WHITE: Rgb = Rgb(255, 255, 255);
const BLACK: Rgb = Rgb(0, 0, 0);

/// An sRGB color with 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rgb(pub(crate) u8, pub(crate) u8, pub(crate) u8);

impl Rgb {
    /// Parses `#rrggbb` or `rrggbb`.
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let digits = text.strip_prefix('#').unwrap_or(text);
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let [_, red, green, blue] = u32::from_str_radix(digits, 16).ok()?.to_be_bytes();
        Some(Self(red, green, blue))
    }

    fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// Hex with an alpha channel, for translucent highlights.
    fn hex_alpha(self, alpha: u8) -> String {
        format!("{}{alpha:02x}", self.hex())
    }

    /// This color with `percent` (0 to 100) of `other` mixed into it.
    fn mix(self, other: Self, percent: u8) -> Self {
        let percent = u32::from(percent.min(100));
        let channel = |own: u8, theirs: u8| {
            let mixed = u32::from(own) * (100 - percent) + u32::from(theirs) * percent;
            u8::try_from((mixed + 50) / 100).unwrap_or(u8::MAX)
        };
        Self(
            channel(self.0, other.0),
            channel(self.1, other.1),
            channel(self.2, other.2),
        )
    }

    /// WCAG relative luminance.
    fn luminance(self) -> f64 {
        let linear = |channel: u8| {
            let value = f64::from(channel) / 255.;
            if value <= 0.040_45 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.0) + 0.7152 * linear(self.1) + 0.0722 * linear(self.2)
    }

    /// White text on the color, or black when the color is too light for white to read.
    fn foreground(self) -> Self {
        if 1.05 / (self.luminance() + 0.05) >= MIN_CONTRAST_WITH_WHITE {
            WHITE
        } else {
            BLACK
        }
    }
}

impl From<mundy::Srgba> for Rgb {
    fn from(color: mundy::Srgba) -> Self {
        let [red, green, blue, _] = color.to_u8_array();
        Self(red, green, blue)
    }
}

/// The accent color last reported by the system.
#[derive(Default)]
struct SystemAccent(Option<Rgb>);

impl Global for SystemAccent {}

/// The accent color to tint the theme with, if there is one.
pub(crate) fn current(cx: &App) -> Option<Rgb> {
    cx.try_global::<SystemAccent>().and_then(|accent| accent.0)
}

/// Stores the accent color that [`current`] reports.
pub(crate) fn set(cx: &mut App, accent: Option<Rgb>) {
    cx.set_global(SystemAccent(accent));
}

/// The color forced through [`OVERRIDE_ENV`], if it is set to a valid color.
fn forced() -> Option<Rgb> {
    env::var(OVERRIDE_ENV)
        .ok()
        .and_then(|text| Rgb::parse(&text))
}

/// Reads the accent color once and stores it. Call before opening the window, on the main thread.
pub(crate) fn init(cx: &mut App) {
    let accent = forced().or_else(|| {
        Preferences::once_blocking(Interest::AccentColor, READ_TIMEOUT)
            .and_then(|preferences| preferences.accent_color.0)
            .map(Rgb::from)
    });
    set(cx, accent);
}

/// Follows accent color changes and calls `on_change` after each one.
pub(crate) fn watch(cx: &mut App, on_change: impl Fn(&mut App) + 'static) {
    if forced().is_some() {
        return;
    }
    cx.spawn(async move |cx| {
        let mut stream = Preferences::stream(Interest::AccentColor);
        while let Some(preferences) = stream.next().await {
            let accent = preferences.accent_color.0.map(Rgb::from);
            cx.update(|cx| {
                if current(cx) != accent {
                    set(cx, accent);
                    on_change(cx);
                }
            });
        }
    })
    .detach();
}

/// Paints every accented color of `theme` with `accent`.
///
/// The `accent.*` colors of the theme are left alone: in GPUI Kit they are hover backgrounds,
/// not the system accent.
pub(crate) fn tint(theme: &mut ThemeConfig, accent: Rgb) {
    // A hovered control moves toward the page: lighter on a light theme, darker on a dark one.
    // A pressed one is darker on both.
    let hover = if theme.mode.is_dark() {
        accent.mix(BLACK, 10)
    } else {
        accent.mix(WHITE, 10)
    };
    let active = accent.mix(BLACK, 20);

    let solid = Some(accent.hex().into());
    let on_accent = Some(accent.foreground().hex().into());
    let colors = &mut theme.colors;

    colors.primary.clone_from(&solid);
    colors.primary_hover = Some(hover.hex().into());
    colors.primary_active = Some(active.hex().into());
    colors.primary_foreground.clone_from(&on_accent);

    colors.ring.clone_from(&solid);
    colors.caret.clone_from(&solid);
    colors.link.clone_from(&solid);
    colors.link_hover = Some(hover.hex().into());
    colors.link_active = Some(active.hex().into());

    colors.slider_bar.clone_from(&solid);
    colors.progress_bar.clone_from(&solid);
    colors.drag_border.clone_from(&solid);
    colors.drop_target = Some(accent.hex_alpha(0x33).into());

    colors.selection = Some(accent.hex_alpha(0x66).into());
    colors.list_active = Some(accent.hex_alpha(0x26).into());
    colors.list_active_border.clone_from(&solid);
    colors.table_active = Some(accent.hex_alpha(0x26).into());
    colors.table_active_border.clone_from(&solid);

    colors.sidebar_primary = solid;
    colors.sidebar_primary_foreground = on_accent;
}

#[cfg(test)]
mod tests {
    use gpui::component::{ThemeConfig, ThemeMode};

    use super::{BLACK, Rgb, WHITE, tint};

    const ORANGE: Rgb = Rgb(0xE9, 0x54, 0x20);

    fn tinted(mode: ThemeMode) -> ThemeConfig {
        // `ThemeConfig` is non-exhaustive: no struct expression.
        let mut theme = ThemeConfig::default();
        theme.mode = mode;
        tint(&mut theme, ORANGE);
        theme
    }

    #[test]
    fn converts_srgba_to_bytes() {
        let color = mundy::Srgba {
            red: 1.,
            green: 0.5,
            blue: 0.,
            alpha: 1.,
        };
        assert_eq!(Rgb::from(color), Rgb(255, 128, 0));
    }

    #[test]
    fn parses_hex() {
        assert_eq!(Rgb::parse("#e95420"), Some(ORANGE));
        assert_eq!(Rgb::parse(" 0060DE "), Some(Rgb(0x00, 0x60, 0xDE)));
        assert_eq!(Rgb::parse("#e954"), None);
        assert_eq!(Rgb::parse("#gg0000"), None);
        assert_eq!(Rgb::parse("+e9542"), None);
        assert_eq!(Rgb::parse("ééé"), None);
    }

    #[test]
    fn formats_hex() {
        assert_eq!(ORANGE.hex(), "#e95420");
        assert_eq!(ORANGE.hex_alpha(0x15), "#e9542015");
    }

    #[test]
    fn mixes_colors() {
        assert_eq!(ORANGE.mix(WHITE, 0), ORANGE);
        assert_eq!(ORANGE.mix(WHITE, 100), WHITE);
        assert_eq!(WHITE.mix(BLACK, 50), Rgb(128, 128, 128));
        assert_eq!(WHITE.mix(BLACK, 200), BLACK);
    }

    #[test]
    fn picks_a_readable_foreground() {
        // Ubuntu orange and macOS blue take white text, a light yellow takes black.
        assert_eq!(ORANGE.foreground(), WHITE);
        assert_eq!(Rgb(0x00, 0x60, 0xDE).foreground(), WHITE);
        assert_eq!(Rgb(0xF5, 0xC2, 0x11).foreground(), BLACK);
    }

    #[test]
    fn tints_the_accented_colors() {
        let theme = tinted(ThemeMode::Light);
        assert_eq!(theme.colors.primary.as_deref(), Some("#e95420"));
        assert_eq!(theme.colors.primary_foreground.as_deref(), Some("#ffffff"));
        assert_eq!(theme.colors.ring.as_deref(), Some("#e95420"));
        assert_eq!(theme.colors.selection.as_deref(), Some("#e9542066"));
        assert_eq!(theme.colors.list_active.as_deref(), Some("#e9542026"));
        // The kit's hover background keeps its own color.
        assert_eq!(theme.colors.accent, None);
    }

    #[test]
    fn hover_is_lighter_on_light_and_darker_on_dark() {
        let light = tinted(ThemeMode::Light);
        let dark = tinted(ThemeMode::Dark);
        assert_eq!(
            light.colors.primary_hover.as_deref(),
            Some(ORANGE.mix(WHITE, 10).hex().as_str())
        );
        assert_eq!(
            dark.colors.primary_hover.as_deref(),
            Some(ORANGE.mix(BLACK, 10).hex().as_str())
        );
        assert_eq!(light.colors.primary_active, dark.colors.primary_active);
    }
}
