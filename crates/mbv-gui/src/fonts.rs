//! The GUI Client's embedded fonts (design D8): the Lexend Deca typeface as
//! the first proportional family and `JetBrains Mono` as the first monospace
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

const LEXEND_DECA: EmbeddedFont = EmbeddedFont {
    family: "Lexend Deca",
    regular: include_bytes!("../assets/fonts/LexendDeca-Regular.ttf"),
    bold: include_bytes!("../assets/fonts/LexendDeca-Bold.ttf"),
};

const JETBRAINS_MONO: EmbeddedFont = EmbeddedFont {
    family: "JetBrains Mono",
    regular: include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    bold: include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
};

/// The egui family of the bold Lexend Deca face. The Queue status word and the
/// footer pills paint in it.
const BOLD_FAMILY: &str = "Lexend Deca Bold";

/// `size` logical points of the bold Lexend Deca face.
pub(crate) fn proportional_bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(BOLD_FAMILY.into()))
}

/// Install the embedded families as the leading proportional and monospace
/// families at egui's default sizes (design D8). egui's built-in fonts stay
/// in each family behind the embedded ones as fallbacks.
pub(crate) fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for font in [&LEXEND_DECA, &JETBRAINS_MONO] {
        fonts.font_data.insert(
            format!("{}-regular", font.family),
            egui::FontData::from_static(font.regular).into(),
        );
        fonts.font_data.insert(
            format!("{}-bold", font.family),
            egui::FontData::from_static(font.bold).into(),
        );
    }
    // Insert at index 0 so the embedded family leads and egui's built-ins
    // follow as fallbacks.
    for (font, proportional) in [(&LEXEND_DECA, true), (&JETBRAINS_MONO, false)] {
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
    // The bold face leads a named family of its own. egui reads a named family
    // only from its own list, so the proportional chain follows the bold face
    // as the fallbacks for glyphs the bold face lacks.
    let proportional = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .clone();
    let mut bold_chain = vec![format!("{}-bold", LEXEND_DECA.family)];
    bold_chain.extend(proportional);
    fonts
        .families
        .insert(egui::FontFamily::Name(BOLD_FAMILY.into()), bold_chain);
    ctx.set_fonts(fonts);
}
