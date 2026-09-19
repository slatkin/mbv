use crate::app::components::settings::SettingsRow;
use crate::app::palette;
use crate::app::render::{render_settings_content, SettingsRenderGeometry, SettingsRenderModel};
use crate::app::types_settings::SettingsDestination;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn settings_fullscreen_fallback_paints_the_shell_over_the_frame() {
    // Width 80 deliberately differs from the retired 40-col clamp: the
    // fullscreen contract must fail if the shell ever re-clamps.
    let width = 80;
    let height = 12;
    let rows = vec![SettingsRow {
        label: "Stay alive".into(),
        value: "off".into(),
        section: false,
        cursor: Some(0),
    }];
    let mut geometry = SettingsRenderGeometry::default();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            render_settings_content(
                frame,
                frame.area(),
                SettingsRenderModel {
                    destination: SettingsDestination::Main,
                    rows: &rows,
                    keys: &[],
                    services: &[],
                    setup: None,
                    cursor: 0,
                    services_cursor: 0,
                    scroll: 0,
                },
                &mut geometry,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(geometry.panel_area, Rect::new(0, 0, width, height));
    assert_eq!(
        buffer[(0, 0)].bg,
        palette::surface_colors(palette::Surface::SidebarBody, false).fill
    );
    assert_eq!(
        buffer[(2, 1)].bg,
        palette::surface_colors(palette::Surface::SidebarBand, false).fill
    );
    assert_eq!(buffer[(3, 1)].symbol(), "S");
    assert_eq!(buffer[(width - 1, 2)].symbol(), " ");
}
