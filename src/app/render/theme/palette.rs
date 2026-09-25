//! The closed palette of UI colours.

use ratatui::style::Color;

/// The 19 approved palette variants (hue families, dark-to-light within each
/// family; see `openspec/changes/archive/2026-09-19-palette-enum/name-table.md`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub(in crate::app) enum Palette {
    Grey1,
    Grey2,
    Grey3,
    Green1,
    Green2,
    Green3,
    Green,
    Iris,
    Aqua,
    Ink,
    Slate,
    Storm,
    Foam,
    Mauve,
    Red,
    Orange,
    Yellow,
    Cream,
    White,
}

impl Palette {
    /// The variant's colour. The only place a palette `Color::Rgb(...)`
    /// literal lives in theme code.
    pub(in crate::app) const fn color(self) -> Color {
        match self {
            Palette::Grey1 => Color::Rgb(0x1a, 0x1a, 0x1a),
            Palette::Grey2 => Color::Rgb(0x9e, 0x9e, 0x9e),
            Palette::Grey3 => Color::Rgb(0xe6, 0xe6, 0xe6),
            Palette::Green1 => Color::Rgb(0x2e, 0x38, 0x3c),
            Palette::Green2 => Color::Rgb(0x37, 0x41, 0x45),
            Palette::Green3 => Color::Rgb(0x6c, 0x76, 0x6c),
            Palette::Green => Color::Rgb(0x93, 0xb2, 0x59),
            Palette::Iris => Color::Rgb(0xa7, 0xc0, 0x80),
            Palette::Aqua => Color::Rgb(0x35, 0xa7, 0x7c),
            Palette::Ink => Color::Rgb(0x1e, 0x23, 0x26),
            Palette::Slate => Color::Rgb(0x27, 0x2e, 0x33),
            Palette::Storm => Color::Rgb(0x2b, 0x32, 0x38),
            Palette::Foam => Color::Rgb(0x3a, 0x94, 0xc5),
            Palette::Mauve => Color::Rgb(0xd6, 0x99, 0xb6),
            Palette::Red => Color::Rgb(0xe5, 0x7e, 0x80),
            Palette::Orange => Color::Rgb(0xe5, 0x98, 0x75),
            Palette::Yellow => Color::Rgb(0xdb, 0xbc, 0x7f),
            Palette::Cream => Color::Rgb(0xfa, 0xed, 0xcd),
            Palette::White => Color::Rgb(0xfd, 0xf6, 0xe3),
        }
    }
}
