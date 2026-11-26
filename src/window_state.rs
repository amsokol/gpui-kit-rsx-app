//! Saves and restores the window's size, position, display and maximized state.
//!
//! The file is `window.json` in the per-user local config directory:
//!
//! - Linux: `~/.config/gpui-kit-rsx-app/` (`$XDG_CONFIG_HOME` is honored)
//! - macOS: `~/Library/Application Support/gpui-kit-rsx-app/`
//! - Windows: `%LOCALAPPDATA%\gpui-kit-rsx-app\`
//!
//! Wayland compositors choose where a window goes, so there only the size and the maximized
//! state come back.

use std::{fs, io, path::PathBuf};

use gpui::{App, Bounds, DisplayId, Pixels, PlatformDisplay, Size, WindowBounds, point, px, size};
use serde::{Deserialize, Serialize};

const APP_DIR: &str = "gpui-kit-rsx-app";
const FILE_NAME: &str = "window.json";

/// The smallest window: the limit while resizing, and for a size read from the file.
pub(crate) const MIN_SIZE: Size<Pixels> = Size {
    width: px(320.),
    height: px(240.),
};

/// The window geometry that survives a restart, in logical pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct WindowState {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    maximized: bool,
    /// The [`display_key`] of the display the window was on. macOS gives `x` and `y`
    /// relative to it, and Windows takes them only together with it.
    #[serde(default)]
    display: Option<String>,
}

/// An identifier of a display that lasts across restarts, when the system has one. X11 gives
/// every screen the same empty one.
pub(crate) fn display_key(display: &dyn PlatformDisplay) -> Option<String> {
    let uuid = display.uuid().ok()?;
    (!uuid.is_nil()).then(|| uuid.to_string())
}

/// A connected display, as far as placing a window needs to know it.
#[derive(Debug, Clone)]
struct Screen {
    key: Option<String>,
    bounds: Bounds<Pixels>,
}

/// The display the saved window belongs on: the saved one, if it is still connected. Without
/// a saved key, the one that holds the center of the window.
fn pick_screen(saved: Option<&str>, window: Bounds<Pixels>, screens: &[Screen]) -> Option<usize> {
    match saved {
        Some(saved) => screens
            .iter()
            .position(|screen| screen.key.as_deref() == Some(saved)),
        None => screens
            .iter()
            .position(|screen| screen.bounds.contains(&window.center())),
    }
}

/// The window on that display: no larger than it, and centered on it when the saved position
/// would not fit, because the window was shrunk or its center is off the display.
fn fit(window: Bounds<Pixels>, screen: Bounds<Pixels>) -> Bounds<Pixels> {
    let fitted = size(
        window.size.width.min(screen.size.width).max(MIN_SIZE.width),
        window
            .size
            .height
            .min(screen.size.height)
            .max(MIN_SIZE.height),
    );
    if fitted == window.size && screen.contains(&window.center()) {
        window
    } else {
        Bounds::centered_at(screen.center(), fitted)
    }
}

/// Where to open the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    pub(crate) bounds: WindowBounds,
    /// `None` leaves the choice to GPUI: the primary display.
    pub(crate) display: Option<DisplayId>,
}

impl WindowState {
    /// Captures the window bounds. A maximized window keeps its restore size, and a fullscreen
    /// window is saved as a normal one.
    pub(crate) fn from_window_bounds(bounds: WindowBounds, display: Option<String>) -> Self {
        let maximized = matches!(bounds, WindowBounds::Maximized(_));
        let bounds = bounds.get_bounds();
        Self {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
            maximized,
            display,
        }
    }

    /// Where to open the window. When its display is gone, or the position is on none, it
    /// goes to the center of the primary display.
    pub(crate) fn placement(&self, cx: &App) -> Placement {
        let window = Bounds::new(
            point(px(self.x), px(self.y)),
            size(px(self.width), px(self.height)),
        );
        let displays = cx.displays();
        let screens: Vec<Screen> = displays
            .iter()
            .map(|display| Screen {
                key: display_key(display.as_ref()),
                bounds: display.bounds(),
            })
            .collect();
        let found = pick_screen(self.display.as_deref(), window, &screens).and_then(|index| {
            let display = displays.get(index)?.id();
            Some((display, fit(window, screens.get(index)?.bounds)))
        });
        let (display, bounds) = if let Some((display, bounds)) = found {
            (Some(display), bounds)
        } else {
            let size = cx
                .primary_display()
                .map_or(window.size, |primary| fit(window, primary.bounds()).size);
            (None, Bounds::centered(None, size, cx))
        };
        let bounds = if self.maximized {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        };
        Placement { bounds, display }
    }

    fn is_valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|value| value.is_finite())
            && self.width >= f32::from(MIN_SIZE.width)
            && self.height >= f32::from(MIN_SIZE.height)
    }
}

/// What to keep for the next start, given what was kept so far and what the window reports.
///
/// A maximized or fullscreen window may report the size of the screen (X11 does), so the size
/// it had before is kept.
pub(crate) fn remember(kept: WindowBounds, reported: WindowBounds) -> WindowBounds {
    match reported {
        WindowBounds::Windowed(_) => reported,
        WindowBounds::Maximized(_) => WindowBounds::Maximized(kept.get_bounds()),
        WindowBounds::Fullscreen(_) => kept,
    }
}

/// A part of the requested bounds that the system did not apply when the window opened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Unapplied {
    Maximize,
    Resize(Size<Pixels>),
}

/// Compares an opened window with what was asked for. On X11, GPUI asks to maximize before
/// the window is shown, which window managers ignore, and a window manager may grow the window
/// by the margin of its shadow, so that it would be larger on every start.
pub(crate) fn unapplied(
    requested: WindowBounds,
    is_maximized: bool,
    window_size: Size<Pixels>,
) -> Option<Unapplied> {
    match requested {
        WindowBounds::Maximized(_) if !is_maximized => Some(Unapplied::Maximize),
        WindowBounds::Windowed(bounds) if !is_maximized && bounds.size != window_size => {
            Some(Unapplied::Resize(bounds.size))
        }
        _ => None,
    }
}

/// The file the window state is kept in.
#[derive(Debug, Clone)]
pub(crate) struct WindowStateFile {
    path: PathBuf,
}

impl WindowStateFile {
    /// The file in the per-user config directory, or `None` when the system has no such place.
    pub(crate) fn system() -> Option<Self> {
        dirs::config_local_dir().map(|dir| Self::at(dir.join(APP_DIR).join(FILE_NAME)))
    }

    fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// The saved state. A missing, unreadable or invalid file gives `None`.
    pub(crate) fn load(&self) -> Option<WindowState> {
        let content = match fs::read_to_string(&self.path) {
            Ok(content) => content,
            Err(error) => {
                if error.kind() != io::ErrorKind::NotFound {
                    self.report("cannot read", &error);
                }
                return None;
            }
        };
        match serde_json::from_str::<WindowState>(&content) {
            Ok(state) if state.is_valid() => Some(state),
            Ok(_) => {
                self.report("ignoring", &"the window size is not usable");
                None
            }
            Err(error) => {
                self.report("ignoring", &error);
                None
            }
        }
    }

    /// Writes the state. A failure is logged: losing the window position must not stop the app.
    pub(crate) fn save(&self, state: &WindowState) {
        if let Err(error) = self.try_save(state) {
            self.report("cannot save", &error);
        }
    }

    /// Goes through a temporary file, so that a crash cannot leave half a file.
    fn try_save(&self, state: &WindowState) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, serde_json::to_string_pretty(state)?)?;
        fs::rename(&temp, &self.path)
    }

    fn report(&self, what: &str, why: &dyn std::fmt::Display) {
        eprintln!("window state: {what} {}: {why}", self.path.display());
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use gpui::{Bounds, TestAppContext, WindowBounds, point, px, size};

    use super::{
        FILE_NAME, Screen, Unapplied, WindowState, WindowStateFile, display_key, fit, pick_screen,
        remember, unapplied,
    };

    fn state() -> WindowState {
        WindowState {
            x: 120.,
            y: 80.5,
            width: 900.,
            height: 700.,
            maximized: true,
            display: None,
        }
    }

    /// A fresh directory path for one test, which removes it at the end.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gpui-kit-rsx-app-{name}-{}", std::process::id()));
        // Left over from an earlier run, if any; usually it does not exist.
        fs::remove_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn captures_window_bounds() {
        let bounds = Bounds::new(point(px(120.), px(80.5)), size(px(900.), px(700.)));
        assert_eq!(
            WindowState::from_window_bounds(WindowBounds::Maximized(bounds), None),
            state()
        );
        assert!(!WindowState::from_window_bounds(WindowBounds::Fullscreen(bounds), None).maximized);
    }

    #[gpui::test]
    fn restores_a_position_on_a_display_and_centers_one_that_is_off(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let visible = WindowState {
                maximized: false,
                ..state()
            };
            let restored = visible.placement(cx);
            assert!(matches!(restored.bounds, WindowBounds::Windowed(_)));
            assert!(restored.display.is_some());
            assert_eq!(
                WindowState::from_window_bounds(restored.bounds, None),
                visible
            );

            let lost = WindowState {
                x: 100_000.,
                ..visible.clone()
            };
            let bounds = lost.placement(cx).bounds.get_bounds();
            assert_eq!(bounds, Bounds::centered(None, bounds.size, cx));
            assert_eq!(bounds.size, size(px(900.), px(700.)));

            assert!(matches!(
                state().placement(cx).bounds,
                WindowBounds::Maximized(_)
            ));
        });
    }

    #[gpui::test]
    fn returns_to_the_saved_display_while_it_is_connected(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let primary = cx.primary_display().expect("the test has a display");
            let here = WindowState {
                display: display_key(primary.as_ref()),
                ..state()
            };
            assert!(here.display.is_some());
            assert_eq!(here.placement(cx).display, Some(primary.id()));

            let unplugged = WindowState {
                display: Some("a display that is gone".to_owned()),
                ..state()
            };
            let placement = unplugged.placement(cx);
            assert_eq!(placement.display, None);
            let bounds = placement.bounds.get_bounds();
            assert_eq!(bounds, Bounds::centered(None, bounds.size, cx));
        });
    }

    /// Two displays side by side, in global coordinates.
    fn two_screens() -> [Screen; 2] {
        let screen = |key: &str, x: f32| Screen {
            key: Some(key.to_owned()),
            bounds: Bounds::new(point(px(x), px(0.)), size(px(1920.), px(1080.))),
        };
        [screen("left", 0.), screen("right", 1920.)]
    }

    #[test]
    fn picks_the_saved_display_even_when_positions_are_relative_to_it() {
        // macOS: a position on the right display looks like one on the left.
        let window = bounds(700., 460.);
        assert_eq!(pick_screen(Some("right"), window, &two_screens()), Some(1));
        assert_eq!(pick_screen(Some("gone"), window, &two_screens()), None);
    }

    #[test]
    fn without_a_saved_display_picks_the_one_under_the_window() {
        let window = Bounds::new(point(px(2500.), px(100.)), size(px(700.), px(460.)));
        assert_eq!(pick_screen(None, window, &two_screens()), Some(1));
        let nowhere = Bounds::new(point(px(9000.), px(100.)), window.size);
        assert_eq!(pick_screen(None, nowhere, &two_screens()), None);
    }

    #[test]
    fn fits_a_window_to_its_display() {
        let [_, right] = two_screens();
        let inside = Bounds::new(point(px(2500.), px(100.)), size(px(700.), px(460.)));
        assert_eq!(fit(inside, right.bounds), inside);

        let oversized = Bounds::new(point(px(2500.), px(100.)), size(px(3000.), px(2000.)));
        assert_eq!(fit(oversized, right.bounds), right.bounds);

        let off = Bounds::new(point(px(100.), px(100.)), size(px(700.), px(460.)));
        assert_eq!(
            fit(off, right.bounds),
            Bounds::centered_at(right.bounds.center(), off.size)
        );
    }

    #[test]
    fn reads_a_file_saved_without_a_display() {
        let json = r#"{"x":120.0,"y":80.5,"width":900.0,"height":700.0,"maximized":true}"#;
        assert_eq!(serde_json::from_str::<WindowState>(json).unwrap(), state());
    }

    fn bounds(width: f32, height: f32) -> Bounds<gpui::Pixels> {
        Bounds::new(point(px(10.), px(20.)), size(px(width), px(height)))
    }

    #[test]
    fn a_maximized_or_fullscreen_window_keeps_its_earlier_size() {
        let small = WindowBounds::Windowed(bounds(700., 460.));
        let screen = bounds(1920., 1171.);
        let maximized = remember(small, WindowBounds::Maximized(screen));
        assert_eq!(maximized, WindowBounds::Maximized(bounds(700., 460.)));
        assert_eq!(
            remember(maximized, WindowBounds::Fullscreen(screen)),
            maximized
        );
        let resized = WindowBounds::Windowed(bounds(800., 600.));
        assert_eq!(remember(maximized, resized), resized);
    }

    #[test]
    fn finds_what_the_system_did_not_apply() {
        let windowed = WindowBounds::Windowed(bounds(700., 460.));
        let maximized = WindowBounds::Maximized(bounds(700., 460.));
        let asked = size(px(700.), px(460.));
        let grown = size(px(740.), px(500.));

        assert_eq!(unapplied(windowed, false, asked), None);
        assert_eq!(
            unapplied(windowed, false, grown),
            Some(Unapplied::Resize(asked))
        );
        // Maximized by the user in the meantime: leave it alone.
        assert_eq!(unapplied(windowed, true, grown), None);
        assert_eq!(
            unapplied(maximized, false, asked),
            Some(Unapplied::Maximize)
        );
        assert_eq!(unapplied(maximized, true, grown), None);
    }

    #[test]
    fn rejects_tiny_or_non_finite_sizes() {
        assert!(state().is_valid());
        assert!(
            !WindowState {
                width: 10.,
                ..state()
            }
            .is_valid()
        );
        assert!(
            !WindowState {
                height: f32::NAN,
                ..state()
            }
            .is_valid()
        );
    }

    #[test]
    fn saves_and_loads_through_the_file() {
        let dir = scratch_dir("save-load");
        let file = WindowStateFile::at(dir.join("nested").join(FILE_NAME));
        assert_eq!(file.load(), None);
        let on_a_display = WindowState {
            display: Some("left".to_owned()),
            ..state()
        };
        file.save(&on_a_display);
        assert_eq!(file.load(), Some(on_a_display));
        file.save(&state());
        assert_eq!(file.load(), Some(state()));
        // Saving again replaces the file, and the temporary one is renamed away.
        file.save(&state());
        assert!(!file.path.with_extension("json.tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ignores_a_damaged_or_invalid_file() {
        let dir = scratch_dir("damaged");
        fs::create_dir_all(&dir).unwrap();
        let file = WindowStateFile::at(dir.join(FILE_NAME));
        fs::write(&file.path, "{ not json").unwrap();
        assert_eq!(file.load(), None);
        file.save(&WindowState {
            width: 10.,
            ..state()
        });
        assert_eq!(file.load(), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failed_save_does_not_panic() {
        let dir = scratch_dir("failed");
        // A directory where the file should be: the rename fails and is only logged.
        let path = dir.join(FILE_NAME);
        fs::create_dir_all(&path).unwrap();
        WindowStateFile::at(path).save(&state());
        fs::remove_dir_all(&dir).unwrap();
    }
}
