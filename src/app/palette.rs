use ratatui::style::Color;
pub const BASE: Color = Color::Rgb(26, 26, 26); // near-black, for text on colored bg
pub const PANEL_BG: Color = Color::Rgb(60, 66, 74); // #3c424a sidebar/panel background   // near-black, for text on colored bg
pub const OVERLAY: Color = Color::Rgb(63, 63, 63); // gray, unfocused borders
pub const MUTED: Color = Color::Rgb(108, 108, 108); // dim text, icons
pub const SUBTLE: Color = Color::Rgb(158, 158, 158); // secondary text
pub const TEXT: Color = Color::Rgb(230, 230, 230); // primary text
pub const WHITE: Color = Color::Rgb(253, 246, 227); // warm white (#fdf6e3)
pub const BG_GREEN_SOFT: Color = Color::Rgb(72, 88, 78); // softer green-grey (#48584e)
pub const QUEUE_UNFOCUSED_FG: Color = BG_GREEN_SOFT; // queue row text
pub const YELLOW: Color = Color::Rgb(219, 188, 127); // muted gold (#dbbc7f)
pub const AQUA: Color = Color::Rgb(53, 167, 124); // emby green — folders, watched (#35a77c)
pub const FOAM: Color = Color::Rgb(58, 148, 197); // project blue (#3a94c5)
pub const BG_GREEN: Color = Color::Rgb(60, 72, 65); // dark green-grey (#3c4841)
pub const GREEN: Color = Color::Rgb(147, 178, 89); // green (#93b259)
pub const IRIS: Color = Color::Rgb(167, 192, 128); // sage green — active tab, focused pill text (#A7C080)
pub const FOCUSED: Color = Color::Rgb(83, 83, 83); // focused item bg (#535353)
pub const RED: Color = Color::Rgb(229, 126, 128); // muted red (#e57e80)
pub const STATUS_PILL_BG: Color = Color::Rgb(40, 40, 40); // status bar pill background (#282828)
pub const SEEK_TRACK: Color = Color::Rgb(70, 84, 95); // unplayed seek track (design #46545f)
pub const DARK_BG: Color = Color::Rgb(30, 35, 38); // tab bar (Home, etc) background (#1e2326)
pub const LIBRARY_SIDE_BG: Color = Color::Rgb(45, 53, 59); // library-side background, reused for the queue column's unfocused/dim state (#2d353b)
pub const PLAYBACK_PANEL_BG: Color = Color::Rgb(51, 60, 67); // now-playing panel background (#333c43)
pub const PLAYBACK_CONTENT_FG: Color = Color::Rgb(131, 192, 146); // playback title/codec value (#83c092)
pub const PLAYBACK_META_FG: Color = Color::Rgb(133, 146, 137); // playback metadata captions/time (#859289)
pub const TOAST_BG: Color = RED; // toast background (#e57e80)
pub const TOAST_BG_SUCCESS: Color = Color::Rgb(100, 140, 90); // success toast green (#648c5a)
pub const TOAST_BG_WARNING: Color = Color::Rgb(180, 150, 80); // warning toast yellow (#b49650)
pub const TOAST_FG: Color = Color::Rgb(30, 35, 38); // toast foreground (#1e2326)
pub const MUTED_GREEN: Color = Color::Rgb(108, 118, 108); // muted greenish-grey for detail/label text (#6c766c)
pub const SOFT_WHITE: Color = Color::Rgb(244, 234, 211); // warm off-white (#f4ead3)
pub const PILL_SELECTOR_ROW_BG: Color = Color::Rgb(30, 35, 38); // pill-selector row background (#1e2326)
pub const PILL_SELECTOR_BG: Color = Color::Rgb(30, 35, 38); // unselected pill-selector surface (#1e2326)
pub const PILL_SELECTOR_FG: Color = Color::Rgb(73, 81, 86); // unselected pill-selector foreground (#495156)
pub const PILL_SELECTOR_SELECTED_BG: Color = FOAM; // selected pill-selector surface
pub const PILL_SELECTOR_SELECTED_FG: Color = Color::Rgb(30, 35, 38); // selected pill-selector foreground (#1e2326)
pub const PILL_SELECTOR_OVERFLOW_FG: Color = BG_GREEN; // pill-selector overflow/edge accent
pub const SCROLLBAR: Color = BG_GREEN_SOFT; // library/chrome scrollbar track/thumb
pub const ORANGE: Color = Color::Rgb(229, 152, 117); // warm orange (#e59875)
pub const PURPLE: Color = Color::Rgb(214, 153, 182); // muted purple (#d699b6)

// ---------------------------------------------------------------------------
// Roles (openspec/changes/centralize-ui-design-language) — the public API.
// A role names what a colour *means*, never what hue it is. The raw
// constants above are primitives; new call sites use roles, never
// primitives directly. See design.md "Role vocabulary" for the source
// table this block implements verbatim.
// ---------------------------------------------------------------------------

// Surfaces
pub const SURFACE_BACKDROP: Color = LIBRARY_SIDE_BG;
pub const SURFACE_CHROME: Color = DARK_BG;
pub const SURFACE_PANEL: Color = PANEL_BG;
pub const SURFACE_FOCUSED: Color = BG_GREEN;
pub const SURFACE_RESTING: Color = PLAYBACK_PANEL_BG; // resting-content / unfocused half
pub const SURFACE_PLAYBACK: Color = PLAYBACK_PANEL_BG; // now-playing-strip half
pub const SURFACE_ACCENT_SOFT: Color = BG_GREEN_SOFT;
pub const SURFACE_ITEM_FOCUSED: Color = FOCUSED;
pub const SURFACE_STATUS_PILL: Color = STATUS_PILL_BG;

// Text
pub const TEXT_PRIMARY: Color = TEXT;
pub const TEXT_SECONDARY: Color = SUBTLE;
pub const TEXT_MUTED: Color = MUTED;
pub const TEXT_EMPHASIS: Color = WHITE;
pub const TEXT_SOFT: Color = SOFT_WHITE;
pub const TEXT_ON_ACCENT: Color = BASE;
pub const TEXT_ON_STATE: Color = TOAST_FG;
pub const TEXT_DETAIL: Color = MUTED_GREEN;
pub const TEXT_QUEUE_UNFOCUSED: Color = QUEUE_UNFOCUSED_FG;
pub const TEXT_PLAYBACK: Color = PLAYBACK_CONTENT_FG;
pub const TEXT_PLAYBACK_META: Color = PLAYBACK_META_FG;

// Accents
pub const ACCENT: Color = AQUA; // selection marker, watched, folders
pub const ACCENT_BLUE: Color = FOAM;
pub const ACCENT_GREEN: Color = GREEN;
pub const ACCENT_SAGE: Color = IRIS; // active tab, focused pill text
pub const ACCENT_WARM: Color = YELLOW;
pub const ACCENT_ORANGE: Color = ORANGE;
pub const ACCENT_PURPLE: Color = PURPLE;

// Rules
pub const RULE: Color = SEEK_TRACK; // hero shell border and seek track share this role
pub const BORDER_UNFOCUSED: Color = OVERLAY;

// State
pub const STATE_ERROR: Color = RED;
pub const STATE_SUCCESS: Color = TOAST_BG_SUCCESS;
pub const STATE_WARNING: Color = TOAST_BG_WARNING;

// Pill selector (renamed without value changes)
pub const PILL_ROW_BG: Color = PILL_SELECTOR_ROW_BG;
pub const PILL_BG: Color = PILL_SELECTOR_BG;
pub const PILL_FG: Color = PILL_SELECTOR_FG;
pub const PILL_SELECTED_BG: Color = PILL_SELECTOR_SELECTED_BG;
pub const PILL_SELECTED_FG: Color = PILL_SELECTOR_SELECTED_FG;
pub const PILL_OVERFLOW_FG: Color = PILL_SELECTOR_OVERFLOW_FG;

/// The central focus lever (design decision 8). Every panel and component
/// resolves its focused/unfocused surface through this single function
/// instead of naming `SURFACE_FOCUSED`/`SURFACE_RESTING` at the call site.
///
/// `focused` is the caller's two-input focus model already collapsed to one
/// bool: the existing `PanelFocus` (which panel is focused) for
/// hero-on-top screens with one focusable region, or `PanelFocus` combined
/// with a pane bit (`left_focused`) for hero-on-left screens with two.
pub fn resolve_surface_focus(focused: bool) -> Color {
    if focused {
        SURFACE_FOCUSED
    } else {
        SURFACE_RESTING
    }
}
