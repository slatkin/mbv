//! Semantic theme roles and palette values for every painted surface.
//!
//! Callers name what a colour means (`TEXT_METADATA`, `SURFACE_FOCUSED`), never the
//! hue; the closed `Palette` enum stays private. Layout and painting live in
//! `mbv-render`, which consumes these roles.

#[cfg(test)]
mod palette_json;
mod role;
mod slot;
mod surface;
mod surface_resolve;
mod surface_table;

#[doc(inline)]
pub use role::{HERO_META_ROLES, Role};
#[doc(inline)]
pub use surface::{HINT_CHIPS, Surface};
#[doc(inline)]
pub use surface_resolve::surface_colors;

use ratatui::style::Color;

use slot::{Slot, Theme};

// ---------------------------------------------------------------------------
// Transitional aliases (theme-slot-model task 1.1) — the legacy `pub const
// ROLE: Color` names, each resolving its slot through the default theme so
// the workspace keeps building while call sites migrate. Task 7.1 deletes
// this block.
// ---------------------------------------------------------------------------

// Surfaces
pub const SURFACE_BACKDROP: Color = Theme::DEFAULT.get(Slot::Bg0);
pub const SURFACE_CHROME: Color = Theme::DEFAULT.get(Slot::BgDim);
/// The focused surface's own fill; deliberately not the text-green
/// `TEXT_ACCENT_MUTED`, whose `Palette::Green1` value it shares today: the
/// two are equal today and independently editable, so a text-colour edit
/// moves the text alone (and vice versa; unify-surface-colour-neutral
/// task 4.2).
pub const SURFACE_FOCUSED: Color = Theme::DEFAULT.get(Slot::Bg2);
pub const SURFACE_RESTING: Color = Theme::DEFAULT.get(Slot::Bg1); // resting-content / unfocused half
/// Expanded (F1-F4) sidebar body fill (the chrome panel shell's content
/// area; the header/footer band keeps its own ink). Its own role over the
/// scrollbar's `Palette::Green2` value it shares today (`SCROLLBAR`): the
/// two are equal today and independently editable, so a scrollbar edit
/// moves the scrollbar alone.
pub const SURFACE_SIDEBAR: Color = Theme::DEFAULT.get(Slot::Bg3);

// Hero surfaces
pub const HERO_OVERVIEW_SEPARATOR: Color = Theme::DEFAULT.get(Slot::Green); // overview/credits separator
pub const HERO_CREDITS_STRIPE: Color = Theme::DEFAULT.get(Slot::Bg1); // alternating credits row
pub const HERO_CREDITS_NAME: Color = Theme::DEFAULT.get(Slot::Yellow); // credits name column

// Accents
pub const ACCENT: Color = Theme::DEFAULT.get(Slot::Aqua); // focus accent, watched, folders, Emby brand glyph
pub const ACCENT_ACTIVE: Color = Theme::DEFAULT.get(Slot::Green); // active tab, focused pill/badge text, selected-row bg
pub const ACCENT_AUDIOBOOKSHELF: Color = Theme::DEFAULT.get(Slot::Yellow); // audiobookshelf brand glyph

// Rules

// Pill selector (renamed without value changes)
pub const PILL_ROW_BG: Color = Theme::DEFAULT.get(Slot::BgDim);
pub const PILL_BG: Color = Theme::DEFAULT.get(Slot::BgDim);
pub const PILL_SELECTED_BG: Color = Theme::DEFAULT.get(Slot::Blue); // selected pill-selector surface (#3a94c5);
// shares `TEXT_METADATA`'s `Palette::Foam`
// value today, kept separate from the
// metadata text: the two are equal today
// and independently editable, so an edit
// to either moves it alone
pub const PILL_SELECTED_FG: Color = Theme::DEFAULT.get(Slot::BgDim);
/// The pill-selector's overflow/edge accent. A former value alias of the
/// text green (`PILL_SELECTOR_OVERFLOW_FG = BG_GREEN`), now its own role: it
/// shares `TEXT_ACCENT_MUTED`'s `Palette::Green1` value today — the two are
/// equal today and independently editable, so an edit to either moves it
/// alone.
pub const PILL_OVERFLOW_FG: Color = Theme::DEFAULT.get(Slot::Bg2);

// Text: readable content foregrounds, independent of surface
pub const TEXT_PRIMARY: Color = Theme::DEFAULT.get(Slot::Fg);
pub const TEXT_SECONDARY: Color = Theme::DEFAULT.get(Slot::FgMuted); // paired with TEXT_PRIMARY/TEXT_STRONG
pub const TEXT_MUTED: Color = Theme::DEFAULT.get(Slot::FgMuted); // dim text, icons, unfocused state
/// A played media-list row's title (single-part primary, or a played split
/// row's item title; the split context keeps `SPLIT_ROW_CONTEXT_FG`). Its own
/// role rather than the generic dim `TEXT_MUTED`, so a played row can read as
/// watched without moving icons and unfocused chrome.
pub const TEXT_STRONG: Color = Theme::DEFAULT.get(Slot::FgBright); // bold titles/headings
pub const TEXT_EMPHASIS: Color = Theme::DEFAULT.get(Slot::FgWarm); // warm emphasis text (focused rows, dialogs)
/// The selected tab's underline run in the tab bar (upper-eighth blocks on
/// the row below the tab text). Currently experimenting with Mauve
/// (2026-10-05); it replaced the underline's original `TEXT_EMPHASIS` value.
pub const TAB_SELECTED_UNDERLINE: Color = Theme::DEFAULT.get(Slot::Purple);
pub const TEXT_FOCUS_ACCENT: Color = Theme::DEFAULT.get(Slot::Yellow); // focused-row title accent
pub const TEXT_HERO_TITLE: Color = Theme::DEFAULT.get(Slot::Yellow); // hero header title (the first metadata line)
/// The Workspace box's header label (`TRACKLIST`) in a music album Hero. Its
/// own role rather than `MUSIC_HEADER`, the music tree's artist/section role
/// it used to borrow: this one row reads as a label, not as the tree's
/// emphasis text, and an emphasis edit must not move it. Deliberately its own
/// over the shared metadata blue it matches today (`TEXT_METADATA`, and the
/// `PILL_SELECTED_BG` chip fill): equal today and independently editable, so
/// a metadata-colour edit moves metadata alone.
pub const WORKSPACE_HEADER_FG: Color = Theme::DEFAULT.get(Slot::Blue);
pub const TEXT_ON_ACCENT: Color = Theme::DEFAULT.get(Slot::OnAccent); // near-black text painted on a colored surface
pub const TEXT_ACCENT_MUTED: Color = Theme::DEFAULT.get(Slot::Bg2); // "loaded"/"playing"/confirmed value text;
// deliberately not the focused surface's
// `SURFACE_FOCUSED`, whose `Palette::Green1`
// value it shares today: the two are equal
// today and independently editable, so a
// text-colour edit moves the text alone
// (unify-surface-colour-neutral task 4.2)
/// A playlist row whose playlist is currently loaded in the queue (F4 list).
/// Its own role over the progress orange it shares today
/// (`PROGRESS_PERCENT`): the loaded marker must read on the slate panel
/// surface, which `TEXT_ACCENT_MUTED`'s dark green does not — equal today
/// and independently editable, so a progress-colour edit moves progress alone.
pub const PLAYLIST_LOADED_FG: Color = Theme::DEFAULT.get(Slot::Orange);
/// The F4 playlists list's secondary zebra fill (`#2e383c`). Its own role
/// over the focused-surface green it shares today (`SURFACE_FOCUSED`): equal
/// today and independently editable, so a focus-fill edit moves focus alone.
pub const PLAYLIST_STRIPE_BG: Color = Theme::DEFAULT.get(Slot::Bg2);
/// The settings list's secondary zebra fill (`#2e383c`). Its own role over
/// the playlist stripe and focused-surface green it shares today
/// (`PLAYLIST_STRIPE_BG`, `SURFACE_FOCUSED`): equal today and independently
/// editable, so those edits move theirs alone.
pub const SETTINGS_STRIPE_BG: Color = Theme::DEFAULT.get(Slot::Bg2);
/// The F3 sessions sidebar list's secondary zebra fill (`#2e383c`). Its own
/// role replacing the sidebar-band chrome the list painted before
/// (`SURFACE_CHROME`, `#1e2326` ink), independently editable from that chrome
/// and the other lists' stripes.
pub const SESSIONS_STRIPE_BG: Color = Theme::DEFAULT.get(Slot::Bg2);
/// The TV Workspace box's focused fill (`#374145`): the focused Workspace
/// body, one step lighter than its resting `MainContentBox` backdrop.
pub const WORKSPACE_FOCUSED_FILL: Color = Theme::DEFAULT.get(Slot::Bg3);
/// The TV Workspace list's focused secondary zebra fill (`#2e383c`): the
/// stripe the focused Workspace's rows alternate against
/// [`WORKSPACE_FOCUSED_FILL`].
pub const WORKSPACE_FOCUSED_STRIPE: Color = Theme::DEFAULT.get(Slot::Bg2);
pub const TEXT_DETAIL_META: Color = Theme::DEFAULT.get(Slot::FgFaint); // detail-screen label/meta text
pub const TEXT_METADATA: Color = Theme::DEFAULT.get(Slot::Blue); // secondary metadata (durations, badges)
/// Selected-row bar fill (audition: an opaque full-width bar replaces the
/// punch-through and the gutter-accent-only title treatment).
pub const SELECTED_ROW_BG: Color = Theme::DEFAULT.get(Slot::Green);
/// Selected-row text on the Iris bar. Its own role keeps the bar's foreground
/// independent from the ordinary title and metadata hierarchy.
pub const SELECTED_ROW_FG: Color = Theme::DEFAULT.get(Slot::BgDim);
/// The resume-progress percentage of a selected row: the bar's darker
/// companion to `SELECTED_ROW_FG`, because the ordinary progress orange
/// (`PROGRESS_PERCENT`) does not read on the light Iris bar. Its own role
/// rather than the surfaces that share its `Palette::Storm` value today
/// (`SURFACE_RESTING`, `HERO_CREDITS_STRIPE`): equal today and independently
/// editable, so a resting-surface edit moves the surface alone.
pub const SELECTED_ROW_PROGRESS_FG: Color = Theme::DEFAULT.get(Slot::Bg1);
/// The selected/connected-bar text policy: a row painted over the opaque
/// `SELECTED_ROW_BG` bar renders in the bar's own foreground because
/// ordinary text roles do not read on the light fill; elsewhere a row keeps
/// the role it was given. Shared by the media list and the sessions sidebar,
/// the two surfaces that paint this bar.
#[must_use]
pub fn bar_role_fg(role: Color, on_bar: bool) -> Color {
    if on_bar {
        Role::SelectedRowFg.color()
    } else {
        role
    }
}

/// The Library Hero overlay's display-only hint chips' fills, rotated by chip
/// index (foam, yellow, orange, repeating). A role set of its own rather than
/// borrowed text roles: these are chip *surfaces*, and a text-colour edit must
/// not move them. Transitional alias; [`HINT_CHIPS`] replaces it (design D5).
pub const HINT_PILL_FILLS: [Color; 3] = [
    Theme::DEFAULT.get(Slot::Blue),
    Theme::DEFAULT.get(Slot::Yellow),
    Theme::DEFAULT.get(Slot::Orange),
];

// Status
pub const STATUS_ERROR: Color = Theme::DEFAULT.get(Slot::Red);
pub const STATUS_AVAILABLE: Color = Theme::DEFAULT.get(Slot::GreenDeep); // checkmarks, available/positive metadata

// Media list metadata
/// The right-aligned duration column of the Queue list. Its own role rather
/// than the green `STATUS_AVAILABLE` metadata role, so a status-colour edit
/// cannot move the durations. Only the Queue list projects a duration; library
/// browse rows carry no time.
pub const DURATION: Color = Theme::DEFAULT.get(Slot::Green); // the sage

/// The primary (context/container) part of a split media-list row — the
/// podcast an episode row came from. Its own role rather than
/// `PLAYBACK_CONTEXT_FG`, the playback strip's context role: browse lists
/// paint their split-row context in the soft-white emphasis colour while the
/// strip keeps gold.
pub const SPLIT_ROW_CONTEXT_FG: Color = Theme::DEFAULT.get(Slot::FgWarm); // soft white (#faedcd)

/// The secondary title of a split media-list row — the item's own name after
/// the container/context (which paints `SPLIT_ROW_CONTEXT_FG`). Its own role
/// rather than `PLAYBACK_TITLE_FG`, the playback strip's own-name role:
/// browse lists paint their split-row titles in a light grey while the strip
/// keeps aqua.
pub const SPLIT_ROW_TITLE_FG: Color = Theme::DEFAULT.get(Slot::FgMuted); // light grey (#9e9e9e)

// Media indicators (resolution/audio glyphs)
pub const INDICATOR_RESOLUTION_FG: Color = Theme::DEFAULT.get(Slot::Orange);
pub const INDICATOR_AUDIO_FG: Color = Theme::DEFAULT.get(Slot::Purple);

// Playback panel
pub const PLAYBACK_VALUE_FG: Color = Theme::DEFAULT.get(Slot::Green); // title/codec value
pub const PLAYBACK_META_FG: Color = Theme::DEFAULT.get(Slot::Green); // captions/time
/// The now-playing title row's title part: the item's own name (episode,
/// track, entry, ...). Its own role rather than `ACCENT`/`TEXT_FOCUS_ACCENT`,
/// whose `Palette::Aqua` value it shares today: the two are equal today and
/// independently editable, so a focus-accent or brand edit moves the accent
/// alone (now-playing-media-type-titles D2).
pub const PLAYBACK_TITLE_FG: Color = Theme::DEFAULT.get(Slot::Aqua);
/// The now-playing title row's context part: the container the item came
/// from (series, artist, show, subscription). Its own role rather than the
/// focused-row accent, whose `Palette::Yellow` value it shares today: the
/// two are equal today and independently editable, so an accent edit moves
/// the accent alone (now-playing-media-type-titles D2).
pub const PLAYBACK_CONTEXT_FG: Color = Theme::DEFAULT.get(Slot::Yellow);
/// The now-playing header's remote target name: the device or route the
/// media plays on, painted after `on`. Its own role rather than the audio
/// indicator's `Palette::Mauve` value it shares today
/// (`INDICATOR_AUDIO_FG`): the two are equal today and independently
/// editable, so an indicator edit moves the indicator alone.
pub const PLAYBACK_HOST_REMOTE_FG: Color = Theme::DEFAULT.get(Slot::Purple);
/// The idle-feed marquee title: the RSS entry the strip shows while nothing
/// is playing. Its own role rather than `ACCENT`, whose `Palette::Aqua`
/// value it painted before (user decision 2026-10-05): the idle title is
/// ambience, not a focus accent, so it takes the foam blue and an accent
/// edit can never move it.
pub const IDLE_FEED_TITLE_FG: Color = Theme::DEFAULT.get(Slot::Green);

// Progress and queue
pub const PROGRESS_TRACK: Color = Theme::DEFAULT.get(Slot::FgMuted); // unplayed seek/progress track
/// The resume-progress percentage text (`47%`) of a media-list row and of a
/// hero metadata row. Its own role rather than the shared metadata blue
/// (`TEXT_METADATA`, `Palette::Foam`) it used to take, and deliberately its
/// own role over the resolution glyph's `Palette::Orange` value it shares
/// today (`INDICATOR_RESOLUTION_FG`): the two are equal today and
/// independently editable, so an indicator edit moves the indicator alone.
pub const PROGRESS_PERCENT: Color = Theme::DEFAULT.get(Slot::Orange);
/// Group headings (`MediaListRow::Heading`) in every grouped media list. Its
/// own role over the secondary metadata blue it shares today
/// (`TEXT_METADATA`): equal today and independently editable, so a
/// metadata-colour edit moves the metadata alone.
pub const GROUP_HEADING_FG: Color = Theme::DEFAULT.get(Slot::Blue);
/// The empty-queue placeholder (`¯\_(ツ)_/¯`). Its own role over the audio
/// indicator's `Palette::Mauve` value it shares today
/// (`INDICATOR_AUDIO_FG`): the two are equal today and independently
/// editable, so an indicator edit moves the indicator alone.
pub const EMPTY_QUEUE_FG: Color = Theme::DEFAULT.get(Slot::Purple);

// Chrome
/// Library/chrome scrollbar track/thumb; a former value alias of the soft
/// content-body fill (`SCROLLBAR = BG_GREEN_SOFT`), now its own role:
/// deliberately not the soft content-body surface, whose `Palette::Green2`
/// value it shares today — the two are equal today and independently
/// editable, so a scrollbar edit moves the scrollbar alone
/// (unify-surface-colour-neutral task 4.2).
pub const SCROLLBAR: Color = Theme::DEFAULT.get(Slot::Bg3);
/// Sidebar scrollbar track/thumb. Its own role over the dim-text grey it
/// shares today (`TEXT_MUTED`): the sidebar bodies paint the scrollbar's
/// own `Palette::Green2` value, so the shared chrome scrollbar would vanish
/// against them — equal today and independently editable, so a dim-text
/// edit moves dim text alone.
pub const SIDEBAR_SCROLLBAR: Color = Theme::DEFAULT.get(Slot::FgMuted);
