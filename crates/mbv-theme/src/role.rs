//! The closed role set (change `theme-slot-model`, design D3).
//!
//! A [`Role`] names what a foreground means, never what hue it is. Variants
//! are the legacy colour constants' names in `UpperCamelCase`; each maps to
//! the slot of the `Palette` variant it painted with (design D1). Call sites
//! resolve with [`Role::color`], which reads the active theme.

use ratatui::style::Color;

use super::slot::{Slot, Theme, active};

/// A semantic foreground colour: text, indicators, scrollbar, underline.
/// Foregrounds resolve only through roles; background fills resolve only
/// through surface rows (`ui-design-language`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Role {
    TextPrimary,
    TextSecondary,
    TextMuted,
    TextStrong,
    TextEmphasis,
    TabSelectedUnderline,
    TextFocusAccent,
    TextHeroTitle,
    WorkspaceHeaderFg,
    TextOnAccent,
    TextAccentMuted,
    PlaylistLoadedFg,
    TextDetailMeta,
    TextMetadata,
    SelectedRowFg,
    SelectedRowProgressFg,
    StatusError,
    StatusAvailable,
    Duration,
    SplitRowContextFg,
    SplitRowTitleFg,
    IndicatorResolutionFg,
    IndicatorAudioFg,
    PlaybackValueFg,
    PlaybackMetaFg,
    PlaybackTitleFg,
    PlaybackContextFg,
    PlaybackHostRemoteFg,
    IdleFeedTitleFg,
    ProgressTrack,
    ProgressPercent,
    GroupHeadingFg,
    EmptyQueueFg,
    Scrollbar,
    SidebarScrollbar,
    HeroOverviewSeparator,
    HeroCreditsName,
    Accent,
    AccentActive,
    AccentAudiobookshelf,
    PillSelectedFg,
    PillOverflowFg,
}

impl Role {
    /// The role's slot: the one role table (design D3).
    pub(crate) const fn slot(self) -> Slot {
        match self {
            Role::TextPrimary => Slot::Fg,
            Role::TextSecondary
            | Role::TextMuted
            | Role::SplitRowTitleFg
            | Role::ProgressTrack
            | Role::SidebarScrollbar => Slot::FgMuted,
            Role::TextStrong => Slot::FgBright,
            Role::TextEmphasis | Role::SplitRowContextFg => Slot::FgWarm,
            Role::TabSelectedUnderline
            | Role::IndicatorAudioFg
            | Role::PlaybackHostRemoteFg
            | Role::EmptyQueueFg => Slot::Purple,
            Role::TextFocusAccent
            | Role::TextHeroTitle
            | Role::PlaybackContextFg
            | Role::HeroCreditsName
            | Role::AccentAudiobookshelf => Slot::Yellow,
            Role::WorkspaceHeaderFg | Role::TextMetadata | Role::GroupHeadingFg => Slot::Blue,
            Role::TextOnAccent => Slot::OnAccent,
            Role::TextAccentMuted | Role::PillOverflowFg => Slot::Bg2,
            Role::PlaylistLoadedFg | Role::IndicatorResolutionFg | Role::ProgressPercent => {
                Slot::Orange
            }
            Role::TextDetailMeta => Slot::FgFaint,
            Role::SelectedRowFg | Role::PillSelectedFg => Slot::BgDim,
            Role::SelectedRowProgressFg => Slot::Bg1,
            Role::StatusError => Slot::Red,
            Role::StatusAvailable => Slot::GreenDeep,
            Role::Duration
            | Role::PlaybackValueFg
            | Role::PlaybackMetaFg
            | Role::IdleFeedTitleFg
            | Role::HeroOverviewSeparator
            | Role::AccentActive => Slot::Green,
            Role::PlaybackTitleFg | Role::Accent => Slot::Aqua,
            Role::Scrollbar => Slot::Bg3,
        }
    }

    /// The role's colour, resolved through the active theme.
    #[must_use]
    pub fn color(self) -> Color {
        self.color_in(active())
    }

    /// The role's colour in `theme` (the theme-swap test resolves through
    /// this; design D3).
    pub(crate) fn color_in(self, theme: &Theme) -> Color {
        theme.get(self.slot())
    }
}

#[cfg(test)]
impl Role {
    /// Every role, in declaration order (`docs/palette.json` generator).
    pub(crate) const ALL: &[Role] = &[
        Role::TextPrimary,
        Role::TextSecondary,
        Role::TextMuted,
        Role::TextStrong,
        Role::TextEmphasis,
        Role::TabSelectedUnderline,
        Role::TextFocusAccent,
        Role::TextHeroTitle,
        Role::WorkspaceHeaderFg,
        Role::TextOnAccent,
        Role::TextAccentMuted,
        Role::PlaylistLoadedFg,
        Role::TextDetailMeta,
        Role::TextMetadata,
        Role::SelectedRowFg,
        Role::SelectedRowProgressFg,
        Role::StatusError,
        Role::StatusAvailable,
        Role::Duration,
        Role::SplitRowContextFg,
        Role::SplitRowTitleFg,
        Role::IndicatorResolutionFg,
        Role::IndicatorAudioFg,
        Role::PlaybackValueFg,
        Role::PlaybackMetaFg,
        Role::PlaybackTitleFg,
        Role::PlaybackContextFg,
        Role::PlaybackHostRemoteFg,
        Role::IdleFeedTitleFg,
        Role::ProgressTrack,
        Role::ProgressPercent,
        Role::GroupHeadingFg,
        Role::EmptyQueueFg,
        Role::Scrollbar,
        Role::SidebarScrollbar,
        Role::HeroOverviewSeparator,
        Role::HeroCreditsName,
        Role::Accent,
        Role::AccentActive,
        Role::AccentAudiobookshelf,
        Role::PillSelectedFg,
        Role::PillOverflowFg,
    ];
}

/// Hero header metadata cycling roles (task 1.2, design D3): the one title/meta
/// painter colours meta row *n* with `HERO_META_ROLES[n % 3]` — the three colours
/// the Emby hero headers already used, defined once so destinations cannot style
/// metadata. The caller resolves the indexed role with `.color()`.
pub const HERO_META_ROLES: [Role; 3] = [
    Role::TextDetailMeta,
    Role::TextMetadata,
    Role::TextSecondary,
];
