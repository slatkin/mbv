//! The theme section of a frame: the role-to-colour map and the chrome fills
//! (design D7).
//!
//! `mbv-theme` keeps its role list crate-private, so the GUI mirrors the closed
//! role set here. The two `From` conversions are exhaustive matches, so a role
//! that is added on either side fails to compile until both sides carry it.
//! [`GuiRole::ALL`] lists the mirror in declaration order and is the one list
//! that the compiler does not check: a new role must be added there too.
//!
//! [`GuiTheme::chrome`] carries the fill values the front end's chrome
//! variables point at (the review correction of design D7: the frame once
//! carried foreground roles only, so the chrome could not follow the palette).
//! It needs no mirror of its own: `mbv-theme`'s [`Surface`] set and the
//! [`surface_colors`] resolver are public, so each chrome value resolves the
//! named surface directly and the surface table stays the one fill authority.

use std::collections::BTreeMap;

use mbv_theme::{Rgb, Role, Surface, surface_colors};
use serde::Serialize;

/// The active theme: one colour for every role (design D7) and the chrome
/// fills the front end's shadcn-svelte variables point at. The front end
/// writes each entry into a CSS custom property on `:root`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuiTheme {
    /// The colour of each role, keyed by the role's camelCase name.
    pub roles: BTreeMap<GuiRole, GuiRgb>,
    /// The chrome fills ([`GuiChrome`]).
    pub chrome: GuiChrome,
}

impl GuiTheme {
    /// Resolves every role and chrome fill against the active theme: the
    /// roles with `Role::color`, the chrome through [`surface_colors`].
    #[must_use]
    pub fn current() -> Self {
        Self {
            roles: GuiRole::ALL
                .iter()
                .map(|&role| (role, GuiRgb::from(Role::from(role).color())))
                .collect(),
            chrome: GuiChrome::current(),
        }
    }
}

/// The GUI chrome's fill values (design D7): the fills the front end's
/// shadcn-svelte background, surface, border, and input variables point at.
/// Each one resolves the [`Surface`] the TUI paints the same concept with,
/// so the chrome tracks the palette with no second palette: a palette edit
/// recolours the chrome together with the roles. Fields named without a
/// `focused` suffix are resting fills (`surface_colors(surface, false)`);
/// the two `focused` fields resolve the same surface's focused fill, for
/// the surface table's focus-reactive rows the queue column paints
/// (`QueueColumn`, `QueuePanel`) — the front end picks between the pair on
/// the frame's Panel-focus fact, the way the TUI's
/// `panel_appearance_focus` picks. The fill tiers the front end aliases
/// are named in the field docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GuiChrome {
    /// The app backdrop: `Surface::PlaybackPanel`, which the surface table
    /// fixes at the backdrop in both focus states.
    pub background: GuiRgb,
    /// The content-body card: the resting fill of `Surface::MainContentBox`,
    /// the generic pane inset. The TUI's content boxes are recesses that rest
    /// on the backdrop tier, so this aliases `background`; the card reads
    /// through its border and content, not a fill contrast.
    pub card: GuiRgb,
    /// A popup or dialog body: `Surface::PopupFrame`, the fill every TUI
    /// popup and modal paints.
    pub popover: GuiRgb,
    /// A muted row or stripe: `Surface::ListStripe`, the zebra-stripe tier.
    /// It shares `popover`'s tier today.
    pub muted: GuiRgb,
    /// A control row: `Surface::TransportRow`, the tier the TUI's controls
    /// row owns (one step below the backdrop, per its 2026-10-10 colour
    /// decision).
    pub secondary: GuiRgb,
    /// The sidebar body: `Surface::SidebarBody`.
    pub sidebar: GuiRgb,
    /// A border rule: `Surface::PopupBorder`, the tier of the modal frame's
    /// border rule.
    pub border: GuiRgb,
    /// An input's border: `Surface::PopupBorder`, aliasing `border` — mbv has
    /// one border-rule value, and an input's border is that rule.
    pub input: GuiRgb,
    /// The queue column's backdrop: `Surface::QueueColumn`, the fill the TUI
    /// paints as the queue column's gutter — the padding around the queue
    /// column's recessed box, its playback band, and its footer band. The
    /// resting fill (`surface_colors(surface, false)`).
    pub queue_column: GuiRgb,
    /// The queue column's focused backdrop: `Surface::QueueColumn` resolved
    /// with the focused bit. The queue column is one of the surface table's
    /// focus-reactive rows (resting `Bg1`, focused `Bg2`), so the front end
    /// picks between this and `queue_column` on the frame's Panel-focus fact
    /// the way the TUI's `panel_appearance_focus` picks.
    pub queue_column_focused: GuiRgb,
    /// The queue panel's recessed box: `Surface::QueuePanel`'s resting fill,
    /// the surface the TUI paints the queue list's box with.
    pub queue_panel: GuiRgb,
    /// The queue panel's focused box: `Surface::QueuePanel` resolved with the
    /// focused bit (resting `Bg0`, focused `Bg3`). The front end picks between
    /// this and `queue_panel` on the same Panel-focus fact.
    pub queue_panel_focused: GuiRgb,
    /// The queue column's playback band: `Surface::QueueOnlyPlaybackPanel`,
    /// the fixed chrome-band fill the TUI paints the queue header row and
    /// transport band with (mode-driven, not focus-driven).
    pub queue_playback_band: GuiRgb,
    /// The queue footer's band: `Surface::QueuePanelBand`, the fill the TUI
    /// paints the queue footer row with, behind its status-bar pills.
    pub queue_footer_band: GuiRgb,
}

impl GuiChrome {
    /// Resolves every chrome fill against the active theme.
    #[must_use]
    pub fn current() -> Self {
        Self {
            background: chrome_fill(Surface::PlaybackPanel),
            card: chrome_fill(Surface::MainContentBox),
            popover: chrome_fill(Surface::PopupFrame),
            muted: chrome_fill(Surface::ListStripe),
            secondary: chrome_fill(Surface::TransportRow),
            sidebar: chrome_fill(Surface::SidebarBody),
            border: chrome_fill(Surface::PopupBorder),
            input: chrome_fill(Surface::PopupBorder),
            queue_column: chrome_fill(Surface::QueueColumn),
            queue_column_focused: focused_chrome_fill(Surface::QueueColumn),
            queue_panel: chrome_fill(Surface::QueuePanel),
            queue_panel_focused: focused_chrome_fill(Surface::QueuePanel),
            queue_playback_band: chrome_fill(Surface::QueueOnlyPlaybackPanel),
            queue_footer_band: chrome_fill(Surface::QueuePanelBand),
        }
    }
}

/// The resting fill of `surface`, as the chrome carries it.
fn chrome_fill(surface: Surface) -> GuiRgb {
    GuiRgb::from(surface_colors(surface, false).fill)
}

/// The focused fill of `surface`, as the chrome carries it: the focused
/// variant of the surface table's focus-reactive rows.
fn focused_chrome_fill(surface: Surface) -> GuiRgb {
    GuiRgb::from(surface_colors(surface, true).fill)
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
