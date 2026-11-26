//! Bold text with a variable font on Linux, where GPUI draws text with `gpui-pre-wgpu`. The
//! crate is patched (`vendor/gpui-pre-wgpu.patch`): unpatched, it uses every variable font at
//! its default weight.

#![cfg(target_os = "linux")]
// Tests may panic; the same lints as at the top of src/main.rs.
#![allow(
    clippy::disallowed_macros,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::string_slice,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_used,
    reason = "tests may panic"
)]

use std::{borrow::Cow, path::PathBuf, process::Command};

use gpui::{
    Font, FontId, FontRun, FontWeight, PlatformTextSystem, RenderGlyphParams, font, point, px,
};
use gpui_wgpu::CosmicTextSystem;

const TEXT: &str = "Hamburgefonstiv";

/// A test font that `cosmic-text`, the text engine under GPUI on Linux, ships in its package.
/// Cargo.lock pins the version, and Cargo knows where it unpacked it.
fn cosmic_text_font(file: &str) -> Vec<u8> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo metadata");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let manifest = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "cosmic-text")
        .and_then(|package| package["manifest_path"].as_str())
        .expect("cosmic-text is a dependency");
    let path = PathBuf::from(manifest).with_file_name("fonts").join(file);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A text system that knows this one font, and the name of its family.
fn text_system(file: &str) -> (CosmicTextSystem, String) {
    let system = CosmicTextSystem::new_without_system_fonts("none");
    system
        .add_fonts(vec![Cow::Owned(cosmic_text_font(file))])
        .unwrap();
    let family = system.all_font_names().remove(0);
    (system, family)
}

fn id(system: &CosmicTextSystem, family: &str, weight: FontWeight) -> FontId {
    let font = Font {
        weight,
        ..font(family.to_owned())
    };
    system.font_id(&font).unwrap()
}

/// The width of [`TEXT`], checking that it was shaped with the requested font.
fn width(system: &CosmicTextSystem, font_id: FontId) -> f32 {
    let runs = [FontRun {
        len: TEXT.len(),
        font_id,
    }];
    let line = system.layout_line(TEXT, px(32.), &runs);
    assert!(line.runs.iter().all(|run| run.font_id == font_id));
    line.width.into()
}

fn advance(system: &CosmicTextSystem, font_id: FontId) -> f32 {
    let glyph = system.glyph_for_char(font_id, 'm').unwrap();
    system.advance(font_id, glyph).unwrap().width
}

/// How much ink the drawn letter has.
fn ink(system: &CosmicTextSystem, font_id: FontId) -> u64 {
    let params = RenderGlyphParams {
        font_id,
        glyph_id: system.glyph_for_char(font_id, 'm').unwrap(),
        font_size: px(64.),
        subpixel_variant: point(0, 0),
        scale_factor: 1.,
        is_emoji: false,
        subpixel_rendering: false,
        dilation: 0,
    };
    let bounds = system.glyph_raster_bounds(&params).unwrap();
    let (_, pixels) = system.rasterize_glyph(&params, bounds).unwrap();
    pixels.iter().map(|&alpha| u64::from(alpha)).sum()
}

#[test]
fn a_variable_font_is_shaped_measured_and_drawn_at_the_requested_weight() {
    let (system, family) = text_system("InterVariable.ttf");
    let [regular, semibold, bold] = [FontWeight::NORMAL, FontWeight::SEMIBOLD, FontWeight::BOLD]
        .map(|weight| id(&system, &family, weight));

    assert_ne!(regular, bold);
    for (lighter, heavier) in [(regular, semibold), (semibold, bold)] {
        assert!(width(&system, lighter) < width(&system, heavier), "shaping");
        assert!(
            advance(&system, lighter) < advance(&system, heavier),
            "metrics"
        );
        assert!(ink(&system, lighter) < ink(&system, heavier), "drawing");
    }
}

#[test]
fn a_weight_keeps_its_font_and_its_looks_after_others_were_used() {
    let (system, family) = text_system("InterVariable.ttf");
    let regular = id(&system, &family, FontWeight::NORMAL);
    let before = (width(&system, regular), ink(&system, regular));
    let bold = id(&system, &family, FontWeight::BOLD);
    ink(&system, bold);

    assert_eq!(id(&system, &family, FontWeight::NORMAL), regular);
    assert_eq!((width(&system, regular), ink(&system, regular)), before);
}

#[test]
fn a_static_font_has_one_font_for_all_weights() {
    let (system, family) = text_system("Inter-Regular.ttf");
    assert_eq!(
        id(&system, &family, FontWeight::NORMAL),
        id(&system, &family, FontWeight::BOLD)
    );
}
