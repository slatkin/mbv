mod palette;
mod surface;
mod surface_resolve;
mod surface_table;

// The closed palette enum (openspec/changes/archive/2026-09-19-palette-enum).
// The role tier
// below derives from it; the surface tier follows in the same section.
// `palette.rs` is private to the theme, so nothing outside `theme` can name
// a hue directly.

// The closed surface table (`unify-surface-colour-neutral` D1/D2/D7). A screen
// names a `Surface` and calls the resolver; the `Level`/`Row`/`FocusSource`
// machinery stays private to the theme. Re-exported here so `render/mod.rs` and
// `palette.rs` can bridge the names to production call sites.
pub(in crate::app) use surface::Surface;
pub(in crate::app) use surface_resolve::surface_colors;

use ratatui::style::Color;

use palette::Palette;

// ---------------------------------------------------------------------------
// Roles (openspec/changes/centralize-ui-design-language,
// openspec/changes/enforce-mbv-ui-design-system) — the public API. A role
// names what a colour *means*, never what hue it is. `palette` holds the
// raw constants and is private to this module, so nothing outside `theme`
// can name a hue directly.
// ---------------------------------------------------------------------------

// Surfaces
pub const SURFACE_BACKDROP: Color = Palette::Slate.color();
pub const SURFACE_CHROME: Color = Palette::Ink.color();
/// The focused surface's own fill; deliberately not the text-green
/// `TEXT_ACCENT_MUTED`, whose `Palette::Green1` value it shares today: the
/// two are equal today and independently editable, so a text-colour edit
/// moves the text alone (and vice versa; unify-surface-colour-neutral
/// task 4.2).
pub const SURFACE_FOCUSED: Color = Palette::Green1.color();
pub const SURFACE_RESTING: Color = Palette::Storm.color(); // resting-content / unfocused half
                                                           // Transitional: retired from production by task 4.2 (`SURFACE_PLAYBACK` was
                                                           // `SURFACE_RESTING`'s value alias); the name stays reachable only because the
                                                           // frozen `render/tests.rs` pins it, which the neutrality rule forbids editing.
#[cfg(test)]
#[allow(dead_code)]
pub const SURFACE_PLAYBACK: Color = Palette::Storm.color(); // now-playing-strip half
                                                            // Transitional: retired from production by task 4.2 (`SURFACE_ACCENT_SOFT`'s
                                                            // value duplicated `SCROLLBAR`'s; the soft variant resolves through the soft
                                                            // content-body value, `Palette::Green2`, now); the name stays reachable only
                                                            // because three frozen test files pin it, which the neutrality rule forbids
                                                            // editing.
#[cfg(test)]
#[allow(dead_code)]
pub const SURFACE_ACCENT_SOFT: Color = Palette::Green2.color();
// Transitional: retired from production by task 4.2 (`SURFACE_ARTWORK_PLACEHOLDER`
// was `SURFACE_BACKDROP`'s value alias); the name stays reachable only because
// the frozen `artwork_placeholder_tests.rs` pins it.
#[cfg(test)]
#[allow(dead_code)]
pub const SURFACE_ARTWORK_PLACEHOLDER: Color = Palette::Slate.color();

// Hero surfaces
pub const HERO_OVERVIEW_SEPARATOR: Color = Palette::Iris.color(); // overview/credits separator
pub const HERO_CREDITS_STRIPE: Color = Palette::Storm.color(); // alternating credits row
pub const HERO_CREDITS_NAME: Color = Palette::Yellow.color(); // credits name column

// Accents
pub const ACCENT: Color = Palette::Aqua.color(); // focus accent, watched, folders, Emby brand glyph
pub const ACCENT_ACTIVE: Color = Palette::Iris.color(); // active tab, focused pill/badge text, selected-row bg
pub const ACCENT_AUDIOBOOKSHELF: Color = Palette::Yellow.color(); // audiobookshelf brand glyph

// Rules

// Pill selector (renamed without value changes)
pub const PILL_ROW_BG: Color = Palette::Ink.color();
pub const PILL_BG: Color = Palette::Ink.color();
pub const PILL_SELECTED_BG: Color = Palette::Foam.color(); // selected pill-selector surface (#3a94c5);
                                                           // shares `TEXT_METADATA`'s `Palette::Foam`
                                                           // value today, kept separate from the
                                                           // metadata text: the two are equal today
                                                           // and independently editable, so an edit
                                                           // to either moves it alone
pub const PILL_SELECTED_FG: Color = Palette::Ink.color();
/// The pill-selector's overflow/edge accent. A former value alias of the
/// text green (`PILL_SELECTOR_OVERFLOW_FG = BG_GREEN`), now its own role: it
/// shares `TEXT_ACCENT_MUTED`'s `Palette::Green1` value today — the two are
/// equal today and independently editable, so an edit to either moves it
/// alone.
pub const PILL_OVERFLOW_FG: Color = Palette::Green1.color();

// Text: readable content foregrounds, independent of surface
pub const TEXT_PRIMARY: Color = Palette::Grey6.color();
pub const TEXT_SECONDARY: Color = Palette::Grey5.color(); // paired with TEXT_PRIMARY/TEXT_STRONG
pub const TEXT_MUTED: Color = Palette::Grey4.color(); // dim text, icons, unfocused state
/// A played media-list row's title (single-part primary, or a played split
/// row's item title; the split context keeps `SPLIT_ROW_CONTEXT_FG`). Its own
/// role rather than the generic dim `TEXT_MUTED`, so a played row can read as
/// watched without moving icons and unfocused chrome.
pub const TEXT_STRONG: Color = Palette::White.color(); // bold titles/headings
pub const TEXT_EMPHASIS: Color = Palette::Cream.color(); // warm emphasis text (focused rows, dialogs)
pub const TEXT_FOCUS_ACCENT: Color = Palette::Yellow.color(); // focused-row title accent
pub const TEXT_HERO_TITLE: Color = Palette::Yellow.color(); // hero header title (the first metadata line)
pub const TEXT_ON_ACCENT: Color = Palette::Grey1.color(); // near-black text painted on a colored surface
pub const TEXT_ACCENT_MUTED: Color = Palette::Green1.color(); // "loaded"/"playing"/confirmed value text;
                                                              // deliberately not the focused surface's
                                                              // `SURFACE_FOCUSED`, whose `Palette::Green1`
                                                              // value it shares today: the two are equal
                                                              // today and independently editable, so a
                                                              // text-colour edit moves the text alone
                                                              // (unify-surface-colour-neutral task 4.2)
pub const TEXT_DETAIL_META: Color = Palette::Green3.color(); // detail-screen label/meta text
pub const TEXT_METADATA: Color = Palette::Foam.color(); // secondary metadata (durations, pct, badges)
/// Selected-row bar fill (audition: an opaque full-width bar replaces the
/// punch-through and the gutter-accent-only title treatment; the row keeps
/// its ordinary foreground roles on the bar).
pub const SELECTED_ROW_BG: Color = Palette::Slate.color(); // its own role, deliberately not
                                                           // `SURFACE_BACKDROP`, whose `Palette::Slate`
                                                           // value it shares today: the two are equal
                                                           // today and independently editable, so an
                                                           // edit to either moves it alone

// Hero header metadata cycling roles (task 5.5, design D5): the one title/meta
// painter colours meta row *n* with `HERO_META_ROLES[n % 3]` — the three colours
// the Emby hero headers already used, defined once so destinations cannot style
// metadata.
pub const HERO_META_ROLES: [Color; 3] = [TEXT_DETAIL_META, TEXT_METADATA, TEXT_SECONDARY];

/// The Library Hero overlay's display-only hint chips' fills, rotated by chip
/// index (foam, yellow, orange, repeating). A role set of its own rather than
/// borrowed text roles: these are chip *surfaces*, and a text-colour edit must
/// not move them.
pub const HINT_PILL_FILLS: [Color; 3] = [
    Palette::Foam.color(),
    Palette::Yellow.color(),
    Palette::Orange.color(),
];

// Status
pub const STATUS_ERROR: Color = Palette::Red.color();
pub const STATUS_AVAILABLE: Color = Palette::Green.color(); // checkmarks, available/positive metadata

// Media list metadata
/// The right-aligned duration column of the Queue list. Its own role rather
/// than the green `STATUS_AVAILABLE` metadata role, so a status-colour edit
/// cannot move the durations. Only the Queue list projects a duration; library
/// browse rows carry no time.
pub const DURATION: Color = Palette::Iris.color(); // the sage

/// The right-aligned publish-date column of a library browse row — the
/// podcast browser's `17 Sep 26` gutter. Its own role rather than the
/// `DURATION` time column's, so a duration edit cannot move the dates; a date
/// is metadata about the item, not a playback time.
pub const ROW_DATE_FG: Color = Palette::Yellow.color(); // muted gold (#dbbc7f)

/// The primary (context/container) part of a split media-list row — the
/// podcast an episode row came from. Its own role rather than
/// `PLAYBACK_CONTEXT_FG`, the playback strip's context role: browse lists
/// paint their split-row context in the soft-white emphasis colour while the
/// strip keeps gold.
pub const SPLIT_ROW_CONTEXT_FG: Color = Palette::Cream.color(); // soft white (#faedcd)

/// The secondary title of a split media-list row — the item's own name after
/// the container/context (which paints `SPLIT_ROW_CONTEXT_FG`). Its own role
/// rather than `PLAYBACK_TITLE_FG`, the playback strip's own-name role:
/// browse lists paint their split-row titles in a light grey while the strip
/// keeps aqua.
pub const SPLIT_ROW_TITLE_FG: Color = Palette::Grey5.color(); // light grey (#9e9e9e)

// Media indicators (resolution/audio glyphs)
pub const INDICATOR_RESOLUTION_FG: Color = Palette::Orange.color();
pub const INDICATOR_AUDIO_FG: Color = Palette::Purple.color();

// Playback panel
pub const PLAYBACK_VALUE_FG: Color = Palette::Iris.color(); // title/codec value
pub const PLAYBACK_META_FG: Color = Palette::Iris.color(); // captions/time
/// The now-playing title row's title part: the item's own name (episode,
/// track, entry, ...). Its own role rather than `ACCENT`/`TEXT_FOCUS_ACCENT`,
/// whose `Palette::Aqua` value it shares today: the two are equal today and
/// independently editable, so a focus-accent or brand edit moves the accent
/// alone (now-playing-media-type-titles D2).
pub const PLAYBACK_TITLE_FG: Color = Palette::Aqua.color();
/// The now-playing title row's context part: the container the item came
/// from (series, artist, show, subscription). Its own role rather than the
/// focused-row accent, whose `Palette::Yellow` value it shares today: the
/// two are equal today and independently editable, so an accent edit moves
/// the accent alone (now-playing-media-type-titles D2).
pub const PLAYBACK_CONTEXT_FG: Color = Palette::Yellow.color();

// Progress and queue
pub const PROGRESS_TRACK: Color = Palette::Grey5.color(); // unplayed seek/progress track

// Chrome
/// Library/chrome scrollbar track/thumb; a former value alias of the soft
/// content-body fill (`SCROLLBAR = BG_GREEN_SOFT`), now its own role:
/// deliberately not the soft content-body surface, whose `Palette::Green2`
/// value it shares today — the two are equal today and independently
/// editable, so a scrollbar edit moves the scrollbar alone
/// (unify-surface-colour-neutral task 4.2).
pub const SCROLLBAR: Color = Palette::Green2.color();

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
pub fn resolve_surface_focus(focused: bool) -> Color {
    if focused {
        SURFACE_FOCUSED
    } else {
        SURFACE_RESTING
    }
}
