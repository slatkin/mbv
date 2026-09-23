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

/// Group spacing + zebra: a blank line separates groups, data rows
/// alternate the Slate stripe opening on the panel fill, and headers
/// keep the fill. Cursor lines track the shifted document rows.
#[test]
fn settings_groups_space_and_stripe_data_rows() {
    let rows = vec![
        SettingsRow {
            label: "Group A".into(),
            value: String::new(),
            section: true,
            cursor: None,
        },
        SettingsRow {
            label: "alpha".into(),
            value: "1".into(),
            section: false,
            cursor: Some(0),
        },
        SettingsRow {
            label: "beta".into(),
            value: "2".into(),
            section: false,
            cursor: Some(1),
        },
        SettingsRow {
            label: "Group B".into(),
            value: String::new(),
            section: true,
            cursor: None,
        },
        SettingsRow {
            label: "gamma".into(),
            value: "3".into(),
            section: false,
            cursor: Some(2),
        },
    ];
    let (width, height) = (50, 14);
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
    let row_text = |y: u16| -> String {
        (0..width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    };
    let named_row = |name: &str| -> u16 {
        (0..height)
            .find(|&y| row_text(y).contains(name))
            .unwrap_or_else(|| panic!("{name} row painted"))
    };
    let body = palette::surface_colors(palette::Surface::SidebarBody, false).fill;
    // Spacer: the line between beta and the Group B header is blank.
    let beta_y = named_row("beta");
    let group_b_y = named_row("Group B");
    assert_eq!(group_b_y, beta_y + 2);
    assert!(row_text(beta_y + 1).trim().is_empty());
    // Zebra: first data row on the fill, second on the stripe, headers plain.
    let title_bg = |y: u16, name: &str| -> ratatui::style::Color {
        let line = row_text(y);
        let x = line.find(name).unwrap() as u16;
        buffer[(x, y)].bg
    };
    assert_eq!(title_bg(named_row("alpha"), "alpha"), body);
    assert_eq!(title_bg(beta_y, "beta"), palette::SETTINGS_STRIPE_BG);
    assert_eq!(title_bg(named_row("gamma"), "gamma"), body);
    assert_eq!(title_bg(named_row("Group A"), "Group A"), body);
    assert_eq!(title_bg(group_b_y, "Group B"), body);
    // Geometry follows the spaced document: cursor lines skip the spacer.
    assert_eq!(geometry.cursor_lines, vec![1, 2, 5]);
}

/// Overflow paints a visible sidebar scrollbar: at least one cell in the
/// scrollbar column carries the scrollbar role (it must read against the
/// sidebar body fill).
#[test]
fn settings_overflow_paints_a_visible_scrollbar() {
    let mut rows = vec![SettingsRow {
        label: "Group".into(),
        value: String::new(),
        section: true,
        cursor: None,
    }];
    rows.extend((0..12).map(|i| SettingsRow {
        label: format!("option_{i}"),
        value: "on".into(),
        section: false,
        cursor: Some(i),
    }));
    let (width, height) = (50, 10);
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
    let scrollbar_x = geometry.content_area.x + geometry.content_area.width;
    let body = palette::surface_colors(palette::Surface::SidebarBody, false).fill;
    assert_ne!(
        palette::SIDEBAR_SCROLLBAR,
        body,
        "the scrollbar role must contrast the body it paints over"
    );
    let painted = (geometry.content_area.y..geometry.content_area.y + geometry.content_area.height)
        .any(|y| {
            let cell = &buffer[(scrollbar_x, y)];
            // Track cells are blank by design (thumb-only bar); only the
            // thumb may mark the column.
            cell.fg == palette::SIDEBAR_SCROLLBAR && cell.symbol() != " "
        });
    assert!(painted, "overflow must paint the scrollbar thumb");
}
