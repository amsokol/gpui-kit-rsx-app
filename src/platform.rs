//! What differs between the operating systems.
//!
//! The rules are plain functions of [`Os`], not `#[cfg]` blocks: every branch compiles and is
//! linted on every system, and the tests on one system cover all three.

use gpui::Decorations;

/// An operating system the app runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Os {
    Mac,
    Windows,
    /// Linux and the other desktops that follow the freedesktop conventions.
    Linux,
}

impl Os {
    /// The system this build runs on.
    pub(crate) fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// Where the title sits in the title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TitleAlignment {
    Start,
    Center,
}

/// macOS, GNOME and KDE center the title; Windows puts it at the start.
pub(crate) fn title_alignment(os: Os) -> TitleAlignment {
    match os {
        Os::Mac | Os::Linux => TitleAlignment::Center,
        Os::Windows => TitleAlignment::Start,
    }
}

/// macOS, GNOME and KDE write the title in bold; Windows in the regular weight.
pub(crate) fn title_is_bold(os: Os) -> bool {
    os != Os::Windows
}

/// Windows shows the app's icon before the title; macOS and GNOME show the title alone.
pub(crate) fn title_has_icon(os: Os) -> bool {
    os == Os::Windows
}

/// Whether the app has to draw the title bar itself.
///
/// On Linux, "server" decorations mean the window manager draws a title bar (for example X11
/// without a compositor), and ours would be a second one. macOS and Windows always report
/// "server", GPUI's default, although the title bar there is ours to draw.
pub(crate) fn draws_own_title_bar(os: Os, decorations: Decorations) -> bool {
    os != Os::Linux || matches!(decorations, Decorations::Client { .. })
}

#[cfg(test)]
mod tests {
    use gpui::{Decorations, Tiling};

    use super::{
        Os, TitleAlignment, draws_own_title_bar, title_alignment, title_has_icon, title_is_bold,
    };

    #[test]
    fn only_windows_puts_an_icon_in_the_title() {
        assert!(title_has_icon(Os::Windows));
        assert!(!title_has_icon(Os::Mac));
        assert!(!title_has_icon(Os::Linux));
    }

    #[test]
    fn the_title_is_bold_except_on_windows() {
        assert!(title_is_bold(Os::Mac));
        assert!(title_is_bold(Os::Linux));
        assert!(!title_is_bold(Os::Windows));
    }

    #[test]
    fn the_title_is_centered_except_on_windows() {
        assert_eq!(title_alignment(Os::Mac), TitleAlignment::Center);
        assert_eq!(title_alignment(Os::Linux), TitleAlignment::Center);
        assert_eq!(title_alignment(Os::Windows), TitleAlignment::Start);
    }

    #[test]
    fn linux_leaves_the_title_bar_to_a_server_that_draws_one() {
        let client = Decorations::Client {
            tiling: Tiling::default(),
        };
        assert!(draws_own_title_bar(Os::Linux, client));
        assert!(!draws_own_title_bar(Os::Linux, Decorations::Server));
    }

    #[test]
    fn macos_and_windows_draw_it_although_they_report_server_decorations() {
        assert!(draws_own_title_bar(Os::Mac, Decorations::Server));
        assert!(draws_own_title_bar(Os::Windows, Decorations::Server));
    }
}
