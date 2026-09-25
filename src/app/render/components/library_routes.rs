use super::super::super::LibraryRouteStage;
use crate::app::infra::palette;
use crate::app::render::components::modal_frame::render_modal_frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

const LOCAL_NO_ROUTE: &str = "Local (no route)";

pub(in crate::app) fn save_route_config(cfg: &crate::config::Config) -> Result<(), String> {
    crate::config::save_config_settings(cfg)
}

pub(in crate::app) struct LibraryRoutesRenderModel<'a> {
    pub stage: &'a LibraryRouteStage,
    pub cursor: usize,
}

/// Geometry painted by the library-routes popup, reused by its mouse
/// hit-testing (task 5.1, design.md D6).
pub(in crate::app) struct LibraryRoutesRenderGeometry {
    /// The painted modal rect — the outside-click boundary.
    pub frame: Rect,
    /// Painted selectable-row rect -> cursor index.
    pub rows: Vec<(Rect, usize)>,
}

pub(in crate::app) fn render_library_routes_content(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    model: LibraryRoutesRenderModel<'_>,
) -> LibraryRoutesRenderGeometry {
    let (title, lines): (&str, Vec<Line>) = match model.stage {
        LibraryRouteStage::PickLibrary { items } => {
            let lines = items
                .iter()
                .enumerate()
                .map(|(i, (_, name, assigned))| {
                    let focused = i == model.cursor;
                    let arrow = if focused { "▸ " } else { "  " };
                    let name_style = if focused {
                        Style::default().fg(palette::TEXT_PRIMARY)
                    } else {
                        Style::default().fg(palette::TEXT_SECONDARY)
                    };
                    let value = assigned.clone().unwrap_or_else(|| "none".to_string());
                    Line::from(vec![
                        Span::raw(arrow),
                        Span::styled(name.clone(), name_style),
                        Span::raw(" -> "),
                        Span::styled(value, Style::default().fg(palette::TEXT_ACCENT_MUTED)),
                    ])
                })
                .collect();
            (" Library Routes ", lines)
        }
        LibraryRouteStage::PickDevice {
            library_display,
            devices,
            ..
        } => {
            let mut lines = vec![];
            if devices.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No other mbv devices found right now -- make sure the",
                    Style::default().fg(palette::TEXT_MUTED),
                )));
                lines.push(Line::from(Span::styled(
                    "target is running and connected.",
                    Style::default().fg(palette::TEXT_MUTED),
                )));
            }
            // (label, routable) -- a device without a resolvable
            // endpoint (#256) is shown greyed out with its reason
            // appended, rather than omitted, so a device visible in
            // F3 but not currently pickable here isn't a silent
            // mystery. It stays visible via arrow-key navigation but
            // `commit_device_selection` refuses to commit it.
            let mut rows: Vec<(String, bool)> = vec![(LOCAL_NO_ROUTE.to_string(), true)];
            rows.extend(devices.iter().map(|(name, endpoint)| {
                if endpoint.is_some() {
                    (name.clone(), true)
                } else {
                    (format!("{name} (not currently routable)"), false)
                }
            }));
            for (i, (label, routable)) in rows.iter().enumerate() {
                let focused = i == model.cursor;
                let arrow = if focused { "▸ " } else { "  " };
                let name_style = if !routable {
                    Style::default().fg(palette::TEXT_MUTED)
                } else if focused {
                    Style::default().fg(palette::TEXT_PRIMARY)
                } else {
                    Style::default().fg(palette::TEXT_SECONDARY)
                };
                lines.push(Line::from(vec![
                    Span::raw(arrow),
                    Span::styled(label.clone(), name_style),
                ]));
            }
            let _ = library_display;
            (" Pick Device ", lines)
        }
    };

    let max_w = lines.iter().map(|l| l.width()).max().unwrap_or(0);
    let inner_w = ((max_w + 6) as u16).clamp(36, 60);
    let width = inner_w + 2;
    let content_h = lines.len() as u16 + 1;
    let height = content_h + 2;

    let inner = render_modal_frame(
        f,
        dim_backdrop_active,
        title,
        width,
        height,
        palette::surface_colors(palette::Surface::PopupFrame, false).fill,
    );

    let hint = "Enter select  ·  Esc back/close";
    f.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(palette::TEXT_MUTED))),
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
    );
    let list_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };
    f.render_widget(Paragraph::new(lines), list_area);
    // Selectable rows sit below any informational lines (PickDevice's
    // "no other mbv devices" notice), so their rects must skip them.
    let info_lines = match &model.stage {
        LibraryRouteStage::PickDevice { devices, .. } => usize::from(devices.is_empty()) * 2,
        _ => 0,
    };
    let row_count = match &model.stage {
        LibraryRouteStage::PickLibrary { items } => items.len(),
        LibraryRouteStage::PickDevice { devices, .. } => devices.len() + 1,
    };
    let rows = (0..row_count)
        .take((list_area.height as usize).saturating_sub(info_lines))
        .map(|i| {
            (
                Rect {
                    x: list_area.x,
                    y: list_area.y + (info_lines + i) as u16,
                    width: list_area.width,
                    height: 1,
                },
                i,
            )
        })
        .collect();
    LibraryRoutesRenderGeometry { frame: inner, rows }
}
