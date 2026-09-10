mod primitives;

mod surface;
mod surface_resolve;
mod surface_table;

// The closed surface table is public to `crate::app` (a screen names a
// `Surface` and the resolver), but its `Level`/`Row`/`FocusSource` machinery
// stays private to the theme. Re-exported here so `render/mod.rs` and
// `palette.rs` can bridge the names to production call sites.
pub(in crate::app) use surface::Surface;
pub(in crate::app) use surface_resolve::{surface_colors, surface_colors_for_column_focus};

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
// The focused surface's own primitive, deliberately not the text role's
// `TEXT_ACCENT_MUTED_FG`: a text-colour edit can never move a surface
// appearance (and vice versa). The two primitives share a value today.
pub const SURFACE_FOCUSED: Color = primitives::SURFACE_FOCUSED_BG;
pub const SURFACE_RESTING: Color = primitives::PLAYBACK_PANEL_BG; // resting-content / unfocused half

pub const SURFACE_ITEM_FOCUSED: Color = primitives::FOCUSED;
pub const SURFACE_SIDEBAR: Color = primitives::PANEL_BG; // plain (non-hero) sidebar/panel background

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
pub const TEXT_ACCENT_MUTED: Color = primitives::TEXT_ACCENT_MUTED_FG; // "loaded"/"playing"/confirmed value text
pub const TEXT_DETAIL_META: Color = primitives::MUTED_GREEN; // detail-screen label/meta text
pub const TEXT_METADATA: Color = primitives::FOAM; // secondary metadata (durations, pct, badges)
pub const TEXT_TAB_INACTIVE: Color = primitives::TAB_INACTIVE_FG; // tab bar's inactive tab glyph

// Popup
/// The popup dim backdrop's fill: the shade every already-rendered cell is
/// blended halfway toward while a blocking popup is up. It paints no fill of
/// its own; its row is the table's popup-level dim row (`Surface::PopupDimBackdrop`).
pub const DIM_BACKDROP: Color = primitives::DIM_BACKDROP;

/// One cell's colour under the popup dim backdrop: blended halfway toward
/// `base`, the `PopupDimBackdrop` surface's fill.
///
/// `Color::White` and `Color::Black` carry no RGB channels in ratatui, so
/// they are handled explicitly; indexed and other named variants pass through
/// undimmed because the running terminal, not ratatui, defines their RGB.
/// Raw `Color::Rgb` construction lives here, not in the painter, because raw
/// colour primitives are private to the theme.
pub fn dim_backdrop_color(color: Color, base: Color) -> Color {
    let Color::Rgb(br, bg, bb) = base else {
        return color;
    };
    let blend = |r: u8, g: u8, b: u8| {
        Color::Rgb(
            ((r as u16 + br as u16) / 2) as u8,
            ((g as u16 + bg as u16) / 2) as u8,
            ((b as u16 + bb as u16) / 2) as u8,
        )
    };
    match color {
        Color::White => blend(255, 255, 255),
        Color::Black | Color::Reset => base,
        Color::Rgb(r, g, b) => blend(r, g, b),
        other => other,
    }
}

// Status
pub const STATUS_ERROR: Color = primitives::RED;
pub const STATUS_AVAILABLE: Color = primitives::GREEN; // checkmarks, available/positive metadata

// Media indicators (resolution/audio glyphs)
pub const INDICATOR_RESOLUTION_FG: Color = primitives::ORANGE;
pub const INDICATOR_AUDIO_FG: Color = primitives::PURPLE;

// Playback panel
pub const PLAYBACK_VALUE_FG: Color = primitives::PLAYBACK_CONTENT_FG; // title/codec value
pub const PLAYBACK_META_FG: Color = primitives::PLAYBACK_META_FG; // captions/time
pub const PLAYBACK_THROBBER_FG: Color = primitives::AQUA; // now-playing liveness

// Progress and queue
pub const PROGRESS_TRACK: Color = primitives::SEEK_TRACK; // unplayed seek/progress track

// Chrome
pub const SCROLLBAR: Color = primitives::SCROLLBAR;
