use crate::app::palette;
use crate::app::render::components::chrome;
use crate::app::search_sidebar::SearchSidebar;
use crate::app::ui_util::trunc_str;
use crate::app::SEARCH_PANEL_W;
use mbv_core::api::EmbyItem;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

const HINTS: &str = "[\u{2191}\u{2193}]select [\u{21e5}]type [\u{21b5}]open [Esc]close";

/// Geometry painted by the search sidebar, reused by its mouse hit-testing
/// (task 5.1, design.md D6).
pub(in crate::app) struct SearchSidebarRenderGeometry {
    /// The painted panel-shell rect — the outside-click boundary.
    pub frame: Rect,
    /// Painted result-row rect -> absolute filtered-results index.
    pub result_rows: Vec<(Rect, usize)>,
    /// Painted type-filter chip rect -> type_filter index (0 = All).
    pub chips: Vec<(Rect, usize)>,
}

fn badge_for(item_type: &str) -> &'static str {
    match item_type {
        "Movie" => "MOVIE",
        "Series" => "SERIES",
        "Season" => "SEASON",
        "Episode" => "EPISODE",
        "MusicAlbum" => "ALBUM",
        "Audio" => "TRACK",
        "MusicArtist" => "ARTIST",
        "BoxSet" => "COLLECTION",
        _ => "TYPE",
    }
}

/// Render the global Search sidebar (design D9, task 3.1/3.2).
///
/// Extracted from `impl App::render_search_sidebar` as a free function so
/// the Interactive Component (`SearchSidebarComponent`) can call it in
/// `view()` without `App` access. The sidebar state is passed directly.
pub(in crate::app) fn render_search_sidebar(
    f: &mut Frame,
    area: Option<Rect>,
    sidebar: &mut SearchSidebar,
) -> SearchSidebarRenderGeometry {
    let frame = area.unwrap_or_else(|| chrome::panel_shell_rect(f.area(), SEARCH_PANEL_W));
    let content = chrome::render_panel_shell_at(f, frame, "SEARCH", HINTS, area.is_some());
    if content.height == 0 || content.width == 0 {
        return SearchSidebarRenderGeometry {
            frame,
            result_rows: Vec::new(),
            chips: Vec::new(),
        };
    }

    render_query_row(
        f,
        Rect {
            x: content.x,
            y: content.y,
            width: content.width,
            height: 1,
        },
        sidebar,
    );
    if content.height == 1 {
        return SearchSidebarRenderGeometry {
            frame,
            result_rows: Vec::new(),
            chips: Vec::new(),
        };
    }

    let chips = render_type_chips(
        f,
        Rect {
            x: content.x,
            y: content.y + 1,
            width: content.width,
            height: 1,
        },
        sidebar,
    );
    if content.height == 2 {
        return SearchSidebarRenderGeometry {
            frame,
            result_rows: Vec::new(),
            chips,
        };
    }

    let list_area = Rect {
        x: content.x,
        y: content.y + 2,
        width: content.width,
        height: content.height - 2,
    };
    let result_rows = render_results(f, list_area, sidebar);
    SearchSidebarRenderGeometry {
        frame,
        result_rows,
        chips,
    }
}

fn render_query_row(f: &mut Frame, area: Rect, sidebar: &SearchSidebar) {
    let cursor = "\u{2588}";
    let indicator = if sidebar.loading {
        " [loading\u{2026}]"
    } else {
        ""
    };
    let text_w = area.width as usize;
    let query_max = text_w.saturating_sub(cursor.len() + indicator.len());
    let query_display = trunc_str(&sidebar.query, query_max);
    let line = format!("{query_display}{cursor}{indicator}");
    f.render_widget(
        Paragraph::new(Span::styled(
            line,
            Style::default().fg(palette::TEXT_EMPHASIS),
        )),
        area,
    );
}

fn render_type_chips(f: &mut Frame, area: Rect, sidebar: &SearchSidebar) -> Vec<(Rect, usize)> {
    let types = sidebar.available_types();
    let mut chips: Vec<(String, bool)> = Vec::with_capacity(types.len() + 1);
    chips.push(("All".to_string(), sidebar.type_filter == 0));
    for (i, t) in types.iter().enumerate() {
        chips.push((badge_for(t).to_string(), sidebar.type_filter == i + 1));
    }
    let mut spans: Vec<Span> = Vec::new();
    let mut chip_rects: Vec<(Rect, usize)> = Vec::new();
    let mut next_x = area.x;
    for (i, (label, selected)) in chips.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
            next_x += 1;
        }
        let width = (label.len() + 2).min(area.right().saturating_sub(next_x) as usize) as u16;
        if width > 0 {
            chip_rects.push((
                Rect {
                    x: next_x,
                    y: area.y,
                    width,
                    height: 1,
                },
                i,
            ));
        }
        next_x += width;
        let style = if *selected {
            Style::default()
                .fg(palette::PILL_SELECTED_FG)
                .bg(palette::PILL_SELECTED_BG)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::PILL_FG)
        };
        spans.push(Span::styled(format!(" {label} "), style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
    chip_rects
}

fn render_results(f: &mut Frame, area: Rect, sidebar: &mut SearchSidebar) -> Vec<(Rect, usize)> {
    let list_h = area.height as usize;
    sidebar.list_height = list_h;
    let filtered: Vec<&EmbyItem> = sidebar.filtered_results();
    if filtered.is_empty() {
        render_empty_state(f, area, sidebar);
        return Vec::new();
    }
    let mut rows = Vec::new();

    for (vi, item) in filtered.iter().skip(sidebar.scroll).enumerate() {
        if vi >= list_h {
            break;
        }
        let abs_idx = sidebar.scroll + vi;
        let selected = abs_idx == sidebar.cursor;
        let fg = if selected {
            palette::ACCENT_ACTIVE
        } else {
            palette::TEXT_PRIMARY
        };
        let badge = badge_for(&item.item_type);
        let badge_str = format!("{badge:<10} ");
        let name_max = chrome::panel_row_text_width(area.width).saturating_sub(badge_str.len());
        let row_y = area.y + vi as u16;
        chrome::render_panel_row(
            f,
            area.x,
            row_y,
            area.width,
            selected,
            vec![
                Span::styled(
                    badge_str,
                    Style::default().fg(if selected {
                        palette::PILL_SELECTED_FG
                    } else {
                        palette::ACCENT
                    }),
                ),
                Span::styled(
                    trunc_str(&item.display_name(), name_max),
                    Style::default().fg(fg).add_modifier(Modifier::BOLD),
                ),
            ],
        );
        rows.push((
            Rect {
                x: area.x,
                y: row_y,
                width: area.width,
                height: 1,
            },
            abs_idx,
        ));
    }
    chrome::render_sidebar_scrollbar(f, area, filtered.len(), sidebar.scroll);
    rows
}

fn render_empty_state(f: &mut Frame, area: Rect, sidebar: &SearchSidebar) {
    if area.height == 0 {
        return;
    }
    let (text, fg) = if let Some(err) = &sidebar.last_drain_error {
        (format!("Search failed: {err}"), palette::STATUS_ERROR)
    } else if !sidebar.query.is_empty() {
        (
            "No matches on the server".to_string(),
            palette::TEXT_SECONDARY,
        )
    } else {
        ("Type to search".to_string(), palette::TEXT_SECONDARY)
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            trunc_str(&text, area.width as usize),
            Style::default().fg(fg),
        )),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
    );
}
