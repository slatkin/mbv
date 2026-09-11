mod primitives;
mod surface;
mod surface_resolve;
mod surface_table;

// The closed surface table (`unify-surface-colour-neutral` D1/D2/D7). A screen
// names a `Surface` and calls the resolver; the `Level`/`Row`/`FocusSource`
// machinery stays private to the theme. Re-exported here so `render/mod.rs` and
// `palette.rs` can bridge the names to production call sites.
pub(in crate::app) use surface::Surface;
pub(in crate::app) use surface_resolve::surface_colors;

use ratatui::style::Color;

// ---------------------------------------------------------------------------
// Roles (openspec/changes/centralize-ui-design-language,
// openspec/changes/enforce-mbv-ui-design-system) — the public API. A role
// names what a colour *means*, never what hue it is. `primitives` holds the
// raw constants and is private to this module, so nothing outside `theme`
// can name a hue directly.
// ---------------------------------------------------------------------------

// Surfaces
pub const SURFACE_BACKDROP: Color = primitives::LIBRARY_SIDE_BG;
pub const SURFACE_CHROME: Color = primitives::DARK_BG;
pub const SURFACE_FOCUSED: Color = primitives::SURFACE_FOCUSED_BG;
pub const SURFACE_RESTING: Color = primitives::PLAYBACK_PANEL_BG; // resting-content / unfocused half
                                                                  // Transitional: retired from production by task 4.2 (`SURFACE_PLAYBACK` was
                                                                  // `SURFACE_RESTING`'s value alias); the name stays reachable only because the
                                                                  // frozen `render/tests.rs` pins it, which the neutrality rule forbids editing.
#[cfg(test)]
#[allow(dead_code)]
pub const SURFACE_PLAYBACK: Color = primitives::PLAYBACK_PANEL_BG; // now-playing-strip half
                                                                   // Transitional: retired from production by task 4.2 (`SURFACE_ACCENT_SOFT`'s
                                                                   // value duplicated `SCROLLBAR`'s; the soft variant resolves through
                                                                   // `SOFT_CONTENT_BODY_BG` now); the name stays reachable only because three
                                                                   // frozen test files pin it, which the neutrality rule forbids editing.
#[cfg(test)]
#[allow(dead_code)]
pub const SURFACE_ACCENT_SOFT: Color = primitives::SOFT_CONTENT_BODY_BG;
pub const SURFACE_ITEM_FOCUSED: Color = primitives::FOCUSED;
pub const SURFACE_SIDEBAR: Color = primitives::PANEL_BG; // plain (non-hero) sidebar/panel background
                                                         // Transitional: retired from production by task 4.2 (`SURFACE_ARTWORK_PLACEHOLDER`
                                                         // was `SURFACE_BACKDROP`'s value alias); the name stays reachable only because
                                                         // the frozen `artwork_placeholder_tests.rs` pins it.
#[cfg(test)]
#[allow(dead_code)]
pub const SURFACE_ARTWORK_PLACEHOLDER: Color = primitives::ARTWORK_PLACEHOLDER;

// Accents
pub const ACCENT: Color = primitives::AQUA; // selection marker, watched, folders, Emby brand glyph
pub const ACCENT_ACTIVE: Color = primitives::IRIS; // active tab, focused pill/badge text, selected-row bg
pub const ACCENT_AUDIOBOOKSHELF: Color = primitives::AMBER; // audiobookshelf brand glyph

// Rules
pub const BORDER_UNFOCUSED: Color = primitives::OVERLAY;

// Pill selector (renamed without value changes)
pub const PILL_ROW_BG: Color = primitives::PILL_SELECTOR_ROW_BG;
pub const PILL_BG: Color = primitives::PILL_SELECTOR_BG;
pub const PILL_SELECTED_BG: Color = primitives::PILL_SELECTOR_SELECTED_BG;
pub const PILL_FG: Color = primitives::PILL_SELECTOR_FG;
pub const PILL_SELECTED_FG: Color = primitives::PILL_SELECTOR_SELECTED_FG;
pub const PILL_OVERFLOW_FG: Color = primitives::PILL_SELECTOR_OVERFLOW_FG;

// Text: readable content foregrounds, independent of surface
pub const TEXT_PRIMARY: Color = primitives::TEXT;
pub const TEXT_SECONDARY: Color = primitives::SUBTLE; // paired with TEXT_PRIMARY/TEXT_STRONG
pub const TEXT_MUTED: Color = primitives::MUTED; // dim text, icons, unfocused state
pub const TEXT_STRONG: Color = primitives::WHITE; // bold titles/headings
pub const TEXT_EMPHASIS: Color = primitives::SOFT_WHITE; // warm emphasis text (focused rows, dialogs)
pub const TEXT_FOCUS_ACCENT: Color = primitives::YELLOW; // focused-row title accent
pub const TEXT_ON_ACCENT: Color = primitives::BASE; // near-black text painted on a colored surface
pub const TEXT_ACCENT_MUTED: Color = primitives::BG_GREEN; // "loaded"/"playing"/confirmed value text; deliberately
                                                           // not the focused surface's `SURFACE_FOCUSED_BG` (task 4.2)
pub const TEXT_DETAIL_META: Color = primitives::MUTED_GREEN; // detail-screen label/meta text
pub const TEXT_METADATA: Color = primitives::FOAM; // secondary metadata (durations, pct, badges)

// Status
pub const STATUS_ERROR: Color = primitives::RED;
pub const STATUS_AVAILABLE: Color = primitives::GREEN; // checkmarks, available/positive metadata

// Media indicators (resolution/audio glyphs)
pub const INDICATOR_RESOLUTION_FG: Color = primitives::ORANGE;
pub const INDICATOR_AUDIO_FG: Color = primitives::PURPLE;

// Playback panel
pub const PLAYBACK_VALUE_FG: Color = primitives::PLAYBACK_CONTENT_FG; // title/codec value
pub const PLAYBACK_META_FG: Color = primitives::PLAYBACK_META_FG; // captions/time
pub const PLAYBACK_THROBBER_FG: Color = primitives::PLAYBACK_THROBBER_FG; // now-playing liveness

// Progress and queue
pub const PROGRESS_TRACK: Color = primitives::SEEK_TRACK; // unplayed seek/progress track

// Chrome
pub const SCROLLBAR: Color = primitives::SCROLLBAR;

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
