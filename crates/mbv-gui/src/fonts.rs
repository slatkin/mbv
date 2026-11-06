//! The GUI Client's embedded fonts (design D8): the SUSE typeface as the
//! first proportional family and `JetBrains Mono` as the first monospace
//! family, both under the SIL Open Font License 1.1, with the license texts
//! shipped beside the font files (`assets/fonts/`). egui's built-in fonts
//! stay as fallbacks for glyphs these families do not have, such as emoji.
//!
//! Installing at start-up means the fonts never depend on the fonts on the
//! user's system.

use eframe::egui;

/// One embedded family: the egui family name, the two weights (regular and
/// bold), and the proportional slot the family leads.
struct EmbeddedFont {
    family: &'static str,
    regular: &'static [u8],
    bold: &'static [u8],
}

const SUSE: EmbeddedFont = EmbeddedFont {
    family: "SUSE",
    regular: include_bytes!("../assets/fonts/SUSE-Regular.ttf"),
    bold: include_bytes!("../assets/fonts/SUSE-Bold.ttf"),
};

const JETBRAINS_MONO: EmbeddedFont = EmbeddedFont {
    family: "JetBrains Mono",
    regular: include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    bold: include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
};

/// The hinting for each embedded face. Horizontal grid-fitting is on and
/// symmetric rendering is off: the instructions can widen stems, which looks
/// blurrier under egui's analytic rasterizer. The smooth target stays, and
/// every other field keeps its default.
fn embedded_tweak() -> egui::FontTweak {
    let hinting = egui::epaint::text::SmoothHinting {
        symmetric_rendering: false,
        preserve_linear_metrics: false,
        ..egui::epaint::text::SmoothHinting::default()
    };
    egui::FontTweak {
        hinting_target: egui::epaint::text::HintingTarget::Smooth(hinting),
        ..egui::FontTweak::default()
    }
}

/// Install the embedded families as the leading proportional and monospace
/// families at egui's default sizes (design D8). egui's built-in fonts stay
/// in each family behind the embedded ones as fallbacks. Sub-pixel binning
/// is off because it blurs glyphs.
pub(crate) fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for font in [&SUSE, &JETBRAINS_MONO] {
        fonts.font_data.insert(
            format!("{}-regular", font.family),
            egui::FontData::from_static(font.regular)
                .tweak(embedded_tweak())
                .into(),
        );
        fonts.font_data.insert(
            format!("{}-bold", font.family),
            egui::FontData::from_static(font.bold)
                .tweak(embedded_tweak())
                .into(),
        );
    }
    // Insert at index 0 so the embedded family leads and egui's built-ins
    // follow as fallbacks.
    for (font, proportional) in [(&SUSE, true), (&JETBRAINS_MONO, false)] {
        let family = if proportional {
            egui::FontFamily::Proportional
        } else {
            egui::FontFamily::Monospace
        };
        let slots = fonts.families.entry(family).or_default();
        slots.insert(0, format!("{}-regular", font.family));
        // The bold face sits right behind the regular one so egui's emphasis
        // resolves inside the embedded family before the fallbacks.
        slots.insert(1, format!("{}-bold", font.family));
    }
    ctx.set_fonts(fonts);
    // Both theme styles, so a theme switch keeps the setting.
    ctx.all_styles_mut(|style| style.visuals.text_options.subpixel_binning = false);
}
