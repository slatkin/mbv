//! The closed slot set and the [`Theme`] value (change `theme-slot-model`,
//! design D1/D2).
//!
//! A slot is a bare colour position named by its tier or hue — never by a
//! role. A [`Theme`] assigns exactly one [`Color`] to every slot, and
//! [`Theme::DEFAULT`] is the only place a palette `Color::Rgb` literal lives.
//! Both stay crate-private: consumers reach a slot only through a
//! [`super::Role`](crate::Role) or a surface row resolved against
//! [`active()`].

use ratatui::style::Color;

/// The 19 colour slots, named by background/foreground tier or accent hue
/// (design D1). The background ladder is numbered darkest-first (`BgDim`,
/// `Bg0`–`Bg3`); the foreground ladder runs faint-to-bright; the accents keep
/// hue names. The declaration order is the `Theme` storage order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum Slot {
    BgDim,
    Bg0,
    Bg1,
    Bg2,
    Bg3,
    OnAccent,
    FgFaint,
    FgMuted,
    Fg,
    FgWarm,
    FgBright,
    Red,
    Orange,
    Yellow,
    Green,
    GreenDeep,
    Aqua,
    Blue,
    Purple,
}

impl Slot {
    /// The number of slots: the `Theme` storage length.
    pub(crate) const COUNT: usize = 19;
}

#[cfg(test)]
impl Slot {
    /// Every slot, in declaration order. The `Theme` array and the
    /// `docs/palette.json` generator iterate in this order.
    pub(crate) const ALL: &[Slot] = &[
        Slot::BgDim,
        Slot::Bg0,
        Slot::Bg1,
        Slot::Bg2,
        Slot::Bg3,
        Slot::OnAccent,
        Slot::FgFaint,
        Slot::FgMuted,
        Slot::Fg,
        Slot::FgWarm,
        Slot::FgBright,
        Slot::Red,
        Slot::Orange,
        Slot::Yellow,
        Slot::Green,
        Slot::GreenDeep,
        Slot::Aqua,
        Slot::Blue,
        Slot::Purple,
    ];
}

/// A theme: one colour per slot (design D2). Stored as a fixed array indexed
/// by `Slot` in declaration order; [`Theme::get`] is the only accessor.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Theme([Color; Slot::COUNT]);

impl Theme {
    /// The built-in default theme: today's exact palette values (design D1).
    /// The only place a palette `Color::Rgb` literal lives.
    pub(crate) const DEFAULT: Theme = Theme([
        Color::Rgb(0x1e, 0x23, 0x26), // BgDim
        Color::Rgb(0x27, 0x2e, 0x33), // Bg0
        Color::Rgb(0x2b, 0x32, 0x38), // Bg1
        Color::Rgb(0x2e, 0x38, 0x3c), // Bg2
        Color::Rgb(0x37, 0x41, 0x45), // Bg3
        Color::Rgb(0x1a, 0x1a, 0x1a), // OnAccent
        Color::Rgb(0x6c, 0x76, 0x6c), // FgFaint
        Color::Rgb(0x9e, 0x9e, 0x9e), // FgMuted
        Color::Rgb(0xe6, 0xe6, 0xe6), // Fg
        Color::Rgb(0xfa, 0xed, 0xcd), // FgWarm
        Color::Rgb(0xfd, 0xf6, 0xe3), // FgBright
        Color::Rgb(0xe5, 0x7e, 0x80), // Red
        Color::Rgb(0xe5, 0x98, 0x75), // Orange
        Color::Rgb(0xdb, 0xbc, 0x7f), // Yellow
        Color::Rgb(0xa7, 0xc0, 0x80), // Green
        Color::Rgb(0x93, 0xb2, 0x59), // GreenDeep
        Color::Rgb(0x35, 0xa7, 0x7c), // Aqua
        Color::Rgb(0x3a, 0x94, 0xc5), // Blue
        Color::Rgb(0xd6, 0x99, 0xb6), // Purple
    ]);

    /// The slot's colour in this theme.
    pub(crate) const fn get(&self, slot: Slot) -> Color {
        self.0[slot as usize]
    }
}

/// The active theme. Returns the built-in default; selecting another theme is
/// out of scope here (design D2) — a later theming change decides set-once
/// versus live switching behind this one resolution point.
pub(crate) const fn active() -> &'static Theme {
    &Theme::DEFAULT
}

#[cfg(test)]
mod tests {
    use super::{Slot, Theme, active};
    use crate::surface_resolve::surface_colors_in;
    use crate::{Role, Surface};
    use ratatui::style::Color;

    #[test]
    fn slot_all_is_the_declaration_order() {
        // The swap test below builds a `Theme` array by iterating `ALL`, so
        // `ALL` must stay aligned with the storage order (`slot as usize`).
        for (index, &slot) in Slot::ALL.iter().enumerate() {
            assert_eq!(slot as usize, index, "{slot:?} out of order in ALL");
        }
    }

    #[test]
    fn default_theme_assigns_the_design_d1_hexes() {
        let expected = [
            (Slot::BgDim, [0x1e, 0x23, 0x26]),
            (Slot::Bg0, [0x27, 0x2e, 0x33]),
            (Slot::Bg1, [0x2b, 0x32, 0x38]),
            (Slot::Bg2, [0x2e, 0x38, 0x3c]),
            (Slot::Bg3, [0x37, 0x41, 0x45]),
            (Slot::OnAccent, [0x1a, 0x1a, 0x1a]),
            (Slot::FgFaint, [0x6c, 0x76, 0x6c]),
            (Slot::FgMuted, [0x9e, 0x9e, 0x9e]),
            (Slot::Fg, [0xe6, 0xe6, 0xe6]),
            (Slot::FgWarm, [0xfa, 0xed, 0xcd]),
            (Slot::FgBright, [0xfd, 0xf6, 0xe3]),
            (Slot::Red, [0xe5, 0x7e, 0x80]),
            (Slot::Orange, [0xe5, 0x98, 0x75]),
            (Slot::Yellow, [0xdb, 0xbc, 0x7f]),
            (Slot::Green, [0xa7, 0xc0, 0x80]),
            (Slot::GreenDeep, [0x93, 0xb2, 0x59]),
            (Slot::Aqua, [0x35, 0xa7, 0x7c]),
            (Slot::Blue, [0x3a, 0x94, 0xc5]),
            (Slot::Purple, [0xd6, 0x99, 0xb6]),
        ];
        for (slot, [r, g, b]) in expected {
            assert_eq!(Theme::DEFAULT.get(slot), Color::Rgb(r, g, b), "{slot:?}");
        }
    }

    /// Scenario "A theme is swapped" (`ui-design-system` delta): a test theme
    /// with a distinct value per slot, resolved through the real resolvers,
    /// recolors a role and a focus-reactive surface without either changing.
    #[test]
    fn a_swapped_theme_recolors_roles_and_surfaces() {
        let mut colors = [Color::Rgb(0, 0, 0); Slot::COUNT];
        for (value, color) in (0u8..).zip(colors.iter_mut()) {
            *color = Color::Rgb(value, 0xa0, 0x50 + value);
        }
        let theme = Theme(colors);

        let role = Role::PlaybackTitleFg;
        assert_eq!(role.color_in(&theme), colors[role.slot() as usize]);

        // QueueColumn is focus-reactive: resting `Bg1`, focused `Bg2` (D4).
        assert_eq!(
            surface_colors_in(&theme, Surface::QueueColumn, false).fill,
            colors[Slot::Bg1 as usize]
        );
        assert_eq!(
            surface_colors_in(&theme, Surface::QueueColumn, true).fill,
            colors[Slot::Bg2 as usize]
        );
    }

    /// `active()` is the one resolution point and hands back the default.
    #[test]
    fn active_theme_is_the_default() {
        for &slot in Slot::ALL {
            assert_eq!(active().get(slot), Theme::DEFAULT.get(slot));
        }
    }
}
