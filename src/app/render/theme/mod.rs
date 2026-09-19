mod palette;
mod primitives;
mod surface;
mod surface_resolve;
mod surface_table;

use self::palette::Palette;

// The closed palette enum (openspec/changes/palette-enum). Section 3 migrates
// the roles/surfaces onto it: every role below is a compiler-visible
// assignment to a `Palette` variant, and `Palette::color()` (palette.rs) is
// the only place a palette `Rgb` literal lives in theme code.

// The closed surface table (`unify-surface-colour-neutral` D1/D2/D7). A screen
// names a `Surface` and calls the resolver; the `Level`/`Row`/`FocusSource`
// machinery stays private to the theme. Re-exported here so `render/mod.rs` and
// `palette.rs` can bridge the names to production call sites.
pub(in crate::app) use surface::Surface;
pub(in crate::app) use surface_resolve::surface_colors;

// Every `Color` use in this module is a paint-boundary or frozen-test shim
// conversion; the roles themselves are `Palette` assignments.
#[cfg(test)]
use ratatui::style::Color;

// ---------------------------------------------------------------------------
// Roles (openspec/changes/centralize-ui-design-language,
// openspec/changes/enforce-mbv-ui-design-system) — the public API. A role
// names what a colour *means*, never what hue it is. Each role is assigned a
// `Palette` variant (palette.rs), so nothing outside `theme` can name a hue
// directly.
// ---------------------------------------------------------------------------

// Surfaces
pub(in crate::app) const SURFACE_BACKDROP: Palette = Palette::Slate;
pub(in crate::app) const SURFACE_CHROME: Palette = Palette::Ink;
pub(in crate::app) const SURFACE_FOCUSED: Palette = Palette::Green1;
pub(in crate::app) const SURFACE_RESTING: Palette = Palette::Storm; // resting-content / unfocused half
                                                                    // Transitional: retired from production by task 4.2 (`SURFACE_PLAYBACK` was
                                                                    // `SURFACE_RESTING`'s value alias); the name stays reachable only because the
                                                                    // frozen `render/tests.rs` pins it, which the neutrality rule forbids editing.
#[cfg(test)]
#[allow(dead_code)]
pub(in crate::app) const SURFACE_PLAYBACK: Color = Palette::Storm.color(); // now-playing-strip half
                                                                           // Transitional: retired from production by task 4.2 (`SURFACE_ACCENT_SOFT`'s
                                                                           // value duplicated `SCROLLBAR`'s; the soft variant resolves through
                                                                           // `SOFT_CONTENT_BODY_BG` now); the name stays reachable only because three
                                                                           // frozen test files pin it, which the neutrality rule forbids editing.
#[cfg(test)]
#[allow(dead_code)]
pub(in crate::app) const SURFACE_ACCENT_SOFT: Color = Palette::Green2.color();
pub(in crate::app) const SURFACE_ITEM_FOCUSED: Palette = Palette::Grey3;
pub(in crate::app) const SURFACE_SIDEBAR: Palette = Palette::Flint; // plain (non-hero) sidebar/panel background
                                                                    // Transitional: retired from production by task 4.2 (`SURFACE_ARTWORK_PLACEHOLDER`
                                                                    // was `SURFACE_BACKDROP`'s value alias); the name stays reachable only because
                                                                    // the frozen `artwork_placeholder_tests.rs` pins it.
#[cfg(test)]
#[allow(dead_code)]
pub(in crate::app) const SURFACE_ARTWORK_PLACEHOLDER: Color = Palette::Slate.color();

// Hero surfaces
pub(in crate::app) const HERO_OVERVIEW_SEPARATOR: Palette = Palette::Iris; // overview/credits separator
pub(in crate::app) const HERO_CREDITS_STRIPE: Palette = Palette::Storm; // alternating credits row
pub(in crate::app) const HERO_CREDITS_NAME: Palette = Palette::Yellow; // credits name column

// Accents
pub(in crate::app) const ACCENT: Palette = Palette::Aqua; // focus accent, watched, folders, Emby brand glyph
pub(in crate::app) const ACCENT_ACTIVE: Palette = Palette::Iris; // active tab, focused pill/badge text, selected-row bg
pub(in crate::app) const ACCENT_AUDIOBOOKSHELF: Palette = Palette::Gold; // audiobookshelf brand glyph

// Rules
pub(in crate::app) const BORDER_UNFOCUSED: Palette = Palette::Grey2;

// Pill selector (renamed without value changes)
pub(in crate::app) const PILL_ROW_BG: Palette = Palette::Ink;
pub(in crate::app) const PILL_BG: Palette = Palette::Ink;
pub(in crate::app) const PILL_SELECTED_BG: Palette = Palette::Foam;
pub(in crate::app) const PILL_FG: Palette = Palette::Ash;
pub(in crate::app) const PILL_SELECTED_FG: Palette = Palette::Ink;
pub(in crate::app) const PILL_OVERFLOW_FG: Palette = Palette::Green1;

// Text: readable content foregrounds, independent of surface
pub(in crate::app) const TEXT_PRIMARY: Palette = Palette::Grey6;
pub(in crate::app) const TEXT_SECONDARY: Palette = Palette::Grey5; // paired with TEXT_PRIMARY/TEXT_STRONG
pub(in crate::app) const TEXT_MUTED: Palette = Palette::Grey4; // dim text, icons, unfocused state
/// A played media-list row's title (single-part primary, or a played split
/// row's item title; the split context keeps `SPLIT_ROW_CONTEXT_FG`). Its own
/// role rather than the generic dim `TEXT_MUTED`, so a played row can read as
/// watched without moving icons and unfocused chrome.
pub(in crate::app) const PLAYED_ROW_FG: Palette = Palette::Fog; // #bec5b2
pub(in crate::app) const TEXT_STRONG: Palette = Palette::White; // bold titles/headings
pub(in crate::app) const TEXT_EMPHASIS: Palette = Palette::Cream; // warm emphasis text (focused rows, dialogs)
pub(in crate::app) const TEXT_FOCUS_ACCENT: Palette = Palette::Yellow; // focused-row title accent
pub(in crate::app) const TEXT_HERO_TITLE: Palette = Palette::Yellow; // hero header title (the first metadata line)
pub(in crate::app) const TEXT_ON_ACCENT: Palette = Palette::Grey1; // near-black text painted on a colored surface
pub(in crate::app) const TEXT_ACCENT_MUTED: Palette = Palette::Green1; // "loaded"/"playing"/confirmed value text; deliberately
                                                                       // not the focused surface's `SURFACE_FOCUSED_BG` (task 4.2)
pub(in crate::app) const TEXT_DETAIL_META: Palette = Palette::Green3; // detail-screen label/meta text
pub(in crate::app) const TEXT_METADATA: Palette = Palette::Foam; // secondary metadata (durations, pct, badges)
/// Selected-row bar fill (audition: an opaque full-width bar replaces the
/// punch-through and the gutter-accent-only title treatment; the row keeps
/// its ordinary foreground roles on the bar).
pub(in crate::app) const SELECTED_ROW_BG: Palette = Palette::Slate;

// Hero header metadata cycling roles (task 5.5, design D5): the one title/meta
// painter colours meta row *n* with `HERO_META_ROLES[n % 3]` — the three colours
// the Emby hero headers already used, defined once so destinations cannot style
// metadata.
pub(in crate::app) const HERO_META_ROLES: [Palette; 3] =
    [TEXT_DETAIL_META, TEXT_METADATA, TEXT_SECONDARY];

/// The Library Hero overlay's display-only hint chips' fills, rotated by chip
/// index (foam, yellow, orange, repeating). A role set of its own rather than
/// borrowed text roles: these are chip *surfaces*, and a text-colour edit must
/// not move them.
pub(in crate::app) const HINT_PILL_FILLS: [Palette; 3] =
    [Palette::Foam, Palette::Yellow, Palette::Orange];

// Status
pub(in crate::app) const STATUS_ERROR: Palette = Palette::Red;
pub(in crate::app) const STATUS_AVAILABLE: Palette = Palette::Green; // checkmarks, available/positive metadata

// Media list metadata
/// The right-aligned duration column of the Queue list. Its own role rather
/// than the green `STATUS_AVAILABLE` metadata role, so a status-colour edit
/// cannot move the durations. Only the Queue list projects a duration; library
/// browse rows carry no time.
pub(in crate::app) const DURATION: Palette = Palette::Iris; // the sage

/// The right-aligned publish-date column of a library browse row — the
/// podcast browser's `17 Sep 26` gutter. Its own role rather than the
/// `DURATION` time column's, so a duration edit cannot move the dates; a date
/// is metadata about the item, not a playback time.
pub(in crate::app) const ROW_DATE_FG: Palette = Palette::Yellow; // muted gold (#dbbc7f)

/// The primary (context/container) part of a split media-list row — the
/// podcast an episode row came from. Its own role rather than
/// `PLAYBACK_CONTEXT_FG`, the playback strip's context role: browse lists
/// paint their split-row context in the soft-white emphasis colour while the
/// strip keeps gold.
pub(in crate::app) const SPLIT_ROW_CONTEXT_FG: Palette = Palette::Cream; // soft white (#faedcd)

/// The secondary title of a split media-list row — the item's own name after
/// the container/context (which paints `SPLIT_ROW_CONTEXT_FG`). Its own role
/// rather than `PLAYBACK_TITLE_FG`, the playback strip's own-name role:
/// browse lists paint their split-row titles in a light grey while the strip
/// keeps aqua.
pub(in crate::app) const SPLIT_ROW_TITLE_FG: Palette = Palette::Grey5; // light grey (#9e9e9e)

// Media indicators (resolution/audio glyphs)
pub(in crate::app) const INDICATOR_RESOLUTION_FG: Palette = Palette::Orange;
pub(in crate::app) const INDICATOR_AUDIO_FG: Palette = Palette::Purple;

// Playback panel
pub(in crate::app) const PLAYBACK_VALUE_FG: Palette = Palette::Mint; // title/codec value
pub(in crate::app) const PLAYBACK_META_FG: Palette = Palette::Sage; // captions/time
/// The now-playing title row's title part: the item's own name (episode,
/// track, entry, ...). Its own role rather than `ACCENT`/`TEXT_FOCUS_ACCENT`,
/// whose primitives it shares values with today, so a focus-accent or brand
/// edit cannot move it (now-playing-media-type-titles D2).
pub(in crate::app) const PLAYBACK_TITLE_FG: Palette = Palette::Aqua;
/// The now-playing title row's context part: the container the item came
/// from (series, artist, show, subscription). Its own role rather than the
/// focused-row accent, whose primitive it shares a value with today, so an
/// accent edit cannot move it (now-playing-media-type-titles D2).
pub(in crate::app) const PLAYBACK_CONTEXT_FG: Palette = Palette::Yellow;

// Progress and queue
pub(in crate::app) const PROGRESS_TRACK: Palette = Palette::Steel; // unplayed seek/progress track

// Chrome
pub(in crate::app) const SCROLLBAR: Palette = Palette::Green2;

/// The central focus lever (design decision 8). Every panel and component
/// resolves its focused/unfocused surface through this single function
/// instead of naming `SURFACE_FOCUSED`/`SURFACE_RESTING` at the call site.
///
/// `focused` is the caller's two-input focus model already collapsed to one
/// bool: the existing `PanelFocus` (which panel is focused) for
/// inline screens with one focusable region, or `PanelFocus` combined
/// with a pane bit (`left_focused`) for Wide hero screens with two.
// Transitional: `resolve_surface_focus` is the value-aliased resolver the
// surface table replaced; every production caller was migrated, but the
// frozen pre-existing tests still name it (the neutrality rule forbids
// editing them), so it stays as a test-fed re-export.
#[cfg(test)]
#[allow(dead_code)]
pub(in crate::app) fn resolve_surface_focus(focused: bool) -> Color {
    if focused {
        SURFACE_FOCUSED.color()
    } else {
        SURFACE_RESTING.color()
    }
}
