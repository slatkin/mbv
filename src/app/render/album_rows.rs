use super::super::ui_util::trunc_str;
use super::album_plan::ArtistGroupHeader;
use super::list_rows::{
    focused_aqua_or_muted, focused_or_muted, focused_or_muted_soft_white, focused_or_subtle,
};
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;

/// Render-time inputs for `render_album_row`, bundled so the call takes one
/// struct instead of a long positional argument list (mirrors the
/// `ListRenderCtx` pattern in `list_rows.rs`).
pub(super) struct AlbumRowCtx<'a> {
    pub row_area: Rect,
    pub idx: usize,
    pub album_info: &'a [(String, String, String)],
    pub cursor: usize,
    pub avail: usize,
    pub selected_block_bounds: Option<(usize, usize)>,
    pub in_music_group_view: bool,
    pub abs_row_idx: usize,
    pub selected_art_reserved_w: u16,
    pub focused: bool,
}

impl App {
    pub(super) fn render_artist_header_row(
        &self,
        f: &mut Frame,
        row_area: Rect,
        header: &ArtistGroupHeader,
        in_music_group_view: bool,
        selected_block_bounds: Option<(usize, usize)>,
        abs_row_idx: usize,
        selected_art_reserved_w: u16,
    ) {
        let in_selected_block = selected_block_bounds
            .is_some_and(|(top, bottom)| abs_row_idx > top && abs_row_idx < bottom);
        let grouped_block = in_music_group_view && in_selected_block;
        let label_area = if in_selected_block {
            Rect {
                width: row_area.width.saturating_sub(selected_art_reserved_w),
                ..row_area
            }
        } else {
            row_area
        };
        let gutter_w = if grouped_block { 2 } else { 1 };
        let label_avail = (label_area.width as usize).saturating_sub(gutter_w);
        let artist_label = trunc_str(&header.artist_label, label_avail);
        let label_style = Style::default().fg(palette::YELLOW);
        let mut spans = Vec::with_capacity(3);
        if grouped_block {
            spans.push(super::selection_marker(false));
            spans.push(Span::raw(" "));
        } else {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(artist_label, label_style));
        f.render_widget(Paragraph::new(Line::from(spans)), label_area);
    }

    pub(super) fn render_album_row(&self, f: &mut Frame, ctx: AlbumRowCtx) {
        let AlbumRowCtx {
            row_area,
            idx,
            album_info,
            cursor,
            avail,
            selected_block_bounds,
            in_music_group_view,
            abs_row_idx,
            selected_art_reserved_w,
            focused,
        } = ctx;
        let selected = idx == cursor;
        let (_, year_str, album_name) = &album_info[idx];
        let suffix_w = if year_str.is_empty() {
            0
        } else {
            year_str.chars().count() + 2
        };
        let lead_w = 1;
        let name_w = avail.saturating_sub(lead_w + suffix_w);
        let trunc_name = trunc_str(album_name, name_w);
        let in_selected_block = selected_block_bounds
            .is_some_and(|(top, bottom)| abs_row_idx > top && abs_row_idx < bottom);
        let grouped_block = in_music_group_view && in_selected_block;

        if grouped_block {
            let content_width = row_area
                .width
                .saturating_sub(selected_art_reserved_w)
                .saturating_sub(2);
            let suffix = if year_str.is_empty() {
                String::new()
            } else {
                format!("  {year_str}")
            };
            let suffix_width = suffix.chars().count() as u16;
            let title_width = content_width.saturating_sub(suffix_width).max(1);
            let wrapped = wrap(album_name, title_width as usize);
            let wrapped_len = wrapped.len();
            let title_lines: Vec<Line> = wrapped
                .into_iter()
                .enumerate()
                .map(|(line_idx, line)| {
                    let mut spans = if line_idx == 0 {
                        if selected {
                            vec![Span::raw("  ")]
                        } else {
                            vec![Span::raw(" ")]
                        }
                    } else {
                        vec![Span::raw("  ")]
                    };
                    let title_style = if selected {
                        Style::default()
                            .fg(palette::YELLOW)
                            .bg(palette::PLAYBACK_PANEL_BG)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(focused_or_subtle(focused))
                    };
                    spans.push(Span::styled(line.into_owned(), title_style));
                    if line_idx + 1 == wrapped_len && !suffix.is_empty() {
                        spans.push(Span::styled(
                            "  ",
                            Style::default().fg(focused_or_muted(focused)),
                        ));
                        spans.push(Span::styled(
                            year_str.as_str(),
                            Style::default().fg(focused_aqua_or_muted(focused)),
                        ));
                    }
                    Line::from(spans)
                })
                .collect();
            f.render_widget(
                Paragraph::new(title_lines.clone()),
                Rect {
                    width: row_area.width.saturating_sub(selected_art_reserved_w),
                    height: title_lines.len() as u16,
                    ..row_area
                },
            );
            return;
        }

        // Detect if this album is inside a colored block frame
        let has_block = selected
            && selected_block_bounds.is_some_and(|(top_pad_abs, _)| abs_row_idx == top_pad_abs + 1);

        if has_block {
            let content_width = row_area
                .width
                .saturating_sub(selected_art_reserved_w)
                .saturating_sub(1);
            let suffix = if year_str.is_empty() {
                String::new()
            } else {
                format!("  {year_str}")
            };
            let suffix_width = suffix.chars().count();
            let title_lines: Vec<Line> = wrap(
                album_name,
                content_width.saturating_sub(suffix_width as u16).max(1) as usize,
            )
            .into_iter()
            .enumerate()
            .map(|(line_idx, line)| {
                let mut spans = vec![
                    Span::raw(" "),
                    Span::styled(
                        line.into_owned(),
                        Style::default()
                            .fg(palette::YELLOW)
                            .bg(palette::PLAYBACK_PANEL_BG)
                            .add_modifier(Modifier::BOLD),
                    ),
                ];
                if line_idx + 1
                    == wrap(
                        album_name,
                        content_width.saturating_sub(suffix_width as u16).max(1) as usize,
                    )
                    .len()
                    && !suffix.is_empty()
                {
                    spans.push(Span::styled(
                        "  ",
                        Style::default()
                            .fg(focused_or_muted(focused))
                            .bg(palette::PLAYBACK_PANEL_BG),
                    ));
                    spans.push(Span::styled(
                        year_str.as_str(),
                        Style::default()
                            .fg(focused_aqua_or_muted(focused))
                            .bg(palette::PLAYBACK_PANEL_BG),
                    ));
                }
                Line::from(spans)
            })
            .collect();
            f.render_widget(
                Paragraph::new(title_lines.clone()),
                Rect {
                    width: row_area.width.saturating_sub(selected_art_reserved_w),
                    height: title_lines.len() as u16,
                    ..row_area
                },
            );
            return;
        }

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::raw(" "));

        let title_style = if selected {
            Style::default()
                .fg(palette::YELLOW)
                .bg(palette::PLAYBACK_PANEL_BG)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(focused_or_subtle(focused))
        };
        spans.push(Span::styled(trunc_name, title_style));
        if !year_str.is_empty() {
            spans.push(Span::styled(
                "  ",
                Style::default()
                    .fg(focused_or_muted(focused))
                    .bg(palette::PLAYBACK_PANEL_BG),
            ));
            spans.push(Span::styled(
                year_str.as_str(),
                Style::default()
                    .fg(focused_aqua_or_muted(focused))
                    .bg(palette::PLAYBACK_PANEL_BG),
            ));
        }

        let album_area = row_area;
        f.render_widget(Paragraph::new(Line::from(spans)), album_area);
    }

    pub(super) fn render_wide_selected_album_row(
        &self,
        f: &mut Frame,
        row_area: Rect,
        panel_area: Rect,
        idx: usize,
        album_info: &[(String, String, String)],
    ) {
        let (_, year, title) = &album_info[idx];
        f.render_widget(
            Block::default().style(Style::default().bg(palette::LIBRARY_SIDE_BG)),
            Rect {
                x: panel_area.x,
                y: row_area.y,
                width: panel_area.width,
                height: 1,
            },
        );

        let suffix = if year.is_empty() {
            String::new()
        } else {
            format!("  {year}")
        };
        let title_width = panel_area
            .width
            .saturating_sub(3 + suffix.chars().count() as u16) as usize;
        let title = trunc_str(title, title_width);
        let mut spans = vec![
            Span::styled("▍", Style::default().fg(palette::AQUA)),
            Span::raw(" "),
            Span::raw(" "),
            Span::styled(
                title,
                Style::default()
                    .fg(palette::WHITE)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if !suffix.is_empty() {
            spans.push(Span::styled(suffix, Style::default().fg(palette::GREEN)));
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: panel_area.x,
                y: row_area.y,
                width: panel_area.width,
                height: 1,
            },
        );
    }

    pub(super) fn render_album_action_hint(
        &self,
        f: &mut Frame,
        row_area: Rect,
        in_music_group_view: bool,
        selected_block_bounds: Option<(usize, usize)>,
        abs_row_idx: usize,
        selected_art_reserved_w: u16,
        lib_idx: usize,
        focused: bool,
    ) {
        let in_selected_block = selected_block_bounds
            .is_some_and(|(top, bottom)| abs_row_idx > top && abs_row_idx < bottom);
        let hint = if in_music_group_view
            && in_selected_block
            && self.libs[lib_idx].album_track_focus.is_some()
        {
            "^P: Play | ^A: Enqueue | ^S: Shuffle | BACK: Exit"
        } else {
            "^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks"
        };
        let gutter_w = if in_music_group_view && in_selected_block {
            2
        } else {
            1
        };
        let hint_width = row_area
            .width
            .saturating_sub(selected_art_reserved_w)
            .saturating_sub(gutter_w)
            .max(1) as usize;
        let hint_lines: Vec<Line> = wrap(hint, hint_width)
            .into_iter()
            .map(|line| {
                Line::from(vec![
                    Span::raw(" ".repeat(gutter_w as usize)),
                    Span::styled(
                        line.into_owned(),
                        Style::default().fg(focused_or_muted_soft_white(focused)),
                    ),
                ])
            })
            .collect();
        f.render_widget(
            Paragraph::new(hint_lines.clone()),
            Rect {
                width: row_area.width.saturating_sub(selected_art_reserved_w),
                height: hint_lines.len() as u16,
                ..row_area
            },
        );
    }
}
