use ratatui::style::Color;
pub const BASE:      Color = Color::Rgb(26,  26,  26);   // near-black, for text on colored bg
pub const OVERLAY:   Color = Color::Rgb(63,  63,  63);   // gray, unfocused borders
pub const MUTED:     Color = Color::Rgb(108, 108, 108);  // dim text, icons
pub const SUBTLE:    Color = Color::Rgb(158, 158, 158);  // secondary text
pub const TEXT:      Color = Color::Rgb(230, 230, 230);  // primary text
pub const WHITE:     Color = Color::Rgb(230, 230, 230);  // near-white (#e6e6e6)
pub const YELLOW:    Color = Color::Rgb(250, 220, 70);   // yellow — in-progress, paused
pub const PINE:      Color = Color::Rgb(61,  139, 55);   // dark green — folders, watched
pub const FOAM:      Color = Color::Rgb(0,   164, 220);  // emby blue — now-playing item
pub const IRIS:      Color = Color::Rgb(82,  181, 75);   // emby green — active tab, focused
pub const IRIS_DIM:  Color = Color::Rgb(83,  133, 80);   // seekbar downloaded-unplayed: IRIS@50% over #555555
pub const FOCUSED:   Color = Color::Rgb(83,  83,  83);   // focused item bg (#535353)
pub const RED:       Color = Color::Rgb(220, 60,  60);   // loud volume
pub const SEEK_TRACK: Color = Color::Rgb(70,  84,  95);  // unplayed seek track (design #46545f)
pub const SEEK_KNOB:  Color = Color::Rgb(155, 224, 124); // played-endpoint dot (design #9be07c)
