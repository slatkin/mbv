//! Design D7: the theme colour to toolkit colour conversion is the
//! `ToRatatui` extension trait of this crate, and it carries every 8-bit
//! channel triple through unchanged.

use mbv_theme::Rgb;
use mbv_tui_support::ToRatatui;
use ratatui::style::Color;

#[test]
fn each_channel_reaches_its_toolkit_colour_position() {
    let color = Rgb::new(0x12, 0x34, 0x56).to_ratatui();

    assert_eq!(color, Color::Rgb(0x12, 0x34, 0x56));
}
