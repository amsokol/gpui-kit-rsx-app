//! The font the desktop uses for its interface.
//!
//! The theme's default family is `.SystemUIFont`, which GPUI maps to the system font on macOS
//! (San Francisco) and on Windows (the message font, Segoe UI). On Linux GPUI maps it to a fixed
//! family instead, so the desktop's own setting is read here, through the XDG settings portal.
//!
//! Only the family follows the system. The size stays GPUI Kit's, which its spacing is made for.

use std::{env, time::Duration};

use futures_lite::{FutureExt as _, StreamExt as _, stream};
use gpui::{App, AsyncApp, Global, SharedString};

/// Forces a font family, e.g. `Noto Sans`, to try another font without changing the system.
const OVERRIDE_ENV: &str = "GPUI_KIT_RSX_APP_FONT";

/// The family GPUI resolves by itself, used when the system names none that is installed.
const DEFAULT_FAMILY: &str = ".SystemUIFont";

/// How long to wait for the system at startup. The window opens afterwards, so keep it short.
const READ_TIMEOUT: Duration = Duration::from_millis(200);

/// The words that may follow the family in a GNOME (Pango) font description, in lower case.
const STYLE_WORDS: &[&str] = &[
    "thin",
    "ultra-light",
    "extra-light",
    "light",
    "semi-light",
    "demi-light",
    "book",
    "regular",
    "medium",
    "semi-bold",
    "demi-bold",
    "bold",
    "ultra-bold",
    "extra-bold",
    "heavy",
    "black",
    "ultra-black",
    "extra-black",
    "normal",
    "roman",
    "italic",
    "oblique",
    "small-caps",
    "ultra-condensed",
    "extra-condensed",
    "condensed",
    "semi-condensed",
    "semi-expanded",
    "expanded",
    "extra-expanded",
    "ultra-expanded",
];

/// Where a desktop keeps its UI font, and how it writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// GNOME, and the desktops built on its settings: `Ubuntu Sans Bold 10`.
    Gnome,
    /// KDE Plasma: `Noto Sans,10,-1,5,50,0,0,0,0,0`.
    Kde,
}

impl Source {
    /// The order to ask in. KDE may also answer for GNOME's key, with a default that is not
    /// the user's choice, so on KDE its own key comes first.
    fn in_order(desktop: &str) -> [Self; 2] {
        if desktop.split(':').any(|name| name == "KDE") {
            [Self::Kde, Self::Gnome]
        } else {
            [Self::Gnome, Self::Kde]
        }
    }

    /// The namespace and the key in the settings portal.
    fn setting(self) -> (&'static str, &'static str) {
        match self {
            Self::Gnome => ("org.gnome.desktop.interface", "font-name"),
            Self::Kde => ("org.kde.kdeglobals.General", "font"),
        }
    }

    /// The family names the value may stand for, the most likely first.
    fn families(self, value: &str) -> Vec<String> {
        match self {
            Self::Gnome => gnome_families(value),
            Self::Kde => kde_families(value),
        }
    }
}

/// A Pango description is `family [style words] [size]`, and a family may itself end in a style
/// word ("Arial Black"). So every reading is returned, the longest family first, for the caller
/// to take the first one that is installed.
fn gnome_families(value: &str) -> Vec<String> {
    // Variations such as `@wght=500` follow the size.
    let mut words: Vec<&str> = value
        .split_whitespace()
        .filter(|word| !word.starts_with('@'))
        .collect();
    let is_size = |word: &&str| {
        word.strip_suffix("px")
            .unwrap_or(word)
            .parse::<f32>()
            .is_ok()
    };
    if words.last().is_some_and(is_size) {
        words.pop();
    }

    let mut families = Vec::new();
    while !words.is_empty() {
        families.push(words.join(" ").trim_end_matches(',').to_owned());
        let ends_in_style = words.last().is_some_and(|word| {
            STYLE_WORDS.contains(&word.trim_end_matches(',').to_ascii_lowercase().as_str())
        });
        if !ends_in_style {
            break;
        }
        words.pop();
    }
    families
}

/// KDE writes the family first, then the size and the style as numbers.
fn kde_families(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .next()
        .filter(|family| !family.is_empty())
        .map(str::to_owned)
        .into_iter()
        .collect()
}

/// The first candidate that is installed.
fn first_installed<'a>(candidates: &'a [String], installed: &[String]) -> Option<&'a String> {
    candidates
        .iter()
        .find(|candidate| installed.contains(candidate))
}

/// The installed family the system asked for.
#[derive(Default)]
struct SystemFont(Option<SharedString>);

impl Global for SystemFont {}

/// The family the theme should use.
pub(crate) fn family(cx: &App) -> SharedString {
    cx.try_global::<SystemFont>()
        .and_then(|font| font.0.clone())
        .unwrap_or_else(|| DEFAULT_FAMILY.into())
}

/// Stores the first installed family among `candidates`. Returns whether [`family`] changed.
pub(crate) fn set(cx: &mut App, candidates: &[String]) -> bool {
    let installed = if candidates.is_empty() {
        Vec::new()
    } else {
        cx.text_system().all_font_names()
    };
    let found = first_installed(candidates, &installed)
        .cloned()
        .map(SharedString::from);
    let before = family(cx);
    cx.set_global(SystemFont(found));
    family(cx) != before
}

/// Stores a family as if the system had it installed: the test platform has no fonts.
#[cfg(test)]
pub(crate) fn set_installed(cx: &mut App, family: &'static str) {
    cx.set_global(SystemFont(Some(family.into())));
}

/// The family forced through [`OVERRIDE_ENV`].
fn forced() -> Option<String> {
    env::var(OVERRIDE_ENV)
        .ok()
        .map(|family| family.trim().to_owned())
        .filter(|family| !family.is_empty())
}

fn sources() -> [Source; 2] {
    Source::in_order(&env::var("XDG_CURRENT_DESKTOP").unwrap_or_default())
}

/// The first source that has a value, with that value.
async fn first_setting() -> Option<(Source, String)> {
    for source in sources() {
        let (namespace, key) = source.setting();
        if let Some(value) = portal::read(namespace, key).await {
            return Some((source, value));
        }
    }
    None
}

/// Reads the families the system asks for, to pass to [`set`]. Empty when GPUI's own choice is
/// right (macOS, Windows) or the system does not answer in time.
pub(crate) async fn read(cx: &mut AsyncApp) -> Vec<String> {
    if let Some(family) = forced() {
        return vec![family];
    }
    let timer = cx.background_executor().timer(READ_TIMEOUT);
    let timeout = async {
        timer.await;
        None
    };
    first_setting()
        .or(timeout)
        .await
        .map_or_default(|(source, value)| source.families(&value))
}

/// A setting's value now, if it has one, followed by the values it takes later.
fn current_then(current: Option<String>, changes: stream::Boxed<String>) -> stream::Boxed<String> {
    stream::iter(current).chain(changes).boxed()
}

/// Follows the font in the system settings and calls `on_change` whenever the family to use
/// changes. That includes the first value: [`read`] may have given up before it arrived.
pub(crate) fn watch(cx: &mut App, on_change: impl Fn(&mut App) + 'static) {
    if forced().is_some() {
        return;
    }
    cx.spawn(async move |cx| {
        let Some((source, _)) = first_setting().await else {
            return;
        };
        let (namespace, key) = source.setting();
        // Subscribe before reading, so that no change gets lost between the two.
        let Some(changes) = portal::changes(namespace, key).await else {
            return;
        };
        let mut values = current_then(portal::read(namespace, key).await, changes);
        while let Some(value) = values.next().await {
            let candidates = source.families(&value);
            cx.update(|cx| {
                if set(cx, &candidates) {
                    on_change(cx);
                }
            });
        }
    })
    .detach();
}

/// The XDG settings portal. This is the only code that exists on Linux alone: `ashpd` is a
/// Linux dependency.
#[cfg(target_os = "linux")]
mod portal {
    use ashpd::desktop::settings::Settings;
    use futures_lite::{StreamExt as _, stream};

    /// A setting's text value, or `None` when there is no portal or no such setting.
    pub(super) async fn read(namespace: &str, key: &str) -> Option<String> {
        let settings = Settings::new().await.ok()?;
        settings.read::<String>(namespace, key).await.ok()
    }

    /// The values a setting takes from now on.
    pub(super) async fn changes(namespace: &str, key: &str) -> Option<stream::Boxed<String>> {
        let settings = Settings::new().await.ok()?;
        let values = settings
            .receive_setting_changed_with_args::<String>(namespace, key)
            .await
            .ok()?;
        Some(values.filter_map(Result::ok).boxed())
    }
}

/// Other systems have no portal, and GPUI already picks their UI font.
#[cfg(not(target_os = "linux"))]
mod portal {
    use futures_lite::stream;

    pub(super) async fn read(_namespace: &str, _key: &str) -> Option<String> {
        std::future::ready(None).await
    }

    pub(super) async fn changes(_namespace: &str, _key: &str) -> Option<stream::Boxed<String>> {
        std::future::ready(None).await
    }
}

#[cfg(test)]
mod tests {
    use futures_lite::{StreamExt as _, future, stream};

    use super::{Source, current_then, first_installed, gnome_families, kde_families};

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|&name| name.to_owned()).collect()
    }

    #[test]
    fn reads_a_gnome_font() {
        assert_eq!(gnome_families("Ubuntu Sans 10"), names(&["Ubuntu Sans"]));
        assert_eq!(gnome_families("Cantarell, 11"), names(&["Cantarell"]));
        assert_eq!(gnome_families("Inter 10.5"), names(&["Inter"]));
        assert_eq!(gnome_families("Inter 14px @wght=500"), names(&["Inter"]));
        assert_eq!(gnome_families(""), names(&[]));
    }

    #[test]
    fn offers_every_reading_of_trailing_style_words() {
        assert_eq!(
            gnome_families("Noto Sans Bold Italic 11"),
            names(&["Noto Sans Bold Italic", "Noto Sans Bold", "Noto Sans"])
        );
        assert_eq!(
            gnome_families("Arial Black 10"),
            names(&["Arial Black", "Arial"])
        );
    }

    #[test]
    fn reads_a_kde_font() {
        assert_eq!(
            kde_families("Noto Sans,10,-1,5,50,0,0,0,0,0"),
            names(&["Noto Sans"])
        );
        assert_eq!(kde_families(""), names(&[]));
    }

    #[test]
    fn kde_asks_its_own_key_first() {
        assert_eq!(Source::in_order("KDE"), [Source::Kde, Source::Gnome]);
        assert_eq!(
            Source::in_order("ubuntu:GNOME"),
            [Source::Gnome, Source::Kde]
        );
        assert_eq!(Source::in_order(""), [Source::Gnome, Source::Kde]);
    }

    #[test]
    fn follows_the_current_value_and_then_the_changes() {
        let values = |current: Option<&str>| {
            let changes = stream::iter(names(&["Inter"])).boxed();
            future::block_on(current_then(current.map(str::to_owned), changes).collect::<Vec<_>>())
        };
        assert_eq!(values(Some("Cantarell")), names(&["Cantarell", "Inter"]));
        assert_eq!(values(None), names(&["Inter"]));
    }

    #[test]
    fn takes_the_first_installed_candidate() {
        let installed = names(&["Arial", "Noto Sans"]);
        let candidates = names(&["Arial Black", "Arial"]);
        assert_eq!(
            first_installed(&candidates, &installed).map(String::as_str),
            Some("Arial")
        );
        assert_eq!(first_installed(&names(&["Nope"]), &installed), None);
    }
}
