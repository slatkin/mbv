// The search-modal renderer is being introduced in stages. This file ships
// the new `render_search_modal` function and its helpers; Round 4 (input)
// is responsible for dispatching it from `App::render` when
// `app.search_modal.is_some()`. Until then, every helper here is unused
// outside this file. Remove the module-level `#[allow(dead_code)]` in the
// change that adds the first non-test call site.

#![allow(dead_code)]

use super::super::super::palette;
use super::super::super::search_modal::SearchMode;
use super::super::super::ui_util::{fmt_duration_approx, trunc_str};
use super::super::super::App;
use super::modal_frame::render_modal_frame;
use mbv_core::api::{MediaItem, TICKS_PER_SECOND};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use textwrap::wrap;

const MIN_W: u16 = 40;
const MIN_H: u16 = 12;
const INPUT_ROW_H: u16 = 3;
const TYPE_FILTER_H: u16 = 1;
const ROW_H: u16 = 2;
const HERO_BORDER_ROWS: u16 = 2;
const HERO_MIN_CONTENT_ROWS: u16 = 2;
const BADGE_COL_W: u16 = 11;

const TITLE: &str = " Search ";

pub(in crate::app::render) fn render_search_modal(app: &mut App, f: &mut Frame, area: Rect) {
    let modal = match app.search_modal.as_ref() {
        Some(m) => m,
        None => return,
    };
    let (w, h) = compute_modal_size(area);
    let inner = render_modal_frame(
        f,
        &mut app.dim_backdrop_active,
        TITLE,
        w,
        h,
        palette::LIBRARY_SIDE_BG,
    );
    let (input_area, filter_area, body_area) = layout_inner(inner, modal.mode);

    if input_area.height >= INPUT_ROW_H {
        render_input_row(f, input_area, modal);
    }

    let show_filter =
        matches!(modal.mode, SearchMode::Global) && modal.available_types().len() >= 2;
    if show_filter && filter_area.height >= TYPE_FILTER_H {
        render_type_filter(f, filter_area, modal);
    }

    let show_loading = match modal.mode {
        SearchMode::Fuzzy => modal.loading && modal.corpus.is_empty(),
        SearchMode::Global => modal.loading && modal.results.is_empty(),
    };
    if body_area.height == 0 {
        return;
    }

    if show_loading {
        render_state_message(
            f,
            body_area,
            "Loading…",
            &[],
            palette::SUBTLE,
            palette::LIBRARY_SIDE_BG,
        );
        return;
    }

    let results = modal.filtered_results();
    let n = results.len();
    if n == 0 {
        if modal.query.is_empty() {
            render_state_message(
                f,
                body_area,
                "Type to search",
                &[],
                palette::SUBTLE,
                palette::LIBRARY_SIDE_BG,
            );
            return;
        }
        let (primary, secondary): (&str, &[&str]) = match modal.mode {
            SearchMode::Fuzzy => (
                "No matches in this library",
                &["Press \u{2018}/\u{2018} again to search the whole server"],
            ),
            SearchMode::Global => ("No matches on the server", &[]),
        };
        if modal.last_drain_error.is_some() {
            let err = modal.last_drain_error.as_deref().unwrap_or("Search failed");
            render_state_message(
                f,
                body_area,
                "Search failed",
                &[err],
                palette::RED,
                palette::LIBRARY_SIDE_BG,
            );
            return;
        }
        render_state_message(
            f,
            body_area,
            primary,
            secondary,
            palette::SOFT_WHITE,
            palette::LIBRARY_SIDE_BG,
        );
        return;
    }

    let selected = &results[modal.cursor.min(n - 1)];
    let hero_h = compute_hero_height(selected, body_area.width, body_area.height);
    if let Some(modal_mut) = app.search_modal.as_mut() {
        adjust_scroll(modal_mut, body_area.height, hero_h);
    }
    let modal = app.search_modal.as_ref().unwrap();
    let results = modal.filtered_results();
    let selected = &results[modal.cursor.min(results.len() - 1)];
    render_results(f, body_area, modal, &results, selected, hero_h);
}

fn compute_modal_size(area: Rect) -> (u16, u16) {
    let w = (area.width * 60 / 100).max(MIN_W).min(area.width);
    let h = (area.height * 80 / 100).max(MIN_H).min(area.height);
    (w, h)
}

fn layout_inner(inner: Rect, mode: SearchMode) -> (Rect, Rect, Rect) {
    let show_filter = matches!(mode, SearchMode::Global);
    let input_h = INPUT_ROW_H.min(inner.height);
    let input_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: input_h,
    };
    let after_input = inner.height.saturating_sub(input_h);
    let filter_h = if show_filter {
        TYPE_FILTER_H.min(after_input)
    } else {
        0
    };
    let filter_area = Rect {
        x: inner.x,
        y: inner.y + input_h,
        width: inner.width,
        height: filter_h,
    };
    let body_area = Rect {
        x: inner.x,
        y: inner.y + input_h + filter_h,
        width: inner.width,
        height: inner.height.saturating_sub(input_h + filter_h),
    };
    (input_area, filter_area, body_area)
}

fn render_input_row(
    f: &mut Frame,
    area: Rect,
    modal: &super::super::super::search_modal::SearchModal,
) {
    let show_loading = match modal.mode {
        SearchMode::Fuzzy => modal.loading && modal.corpus.is_empty(),
        SearchMode::Global => modal.loading,
    };
    let text = if show_loading && modal.query.is_empty() {
        "\u{2588} [loading\u{2026}]".to_string()
    } else {
        format!("{}\u{2588}", modal.query)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette::SEEK_TRACK))
        .style(Style::default().bg(palette::PLAYBACK_PANEL_BG));
    f.render_widget(block, area);
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    if inner.width == 0 {
        return;
    }
    f.render_widget(
        Paragraph::new(Span::styled(
            text,
            Style::default()
                .fg(palette::SOFT_WHITE)
                .bg(palette::PLAYBACK_PANEL_BG),
        )),
        inner,
    );
}

fn render_type_filter(
    f: &mut Frame,
    area: Rect,
    modal: &super::super::super::search_modal::SearchModal,
) {
    let types = modal.available_types();
    if types.len() < 2 {
        return;
    }
    let mut spans: Vec<Span> = Vec::new();
    let selected = modal.type_filter;
    let mut chips: Vec<(&str, bool)> = Vec::with_capacity(types.len() + 1);
    chips.push(("All", selected == 0));
    for (i, t) in types.iter().enumerate() {
        chips.push((badge_for(t), selected == i + 1));
    }
    for (i, (label, is_selected)) in chips.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(
                " ",
                Style::default().bg(palette::LIBRARY_SIDE_BG),
            ));
        }
        let style = if *is_selected {
            Style::default()
                .fg(palette::PILL_SELECTOR_SELECTED_FG)
                .bg(palette::PILL_SELECTOR_SELECTED_BG)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(palette::PILL_SELECTOR_FG)
                .bg(palette::LIBRARY_SIDE_BG)
        };
        spans.push(Span::styled(format!(" {} ", label), style));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
    );
}

fn render_state_message(
    f: &mut Frame,
    area: Rect,
    primary: &str,
    secondary: &[&str],
    fg: Color,
    bg: Color,
) {
    if area.height == 0 {
        return;
    }
    f.render_widget(Block::default().style(Style::default().bg(bg)), area);
    let primary_y = area.y + area.height / 2;
    if primary_y < area.y + area.height {
        f.render_widget(
            Paragraph::new(Span::styled(primary, Style::default().fg(fg).bg(bg)))
                .alignment(ratatui::layout::Alignment::Center),
            Rect {
                x: area.x,
                y: primary_y,
                width: area.width,
                height: 1,
            },
        );
    }
    let secondary_y = primary_y + 1;
    if secondary_y < area.y + area.height {
        for (i, line) in secondary.iter().enumerate() {
            let y = secondary_y + i as u16;
            if y >= area.y + area.height {
                break;
            }
            f.render_widget(
                Paragraph::new(Span::styled(
                    *line,
                    Style::default().fg(palette::MUTED).bg(bg),
                ))
                .alignment(ratatui::layout::Alignment::Center),
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
            );
        }
    }
}

fn adjust_scroll(
    modal: &mut super::super::super::search_modal::SearchModal,
    body_height: u16,
    hero_h: u16,
) {
    if body_height == 0 {
        return;
    }
    let total_rows = modal.filtered_results().len();
    if total_rows == 0 {
        modal.scroll = 0;
        return;
    }
    let cursor = modal.cursor.min(total_rows - 1);
    let visible_rows = (body_height / ROW_H) as usize;
    if cursor < modal.scroll {
        modal.scroll = cursor;
        return;
    }
    let row_h = ROW_H as usize;
    let bh = body_height as usize;
    let hh = hero_h as usize;
    if cursor * row_h + hh > modal.scroll * row_h + bh {
        let desired = (cursor * row_h + hh - bh) / row_h;
        let max_scroll = total_rows.saturating_sub(visible_rows);
        modal.scroll = desired.min(max_scroll);
    }
}

fn render_results(
    f: &mut Frame,
    body_area: Rect,
    modal: &super::super::super::search_modal::SearchModal,
    results: &[&MediaItem],
    selected: &MediaItem,
    hero_h: u16,
) {
    if body_area.height == 0 {
        return;
    }
    f.render_widget(
        Block::default().style(Style::default().bg(palette::LIBRARY_SIDE_BG)),
        body_area,
    );
    let total = results.len();
    let visible_rows = (body_area.height / ROW_H) as usize;
    let cursor = modal.cursor.min(total.saturating_sub(1));
    let scroll = modal.scroll.min(cursor);

    let hero_y = body_area.y + ((cursor.saturating_sub(scroll)) as u16) * ROW_H + ROW_H;

    for vi in 0..visible_rows {
        let abs = scroll + vi;
        if abs >= total {
            break;
        }
        let item = results[abs];
        let is_selected = abs == cursor;
        let row_y = body_area.y + (vi as u16) * ROW_H;
        if row_y + ROW_H > body_area.y + body_area.height {
            break;
        }
        let line_bg = if is_selected {
            palette::MEDIA_SELECTED_BG
        } else {
            palette::LIBRARY_SIDE_BG
        };
        let line_fg = if is_selected {
            palette::SOFT_WHITE
        } else {
            palette::TEXT
        };
        let title_style = if is_selected {
            Style::default()
                .fg(line_fg)
                .bg(line_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(line_fg).bg(line_bg)
        };
        let meta_style = if is_selected {
            Style::default()
                .fg(palette::PILL_SELECTOR_SELECTED_FG)
                .bg(line_bg)
        } else {
            Style::default().fg(palette::MUTED).bg(line_bg)
        };
        let title_row = Rect {
            x: body_area.x,
            y: row_y,
            width: body_area.width,
            height: 1,
        };
        let meta_row = Rect {
            x: body_area.x,
            y: row_y + 1,
            width: body_area.width,
            height: 1,
        };
        f.render_widget(
            Block::default().style(Style::default().bg(line_bg)),
            Rect {
                x: body_area.x,
                y: row_y,
                width: body_area.width,
                height: ROW_H,
            },
        );
        let badge = badge_for(&item.item_type);
        let title_w = (body_area.width as usize).saturating_sub(BADGE_COL_W as usize);
        let title_text = trunc_str(&item.display_name(), title_w);
        let mut title_spans: Vec<Span> = Vec::new();
        title_spans.push(Span::styled(
            format!("{:<10} ", badge),
            Style::default()
                .fg(if is_selected {
                    palette::PILL_SELECTOR_SELECTED_FG
                } else {
                    palette::AQUA
                })
                .bg(line_bg)
                .add_modifier(Modifier::BOLD),
        ));
        title_spans.push(Span::styled(title_text, title_style));
        f.render_widget(Paragraph::new(Line::from(title_spans)), title_row);
        let meta_text = row_meta_for(item);
        let meta_indent = " ".repeat(BADGE_COL_W as usize);
        let meta_trunc = trunc_str(&meta_text, body_area.width as usize);
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("{meta_indent}{meta_trunc}"),
                meta_style,
            )),
            meta_row,
        );
    }

    if hero_h > 0 && hero_y < body_area.y + body_area.height {
        let hero_area = Rect {
            x: body_area.x,
            y: hero_y,
            width: body_area.width,
            height: hero_h.min(body_area.y + body_area.height - hero_y),
        };
        if hero_area.height >= HERO_BORDER_ROWS + HERO_MIN_CONTENT_ROWS {
            render_hero(f, hero_area, selected, body_area.width);
        }
    }
}

fn render_hero(f: &mut Frame, area: Rect, item: &MediaItem, full_width: u16) {
    let border_style = Style::default().fg(palette::SEEK_TRACK);
    f.render_widget(
        Block::default().style(Style::default().bg(palette::MEDIA_SELECTED_BG)),
        Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(2),
        },
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "\u{2581}".repeat(area.width as usize),
            border_style,
        ))),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "\u{2594}".repeat(area.width as usize),
            border_style,
        ))),
        Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        },
    );
    let text_w = full_width as usize;
    let overview_lines: Vec<String> = if item.overview.is_empty() {
        Vec::new()
    } else {
        wrap(&item.overview, text_w.max(1))
            .into_iter()
            .map(|s| s.into_owned())
            .collect()
    };
    let mut row = area.y + 1;
    let max_y = area.y + area.height - 1;
    if row < max_y {
        let title = trunc_str(&item.display_name(), text_w);
        f.render_widget(
            Paragraph::new(Span::styled(
                title,
                Style::default()
                    .fg(palette::SOFT_WHITE)
                    .bg(palette::MEDIA_SELECTED_BG)
                    .add_modifier(Modifier::BOLD),
            )),
            Rect {
                x: area.x,
                y: row,
                width: area.width,
                height: 1,
            },
        );
        row += 1;
    }
    if row < max_y {
        let meta = trunc_str(&hero_meta_for(item), text_w);
        f.render_widget(
            Paragraph::new(Span::styled(
                meta,
                Style::default()
                    .fg(palette::PILL_SELECTOR_SELECTED_FG)
                    .bg(palette::MEDIA_SELECTED_BG),
            )),
            Rect {
                x: area.x,
                y: row,
                width: area.width,
                height: 1,
            },
        );
        row += 1;
    }
    for line in &overview_lines {
        if row >= max_y {
            break;
        }
        f.render_widget(
            Paragraph::new(Span::styled(
                line.clone(),
                Style::default()
                    .fg(palette::TEXT)
                    .bg(palette::MEDIA_SELECTED_BG),
            )),
            Rect {
                x: area.x,
                y: row,
                width: area.width,
                height: 1,
            },
        );
        row += 1;
    }
}

fn compute_hero_height(item: &MediaItem, width: u16, available_h: u16) -> u16 {
    let text_w = width as usize;
    let overview_rows: usize = if item.overview.is_empty() {
        0
    } else {
        wrap(&item.overview, text_w.max(1)).len().min(3)
    };
    let content = HERO_MIN_CONTENT_ROWS as usize + overview_rows;
    let total = HERO_BORDER_ROWS as usize + content;
    let cap = (available_h.saturating_sub(ROW_H * 2) as usize)
        .max(HERO_BORDER_ROWS as usize + HERO_MIN_CONTENT_ROWS as usize);
    (total as u16).min(cap as u16)
}

fn badge_for(item_type: &str) -> &'static str {
    match item_type {
        "Movie" => "MOVIE",
        "Series" => "SERIES",
        "Episode" => "EPISODE",
        "MusicAlbum" => "ALBUM",
        "Audio" => "TRACK",
        "MusicArtist" => "ARTIST",
        "BoxSet" => "COLLECTION",
        _ => "TYPE",
    }
}

fn row_meta_for(item: &MediaItem) -> String {
    let mut parts: Vec<String> = Vec::new();
    match item.item_type.as_str() {
        "Movie" => {
            if item.production_year > 0 {
                parts.push(item.production_year.to_string());
            }
            let dur = runtime_approx(item);
            if !dur.is_empty() {
                parts.push(dur);
            }
            if !item.genre.is_empty() {
                parts.push(item.genre.clone());
            }
        }
        "Series" => {
            if item.production_year > 0 {
                parts.push(item.production_year.to_string());
            }
            if item.total_count > 0 {
                parts.push(format!("{} seasons", item.total_count));
            }
        }
        "Episode" => {
            if !item.series_name.is_empty() {
                parts.push(item.series_name.clone());
            }
            let se = se_label(item);
            if !se.is_empty() {
                parts.push(se);
            }
            let dur = runtime_approx(item);
            if !dur.is_empty() {
                parts.push(dur);
            }
        }
        "MusicAlbum" => {
            if !item.album.is_empty() {
                parts.push(item.album.clone());
            }
            if item.production_year > 0 {
                parts.push(item.production_year.to_string());
            }
            if item.total_count > 0 {
                parts.push(format!("{} tracks", item.total_count));
            }
        }
        "Audio" => {
            if !item.artist.is_empty() {
                parts.push(item.artist.clone());
            }
            if !item.album.is_empty() {
                parts.push(item.album.clone());
            }
            let dur = runtime_mmss(item);
            if !dur.is_empty() {
                parts.push(dur);
            }
        }
        "MusicArtist" => {
            if item.total_count > 0 {
                parts.push(format!("{} albums", item.total_count));
            }
        }
        "BoxSet" if item.total_count > 0 => {
            parts.push(format!("{} items", item.total_count));
        }
        _ => {}
    }
    parts.join(" \u{00b7} ")
}

fn hero_meta_for(item: &MediaItem) -> String {
    let mut parts: Vec<String> = Vec::new();
    match item.item_type.as_str() {
        "Movie" => {
            if item.production_year > 0 {
                parts.push(item.production_year.to_string());
            }
            let dur = runtime_approx(item);
            if !dur.is_empty() {
                parts.push(dur);
            }
            if !item.genre.is_empty() {
                parts.push(item.genre.clone());
            }
        }
        "Series" => {
            if item.production_year > 0 {
                parts.push(item.production_year.to_string());
            }
            if item.total_count > 0 {
                parts.push(format!("{} seasons", item.total_count));
            }
        }
        "Episode" => {
            let se = se_label(item);
            if !se.is_empty() {
                parts.push(se);
            }
            let dur = runtime_approx(item);
            if !dur.is_empty() {
                parts.push(dur);
            }
        }
        "MusicAlbum" => {
            if !item.album.is_empty() {
                parts.push(item.album.clone());
            }
            if item.production_year > 0 {
                parts.push(item.production_year.to_string());
            }
        }
        "Audio" => {
            if !item.artist.is_empty() {
                parts.push(item.artist.clone());
            }
            if !item.album.is_empty() {
                parts.push(item.album.clone());
            }
            let dur = runtime_mmss(item);
            if !dur.is_empty() {
                parts.push(dur);
            }
        }
        "MusicArtist" => {
            if item.total_count > 0 {
                parts.push(format!("{} albums", item.total_count));
            }
        }
        "BoxSet" if item.total_count > 0 => {
            parts.push(format!("{} items", item.total_count));
        }
        _ => {}
    }
    parts.join(" \u{00b7} ")
}

fn runtime_approx(item: &MediaItem) -> String {
    if item.runtime_ticks > 0 {
        fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
    } else {
        String::new()
    }
}

fn runtime_mmss(item: &MediaItem) -> String {
    if item.runtime_ticks > 0 {
        let secs = item.runtime_ticks / TICKS_PER_SECOND;
        let m = secs / 60;
        let s = secs % 60;
        format!("{}:{:02}", m, s)
    } else {
        String::new()
    }
}

fn se_label(item: &MediaItem) -> String {
    let season = item.parent_index_number;
    let episode = item.index_number;
    if season <= 0 && episode <= 0 {
        return String::new();
    }
    format!("S{}E{}", season, episode)
}
