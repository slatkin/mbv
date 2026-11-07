//! The bridge theme's chrome section (design D7): each chrome value resolves
//! the `Surface` the TUI paints the same concept with, so the front end's
//! chrome variables track the mbv palette with no second palette.

use mbv_gui::bridge::frame::theme::{GuiRgb, GuiTheme};
use mbv_theme::{Surface, surface_colors};

/// Guards the documented chrome mapping (design D7, review correction): each
/// chrome value is the resting fill of its named authority surface, so an
/// authority swap that moves a tier cannot land silently. Aliased fields
/// (`card`, `muted`, `input`) resolve their own authority, not another
/// field's; the `focused` fields resolve the same surface's focused fill.
#[test]
fn chrome_values_resolve_their_documented_surfaces() {
    let chrome = GuiTheme::current().chrome;
    let fill = |surface| GuiRgb::from(surface_colors(surface, false).fill);
    let focused = |surface| GuiRgb::from(surface_colors(surface, true).fill);
    assert_eq!(chrome.background, fill(Surface::PlaybackPanel));
    assert_eq!(chrome.card, fill(Surface::MainContentBox));
    assert_eq!(chrome.popover, fill(Surface::PopupFrame));
    assert_eq!(chrome.muted, fill(Surface::ListStripe));
    assert_eq!(chrome.secondary, fill(Surface::TransportRow));
    assert_eq!(chrome.sidebar, fill(Surface::SidebarBody));
    assert_eq!(chrome.border, fill(Surface::PopupBorder));
    assert_eq!(chrome.input, fill(Surface::PopupBorder));
    assert_eq!(chrome.queue_column, fill(Surface::QueueColumn));
    assert_eq!(chrome.queue_column_focused, focused(Surface::QueueColumn));
    assert_eq!(chrome.queue_panel, fill(Surface::QueuePanel));
    assert_eq!(chrome.queue_panel_focused, focused(Surface::QueuePanel));
    assert_eq!(
        chrome.queue_playback_band,
        fill(Surface::QueueOnlyPlaybackPanel)
    );
    assert_eq!(chrome.queue_footer_band, fill(Surface::QueuePanelBand));
}
