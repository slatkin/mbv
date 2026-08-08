use super::super::palette;
use super::super::search_sidebar::SearchSidebar;
use super::super::ui_util::trunc_str;
use super::super::{App, SEARCH_PANEL_W};
use mbv_core::api::EmbyItem;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

const HINTS: &str = "[\u{2191}\u{2193}]select [\u{21e5}]type [\u{21b5}]open [Esc]close";

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

impl App {
    pub(in crate::app::render) fn render_search_sidebar(
        &mut self,
        f: &mut Frame,
        area: Option<Rect>,
    ) {
        let content = match area {
            Some(area) => Self::render_panel_shell_at(f, area, "SEARCH", HINTS, true),
            None => Self::render_panel_shell(f, f.area(), SEARCH_PANEL_W, "SEARCH", HINTS),
        };
        let Some(sidebar) = self.search_sidebar.as_mut() else {
            return;
        };
        if content.height == 0 || content.width == 0 {
            return;
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
            return;
        }

        render_type_chips(
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
            return;
        }

        let list_area = Rect {
            x: content.x,
            y: content.y + 2,
            width: content.width,
            height: content.height - 2,
        };
        render_results(f, list_area, sidebar);
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
        Paragraph::new(Span::styled(line, Style::default().fg(palette::SOFT_WHITE))),
        area,
    );
}

fn render_type_chips(f: &mut Frame, area: Rect, sidebar: &SearchSidebar) {
    let types = sidebar.available_types();
    let mut chips: Vec<(String, bool)> = Vec::with_capacity(types.len() + 1);
    chips.push(("All".to_string(), sidebar.type_filter == 0));
    for (i, t) in types.iter().enumerate() {
        chips.push((badge_for(t).to_string(), sidebar.type_filter == i + 1));
    }
    let mut spans: Vec<Span> = Vec::new();
    for (i, (label, selected)) in chips.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let style = if *selected {
            Style::default()
                .fg(palette::PILL_SELECTOR_SELECTED_FG)
                .bg(palette::PILL_SELECTOR_SELECTED_BG)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::PILL_SELECTOR_FG)
        };
        spans.push(Span::styled(format!(" {label} "), style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_results(f: &mut Frame, area: Rect, sidebar: &mut SearchSidebar) {
    let list_h = area.height as usize;
    sidebar.list_height = list_h;
    let filtered: Vec<&EmbyItem> = sidebar.filtered_results();
    if filtered.is_empty() {
        render_empty_state(f, area, sidebar);
        return;
    }

    for (vi, item) in filtered.iter().skip(sidebar.scroll).enumerate() {
        if vi >= list_h {
            break;
        }
        let abs_idx = sidebar.scroll + vi;
        let selected = abs_idx == sidebar.cursor;
        let fg = if selected {
            palette::IRIS
        } else {
            palette::TEXT
        };
        let badge = badge_for(&item.item_type);
        let badge_str = format!("{badge:<10} ");
        let name_max = App::panel_row_text_width(area.width).saturating_sub(badge_str.len());
        let row_y = area.y + vi as u16;
        App::render_panel_row(
            f,
            area.x,
            row_y,
            area.width,
            selected,
            vec![
                Span::styled(
                    badge_str,
                    Style::default().fg(if selected {
                        palette::PILL_SELECTOR_SELECTED_FG
                    } else {
                        palette::AQUA
                    }),
                ),
                Span::styled(
                    trunc_str(&item.display_name(), name_max),
                    Style::default().fg(fg).add_modifier(Modifier::BOLD),
                ),
            ],
        );
    }
    App::render_sidebar_scrollbar(f, area, filtered.len(), sidebar.scroll);
}

fn render_empty_state(f: &mut Frame, area: Rect, sidebar: &SearchSidebar) {
    if area.height == 0 {
        return;
    }
    let (text, fg) = if let Some(err) = &sidebar.last_drain_error {
        (format!("Search failed: {err}"), palette::RED)
    } else if !sidebar.query.is_empty() {
        ("No matches on the server".to_string(), palette::SUBTLE)
    } else {
        ("Type to search".to_string(), palette::SUBTLE)
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
