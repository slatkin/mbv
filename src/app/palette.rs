use ratatui::style::Color;
pub const BASE: Color = Color::Rgb(26, 26, 26); // near-black, for text on colored bg
pub const PANEL_BG: Color = Color::Rgb(60, 66, 74); // #3c424a sidebar/panel background   // near-black, for text on colored bg
pub const OVERLAY: Color = Color::Rgb(63, 63, 63); // gray, unfocused borders
pub const MUTED: Color = Color::Rgb(108, 108, 108); // dim text, icons
pub const SUBTLE: Color = Color::Rgb(158, 158, 158); // secondary text
pub const TEXT: Color = Color::Rgb(230, 230, 230); // primary text
pub const WHITE: Color = Color::Rgb(230, 230, 230); // near-white (#e6e6e6)
pub const YELLOW: Color = Color::Rgb(219, 188, 127); // muted gold (#dbbc7f)
pub const PINE: Color = Color::Rgb(131, 192, 146); // emby green — folders, watched (#83c092)
pub const GREEN: Color = Color::Rgb(60, 72, 65); // dark green-grey (#3c4841)
pub const TEAL: Color = Color::Rgb(127, 187, 179); // light teal (#7fbbb3) — playback panel title text

pub const IRIS: Color = Color::Rgb(167, 192, 128); // sage green — active tab, focused pill text (#A7C080)
pub const IRIS_DIM: Color = Color::Rgb(83, 133, 80); // seekbar downloaded-unplayed: IRIS@50% over #555555
pub const FOCUSED: Color = Color::Rgb(83, 83, 83); // focused item bg (#535353)
pub const RED: Color = Color::Rgb(220, 60, 60); // loud volume
pub const PILL_BG: Color = Color::Rgb(60, 66, 74); // subtle grey control-pill background
pub const STATUS_PILL_BG: Color = Color::Rgb(40, 40, 40); // status bar pill background (#282828)
pub const SEEK_TRACK: Color = Color::Rgb(70, 84, 95); // unplayed seek track (design #46545f)
pub const BAR_BG: Color = Color::Reset; // transparent status/title bar background
pub const CONTINUE_BG: Color = GREEN; // continue watching list background
pub const MEDIA_SELECTED_BG: Color = GREEN; // selected movie block background in the power view
pub const DARK_BG: Color = Color::Rgb(30, 35, 38); // tab bar (Home, etc) background (#1e2326)
pub const POWER_RIGHT_BG: Color = Color::Rgb(45, 53, 59); // power view right panel background (#2d353b)
pub const MUTED_GREEN: Color = Color::Rgb(108, 118, 108); // muted greenish-grey for detail/label text (#6c766c)
pub const PILL: Color = Color::Rgb(211, 198, 170); // unselected pill text (#D3C6AA)
pub const SOFT_WHITE: Color = PILL; // warm off-white for unicode borders (#D3C6AA) -- same value as PILL, aliased so the two can't silently drift apart
