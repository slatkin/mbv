use ratatui::style::Color;

pub(super) const BASE: Color = Color::Rgb(26, 26, 26); // near-black, for text on colored bg
pub(super) const PANEL_BG: Color = Color::Rgb(60, 66, 74); // #3c424a sidebar/panel background   // near-black, for text on colored bg
                                                           // Transitional: retired from production by task 4.2 (the no-artwork fill
                                                           // resolves through the `ArtworkPlaceholder` row's `SURFACE_BACKDROP` value);
                                                           // the primitive stays only for the test-fed `SURFACE_ARTWORK_PLACEHOLDER`.
#[cfg(test)]
#[allow(dead_code)]
pub(super) const ARTWORK_PLACEHOLDER: Color = Color::Rgb(45, 53, 59); // no-artwork surface
/// The artwork-loading inset's fill. A primitive of its own, not the border
/// role's `OVERLAY`: a border-colour edit can never move a surface appearance
/// (the two primitives share a value today).
pub(super) const ARTWORK_LOADING_PLACEHOLDER: Color = Color::Rgb(63, 63, 63); // artwork-loading fill (#3f3f3f), not a border
pub(super) const OVERLAY: Color = Color::Rgb(63, 63, 63); // gray, unfocused borders
pub(super) const MUTED: Color = Color::Rgb(108, 108, 108); // dim text, icons
pub(super) const SUBTLE: Color = Color::Rgb(158, 158, 158); // secondary text
pub(super) const TEXT: Color = Color::Rgb(230, 230, 230); // primary text
pub(super) const WHITE: Color = Color::Rgb(253, 246, 227); // warm white (#fdf6e3)
/// The soft content-body surface fill. A primitive of its own, not the
/// scrollbar's `SCROLLBAR` below: an edit to the scrollbar must not move a
/// surface appearance (unify-surface-colour-neutral task 4.2; the two share
/// a value today).
pub(super) const SOFT_CONTENT_BODY_BG: Color = Color::Rgb(72, 88, 78); // soft content-body green-grey (#48584e)
/// The scrollbar-track green backing `SCROLLBAR`: deliberately not the soft
/// content-body's `SOFT_CONTENT_BODY_BG`, whose value it shares today.
pub(super) const BG_GREEN_SOFT: Color = Color::Rgb(72, 88, 78); // softer green-grey (#48584e)
pub(super) const YELLOW: Color = Color::Rgb(219, 188, 127); // muted gold (#dbbc7f)
pub(super) const AQUA: Color = Color::Rgb(53, 167, 124); // emby green — folders, watched (#35a77c)
/// Liveness green; shares `AQUA`'s value today, kept separate so a throbber
/// edit cannot move the `ACCENT` surface fill (the queue's selected scope
/// pill resolves the `QueueScopePillSelected` row through it).
pub(super) const PLAYBACK_THROBBER_FG: Color = Color::Rgb(53, 167, 124); // now-playing liveness (#35a77c)
pub(super) const AMBER: Color = Color::Rgb(199, 152, 71); // audiobookshelf gold (#c79847)
pub(super) const FOAM: Color = Color::Rgb(58, 148, 197); // project blue (#3a94c5)
/// Selected pill surface; shares `FOAM`'s value today, kept separate so a
/// `TEXT_METADATA` edit cannot move this surface fill.
pub(super) const PILL_SELECTOR_SELECTED_BG: Color = Color::Rgb(58, 148, 197); // selected pill-selector surface (#3a94c5)
/// The focused surface's own primitive, deliberately not the text role's
/// `BG_GREEN`: a text-colour edit can never move a surface appearance (and
/// vice versa; unify-surface-colour-neutral task 4.2). The two primitives
/// share a value today.
pub(super) const SURFACE_FOCUSED_BG: Color = Color::Rgb(60, 72, 65); // focused-surface green-grey (#3c4841)
/// The text-role green (`TEXT_ACCENT_MUTED`'s backing value, also the
/// pill-selector's overflow accent): deliberately not the focused surface's
/// `SURFACE_FOCUSED_BG`, whose value it shares today.
pub(super) const BG_GREEN: Color = Color::Rgb(60, 72, 65); // dark green-grey (#3c4841)
pub(super) const GREEN: Color = Color::Rgb(147, 178, 89); // green (#93b259)
pub(super) const IRIS: Color = Color::Rgb(167, 192, 128); // sage green — active tab, focused pill text (#A7C080)
pub(super) const FOCUSED: Color = Color::Rgb(83, 83, 83); // focused item bg (#535353)
pub(super) const RED: Color = Color::Rgb(229, 126, 128); // muted red (#e57e80)
pub(super) const SEEK_TRACK: Color = Color::Rgb(70, 84, 95); // unplayed seek track (design #46545f)
pub(super) const DARK_BG: Color = Color::Rgb(30, 35, 38); // tab bar (Home, etc) background (#1e2326)
pub(super) const LIBRARY_SIDE_BG: Color = Color::Rgb(45, 53, 59); // library-side background, reused for the queue column's unfocused/dim state (#2d353b)
pub(super) const PLAYBACK_PANEL_BG: Color = Color::Rgb(51, 60, 67); // now-playing panel background (#333c43)
pub(super) const PLAYBACK_CONTENT_FG: Color = Color::Rgb(131, 192, 146); // playback title/codec value (#83c092)
pub(super) const PLAYBACK_META_FG: Color = Color::Rgb(133, 146, 137); // playback metadata captions/time (#859289)
pub(super) const MUTED_GREEN: Color = Color::Rgb(108, 118, 108); // muted greenish-grey for detail/label text (#6c766c)
pub(super) const SOFT_WHITE: Color = Color::Rgb(244, 234, 211); // warm off-white (#f4ead3)
pub(super) const PILL_SELECTOR_ROW_BG: Color = Color::Rgb(30, 35, 38); // pill-selector row background (#1e2326)
pub(super) const PILL_SELECTOR_BG: Color = Color::Rgb(30, 35, 38); // unselected pill-selector surface (#1e2326)
pub(super) const PILL_SELECTOR_FG: Color = Color::Rgb(73, 81, 86); // unselected pill-selector foreground (#495156)
pub(super) const PILL_SELECTOR_SELECTED_FG: Color = Color::Rgb(30, 35, 38); // selected pill-selector foreground (#1e2326)
pub(super) const PILL_SELECTOR_OVERFLOW_FG: Color = BG_GREEN; // pill-selector overflow/edge accent
/// Scrollbar track/thumb; shares `SOFT_CONTENT_BODY_BG`'s value today, kept
/// separate so a scrollbar edit cannot move that surface fill.
pub(super) const SCROLLBAR: Color = BG_GREEN_SOFT; // library/chrome scrollbar track/thumb
pub(super) const ORANGE: Color = Color::Rgb(229, 152, 117); // warm orange (#e59875)
pub(super) const PURPLE: Color = Color::Rgb(214, 153, 182); // muted purple (#d699b6)
