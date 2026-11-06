//! The theme section of a frame: the role-to-colour map (design D7).
//!
//! `mbv-theme` keeps its role list crate-private, so the GUI mirrors the closed
//! role set here. The two `From` conversions are exhaustive matches, so a role
//! that is added on either side fails to compile until both sides carry it.
//! [`GuiRole::ALL`] lists the mirror in declaration order and is the one list
//! that the compiler does not check: a new role must be added there too.

use std::collections::BTreeMap;

use mbv_theme::{Rgb, Role};
use serde::Serialize;

/// The active theme: one colour for every role (design D7). The front end writes
/// each entry into a CSS custom property on `:root`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuiTheme {
    /// The colour of each role, keyed by the role's camelCase name.
    pub roles: BTreeMap<GuiRole, GuiRgb>,
}

impl GuiTheme {
    /// Resolves every role against the active theme with `Role::color`.
    #[must_use]
    pub fn current() -> Self {
        Self {
            roles: GuiRole::ALL
                .iter()
                .map(|&role| (role, GuiRgb::from(Role::from(role).color())))
                .collect(),
        }
    }
}

/// An 8-bit RGB colour, mirroring `mbv_theme::Rgb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GuiRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl From<Rgb> for GuiRgb {
    fn from(colour: Rgb) -> Self {
        Self {
            r: colour.r,
            g: colour.g,
            b: colour.b,
        }
    }
}

/// A semantic foreground colour role, mirroring `mbv_theme::Role` variant for
/// variant. The front end names each CSS custom property after the role's
/// camelCase name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiRole {
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
    PlaybackTargetLocalFg,
    PlaybackTargetRemoteFg,
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

impl GuiRole {
    /// Every role, in declaration order. The theme map carries one entry per
    /// role, so this list has the same 44 roles as the mirror above.
    pub const ALL: [Self; 44] = [
        Self::TextPrimary,
        Self::TextSecondary,
        Self::TextMuted,
        Self::TextStrong,
        Self::TextEmphasis,
        Self::TabSelectedUnderline,
        Self::TextFocusAccent,
        Self::TextHeroTitle,
        Self::WorkspaceHeaderFg,
        Self::TextOnAccent,
        Self::TextAccentMuted,
        Self::PlaylistLoadedFg,
        Self::TextDetailMeta,
        Self::TextMetadata,
        Self::SelectedRowFg,
        Self::SelectedRowProgressFg,
        Self::StatusError,
        Self::StatusAvailable,
        Self::Duration,
        Self::SplitRowContextFg,
        Self::SplitRowTitleFg,
        Self::IndicatorResolutionFg,
        Self::IndicatorAudioFg,
        Self::PlaybackValueFg,
        Self::PlaybackMetaFg,
        Self::PlaybackTitleFg,
        Self::PlaybackContextFg,
        Self::PlaybackHostRemoteFg,
        Self::PlaybackTargetLocalFg,
        Self::PlaybackTargetRemoteFg,
        Self::IdleFeedTitleFg,
        Self::ProgressTrack,
        Self::ProgressPercent,
        Self::GroupHeadingFg,
        Self::EmptyQueueFg,
        Self::Scrollbar,
        Self::SidebarScrollbar,
        Self::HeroOverviewSeparator,
        Self::HeroCreditsName,
        Self::Accent,
        Self::AccentActive,
        Self::AccentAudiobookshelf,
        Self::PillSelectedFg,
        Self::PillOverflowFg,
    ];
}

impl From<Role> for GuiRole {
    fn from(role: Role) -> Self {
        match role {
            Role::TextPrimary => Self::TextPrimary,
            Role::TextSecondary => Self::TextSecondary,
            Role::TextMuted => Self::TextMuted,
            Role::TextStrong => Self::TextStrong,
            Role::TextEmphasis => Self::TextEmphasis,
            Role::TabSelectedUnderline => Self::TabSelectedUnderline,
            Role::TextFocusAccent => Self::TextFocusAccent,
            Role::TextHeroTitle => Self::TextHeroTitle,
            Role::WorkspaceHeaderFg => Self::WorkspaceHeaderFg,
            Role::TextOnAccent => Self::TextOnAccent,
            Role::TextAccentMuted => Self::TextAccentMuted,
            Role::PlaylistLoadedFg => Self::PlaylistLoadedFg,
            Role::TextDetailMeta => Self::TextDetailMeta,
            Role::TextMetadata => Self::TextMetadata,
            Role::SelectedRowFg => Self::SelectedRowFg,
            Role::SelectedRowProgressFg => Self::SelectedRowProgressFg,
            Role::StatusError => Self::StatusError,
            Role::StatusAvailable => Self::StatusAvailable,
            Role::Duration => Self::Duration,
            Role::SplitRowContextFg => Self::SplitRowContextFg,
            Role::SplitRowTitleFg => Self::SplitRowTitleFg,
            Role::IndicatorResolutionFg => Self::IndicatorResolutionFg,
            Role::IndicatorAudioFg => Self::IndicatorAudioFg,
            Role::PlaybackValueFg => Self::PlaybackValueFg,
            Role::PlaybackMetaFg => Self::PlaybackMetaFg,
            Role::PlaybackTitleFg => Self::PlaybackTitleFg,
            Role::PlaybackContextFg => Self::PlaybackContextFg,
            Role::PlaybackHostRemoteFg => Self::PlaybackHostRemoteFg,
            Role::PlaybackTargetLocalFg => Self::PlaybackTargetLocalFg,
            Role::PlaybackTargetRemoteFg => Self::PlaybackTargetRemoteFg,
            Role::IdleFeedTitleFg => Self::IdleFeedTitleFg,
            Role::ProgressTrack => Self::ProgressTrack,
            Role::ProgressPercent => Self::ProgressPercent,
            Role::GroupHeadingFg => Self::GroupHeadingFg,
            Role::EmptyQueueFg => Self::EmptyQueueFg,
            Role::Scrollbar => Self::Scrollbar,
            Role::SidebarScrollbar => Self::SidebarScrollbar,
            Role::HeroOverviewSeparator => Self::HeroOverviewSeparator,
            Role::HeroCreditsName => Self::HeroCreditsName,
            Role::Accent => Self::Accent,
            Role::AccentActive => Self::AccentActive,
            Role::AccentAudiobookshelf => Self::AccentAudiobookshelf,
            Role::PillSelectedFg => Self::PillSelectedFg,
            Role::PillOverflowFg => Self::PillOverflowFg,
        }
    }
}

impl From<GuiRole> for Role {
    fn from(role: GuiRole) -> Self {
        match role {
            GuiRole::TextPrimary => Self::TextPrimary,
            GuiRole::TextSecondary => Self::TextSecondary,
            GuiRole::TextMuted => Self::TextMuted,
            GuiRole::TextStrong => Self::TextStrong,
            GuiRole::TextEmphasis => Self::TextEmphasis,
            GuiRole::TabSelectedUnderline => Self::TabSelectedUnderline,
            GuiRole::TextFocusAccent => Self::TextFocusAccent,
            GuiRole::TextHeroTitle => Self::TextHeroTitle,
            GuiRole::WorkspaceHeaderFg => Self::WorkspaceHeaderFg,
            GuiRole::TextOnAccent => Self::TextOnAccent,
            GuiRole::TextAccentMuted => Self::TextAccentMuted,
            GuiRole::PlaylistLoadedFg => Self::PlaylistLoadedFg,
            GuiRole::TextDetailMeta => Self::TextDetailMeta,
            GuiRole::TextMetadata => Self::TextMetadata,
            GuiRole::SelectedRowFg => Self::SelectedRowFg,
            GuiRole::SelectedRowProgressFg => Self::SelectedRowProgressFg,
            GuiRole::StatusError => Self::StatusError,
            GuiRole::StatusAvailable => Self::StatusAvailable,
            GuiRole::Duration => Self::Duration,
            GuiRole::SplitRowContextFg => Self::SplitRowContextFg,
            GuiRole::SplitRowTitleFg => Self::SplitRowTitleFg,
            GuiRole::IndicatorResolutionFg => Self::IndicatorResolutionFg,
            GuiRole::IndicatorAudioFg => Self::IndicatorAudioFg,
            GuiRole::PlaybackValueFg => Self::PlaybackValueFg,
            GuiRole::PlaybackMetaFg => Self::PlaybackMetaFg,
            GuiRole::PlaybackTitleFg => Self::PlaybackTitleFg,
            GuiRole::PlaybackContextFg => Self::PlaybackContextFg,
            GuiRole::PlaybackHostRemoteFg => Self::PlaybackHostRemoteFg,
            GuiRole::PlaybackTargetLocalFg => Self::PlaybackTargetLocalFg,
            GuiRole::PlaybackTargetRemoteFg => Self::PlaybackTargetRemoteFg,
            GuiRole::IdleFeedTitleFg => Self::IdleFeedTitleFg,
            GuiRole::ProgressTrack => Self::ProgressTrack,
            GuiRole::ProgressPercent => Self::ProgressPercent,
            GuiRole::GroupHeadingFg => Self::GroupHeadingFg,
            GuiRole::EmptyQueueFg => Self::EmptyQueueFg,
            GuiRole::Scrollbar => Self::Scrollbar,
            GuiRole::SidebarScrollbar => Self::SidebarScrollbar,
            GuiRole::HeroOverviewSeparator => Self::HeroOverviewSeparator,
            GuiRole::HeroCreditsName => Self::HeroCreditsName,
            GuiRole::Accent => Self::Accent,
            GuiRole::AccentActive => Self::AccentActive,
            GuiRole::AccentAudiobookshelf => Self::AccentAudiobookshelf,
            GuiRole::PillSelectedFg => Self::PillSelectedFg,
            GuiRole::PillOverflowFg => Self::PillOverflowFg,
        }
    }
}
