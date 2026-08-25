use crate::app::layout::LayoutMain;
use crate::app::render::components::album_art::inline_album_art_cache_key;
use crate::app::render::components::hero::{HeroContent, HeroImage};
use crate::app::render::components::music_wide::wide_album_metadata;
use crate::app::selection_modal_actions::album_track_title;
use crate::app::ui_util::*;
use crate::app::{palette, App};
use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;

const INLINE_ALBUM_TITLE_EXTRA_INDENT: u16 = 1;
const INLINE_ALBUM_TRACK_EXTRA_INDENT: u16 = 2;

/// Image box size for the Model A album hero (`render_album_hero_detail`),
/// matching the Series inline hero's `SERIES_IMAGE_*` sizing (`detail_series.rs`)
/// so both Model A surfaces reserve comparable image rows.
const ALBUM_HERO_IMAGE_COLS: u16 = 18;
const ALBUM_HERO_IMAGE_ROWS: u16 = 12;
const ALBUM_HERO_IMAGE_PLACEHOLDER_ROWS: u16 = 10;

/// Row budget for the Model A album hero's *content* (title through the
/// trailing blank row) -- the caller adds its own block framing
/// (`HERO_BLOCK_EXTRA_ROWS`) on top, mirroring
/// `App::series_inline_detail_rows`'s split between this estimate (used by
/// `build_grouped_album_display_plan` to reserve space) and the actual paint
/// call (`render_album_hero_detail`).
pub(in crate::app::render) fn album_hero_detail_rows(images_enabled: bool) -> usize {
    let mut rows = 1 /* title */ + 1 /* meta */ + 1 /* spacer */;
    let img_rows = if images_enabled {
        ALBUM_HERO_IMAGE_ROWS as usize
    } else {
        0
    };
    // The trailing spacer is also the image's bottom gutter.
    rows = rows.max(img_rows);
    rows + 1 /* trailing spacer */
}

impl App {
    /// Renders the narrow grouped-album Model A hero (task 3.2): title,
    /// "artist • year" meta, and right-aligned album art via
    /// `paint_hero_content` -- no track table, no action hints (those moved
    /// to the selection modal, see `open_album_selection_modal`). Mirrors
    /// `render_series_inline_detail`'s image-sizing shape, simplified since
    /// albums have no overview text.
    pub(in crate::app::render) fn render_album_hero_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        album: &EmbyItem,
        focused: bool,
    ) {
        if area.height == 0 {
            return;
        }

        let artist = self.resolve_group_album_artist(album);
        let (title, year) = wide_album_metadata(album, &artist);
        let show_artist = !artist.is_empty() && artist != "Unknown Artist";
        let meta = match (show_artist, year > 0) {
            (true, true) => format!("{artist} • {year}"),
            (true, false) => artist,
            (false, true) => year.to_string(),
            (false, false) => String::new(),
        };

        let cache_key = inline_album_art_cache_key(&album.id);
        if !album.id.is_empty() && self.images_enabled() {
            self.fetch_card_image(
                cache_key.clone(),
                album.id.clone(),
                album.series_id.clone(),
                crate::app::render::MUSIC_ALBUM_IMAGE_TYPES,
            );
        }
        let img_loading = !album.id.is_empty()
            && self.images_enabled()
            && self.card_image_loading.contains(&cache_key);
        let (img_actual_w, img_height, img_is_placeholder): (u16, u16, bool) = {
            if let Some(state) = self.cached_image_protocol_mut(&cache_key) {
                let avail = ratatui::layout::Size {
                    width: ALBUM_HERO_IMAGE_COLS,
                    height: ALBUM_HERO_IMAGE_ROWS,
                };
                match state.size_for(
                    ratatui_image::Resize::Scale(Some(crate::app::render::RENDER_FILTER)),
                    avail,
                ) {
                    Some(actual) => (actual.width, actual.height, false),
                    None => (
                        ALBUM_HERO_IMAGE_COLS,
                        ALBUM_HERO_IMAGE_PLACEHOLDER_ROWS,
                        true,
                    ),
                }
            } else if img_loading {
                (
                    ALBUM_HERO_IMAGE_COLS,
                    ALBUM_HERO_IMAGE_PLACEHOLDER_ROWS,
                    true,
                )
            } else {
                (0, 0, false)
            }
        };

        let hero_content = HeroContent {
            title: Some(title.as_str()),
            meta_line: (!meta.is_empty()).then_some(meta.as_str()),
            meta_color: palette::TEXT_DETAIL_META,
            show_playing: false,
            unconditional_spacer_after_meta: true,
            lines: &[],
            image: (img_height > 0).then_some(HeroImage {
                actual_w: img_actual_w,
                height: img_height,
            }),
        };
        let result = crate::app::render::components::hero::paint_hero_content(
            f,
            area,
            &hero_content,
            focused,
        );

        if let Some(img_rect) = result.img_rect {
            if img_is_placeholder {
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
                    img_rect,
                );
            } else if let Some(state) = self.cached_image_protocol_mut(&cache_key) {
                type AImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                f.render_stateful_widget(
                    AImg::default().resize(ratatui_image::Resize::Scale(Some(
                        crate::app::render::RENDER_FILTER,
                    ))),
                    img_rect,
                    state,
                );
            }
        }
    }

    /// Renders the music album detail panel (track list) into `area` — the lib
    /// slot below the card. The card itself already shows the album art (handled
    /// in `render_card`). Mirrors `render_compact_detail` for movies.
    ///
    /// Takes `items`/`cursor` explicitly rather than reading `nav_stack`
    /// internally (#145) so it can render either the legacy drilled-in
    /// nav_stack level or the inline-album-detail cache (the currently
    /// highlighted album in the album-folder listing, fetched proactively
    /// via `fetch_album_tracks`) with the same code path.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::app::render) fn render_album_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        items: &[mbv_core::api::EmbyItem],
        cursor: usize,
        focused: bool,
        show_title: bool,
        selected_region_gutter: bool,
        flush_left: bool,
        show_hint: bool,
        art_reserved_w: u16,
        layout: &mut LayoutMain,
    ) {
        if area.height == 0 {
            return;
        }

        let n = items.len();
        if items.is_empty() {
            return;
        }
        let gutter_w = if selected_region_gutter { 2 } else { 1 };
        let max_y = area.y + area.height;
        let mut row = area.y;

        // — Album title (only when no separate row already shows it — the
        // drilled-in single-pane view has no Album(idx) row above this, unlike
        // the inline/grouped call site) —
        if show_title && row < max_y {
            let album_title = items[0].album.clone();
            let title = trunc_str(&album_title, (area.width as usize).saturating_sub(1));
            let title_style = if focused {
                Style::default()
                    .fg(palette::TEXT_FOCUS_ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT_FOCUS_ACCENT)
            };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!(" {title}"), title_style))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }

        // — Inline album actions / spacer row —
        if row < max_y && show_hint {
            if selected_region_gutter {
                let hint_w = (area.width as usize).saturating_sub(gutter_w);
                // `focused` is passed explicitly by the caller: wide passes
                // the component's track focus, narrow always passes `false`
                // (narrow never holds inline track focus). Once track-selection
                // mode is entered, swap the "show tracks" hint for the exit
                // hint.
                let trailing_hint = if focused {
                    "BACK: Exit"
                } else {
                    "ENTER: Show tracks"
                };
                let hint = trunc_str(
                    &format!("^P: Play | ^A: Enqueue | ^S: Shuffle | {trailing_hint}"),
                    hint_w,
                );
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        super::list_rows::selection_marker(
                            false,
                            super::list_rows::MarkerEdge::Left,
                        ),
                        Span::raw(" "),
                        Span::styled(hint.to_string(), Style::default().fg(palette::TEXT_MUTED)),
                    ])),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
                row += 1;
                if row + 1 < max_y {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            super::list_rows::selection_marker(
                                false,
                                super::list_rows::MarkerEdge::Left,
                            ),
                            Span::raw(" "),
                        ])),
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
            if !selected_region_gutter {
                let trailing_hint = if focused {
                    "BACK: Exit"
                } else {
                    "ENTER: Show tracks"
                };
                let hint_width = area
                    .width
                    .saturating_sub(art_reserved_w)
                    .saturating_sub(1)
                    .max(1) as usize;
                let hint_lines: Vec<Line> = wrap(
                    &format!("^P: Play | ^A: Enqueue | ^S: Shuffle | {trailing_hint}"),
                    hint_width,
                )
                .into_iter()
                .map(|line| {
                    Line::from(vec![
                        Span::raw(" "),
                        Span::styled(
                            line.into_owned(),
                            Style::default().fg(palette::TEXT_EMPHASIS),
                        ),
                    ])
                })
                .collect();
                f.render_widget(
                    Paragraph::new(hint_lines.clone()),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width.saturating_sub(art_reserved_w),
                        height: hint_lines.len() as u16,
                    },
                );
                row += hint_lines.len() as u16 + 1;
            }
        }

        // — Scrollable track list —
        let track_indent = if flush_left || selected_region_gutter {
            0
        } else {
            INLINE_ALBUM_TRACK_EXTRA_INDENT
                .saturating_sub(INLINE_ALBUM_TITLE_EXTRA_INDENT)
                .min(area.width)
        };
        let table_area = Rect {
            x: area.x + track_indent,
            y: row,
            width: area
                .width
                .saturating_sub(track_indent)
                .saturating_sub(art_reserved_w),
            height: max_y.saturating_sub(row),
        };
        if table_area.height == 0 {
            return;
        }

        let show_length = table_area.width > 40;
        let dur_col_w: usize = if show_length { 7 } else { 0 };
        let title_col_w = (table_area.width as usize)
            .saturating_sub(gutter_w + 2 + if show_length { dur_col_w + 1 } else { 0 });

        let max_track_num = items
            .iter()
            .map(|item| item.index_number.max(0) as usize)
            .max()
            .unwrap_or(n);
        let track_num_width = max_track_num.to_string().len();

        let rows: Vec<Row> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_cursor = i == cursor;
                let selected = is_cursor && focused;
                let row_style = if is_cursor && focused {
                    Style::default()
                        .fg(palette::TEXT_FOCUS_ACCENT)
                        .bg(palette::SURFACE_FOCUSED)
                } else if focused {
                    Style::default().fg(palette::TEXT_STRONG)
                } else {
                    Style::default().fg(palette::TEXT_SECONDARY)
                };
                let marker = super::list_rows::selection_marker(
                    selected,
                    super::list_rows::MarkerEdge::Left,
                );
                let text_fg = if selected {
                    palette::TEXT_FOCUS_ACCENT
                } else {
                    palette::TEXT_EMPHASIS
                };
                let track_num = if item.index_number > 0 {
                    format!("{:>width$}. ", item.index_number, width = track_num_width)
                } else {
                    format!("{:>width$}. ", i + 1, width = track_num_width)
                };
                let mut title_spans = vec![marker];
                if selected_region_gutter {
                    title_spans.push(Span::raw(" "));
                }
                let num_w = track_num.chars().count();
                let title_width = title_col_w.saturating_sub(num_w).max(1);
                let title = album_track_title(item);
                let title_lines = wrap(&title, title_width);
                let mut wrapped_title_lines = Vec::with_capacity(title_lines.len());
                for (line_idx, line) in title_lines.into_iter().enumerate() {
                    if line_idx == 0 {
                        let mut first_line = title_spans.clone();
                        first_line.push(Span::styled(
                            track_num.clone(),
                            Style::default().fg(text_fg),
                        ));
                        first_line.push(Span::styled(
                            line.into_owned(),
                            Style::default().fg(text_fg),
                        ));

                        wrapped_title_lines.push(Line::from(first_line));
                    } else {
                        wrapped_title_lines.push(Line::from(vec![
                            Span::raw(
                                " ".repeat(1 + if selected_region_gutter { 1 } else { 0 } + num_w),
                            ),
                            Span::styled(line.into_owned(), Style::default().fg(text_fg)),
                        ]));
                    }
                }
                let title_height = wrapped_title_lines.len() as u16;
                let title_cell = Cell::from(Text::from(wrapped_title_lines));
                let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
                let length = if len_secs > 0 {
                    fmt_duration_mmss(len_secs)
                } else {
                    "\u{2014}".to_string()
                };
                if show_length {
                    Row::new([
                        title_cell,
                        Cell::from(Line::from(length).alignment(Alignment::Right)).style(
                            Style::default().fg(if selected {
                                palette::TEXT_FOCUS_ACCENT
                            } else {
                                palette::TEXT_SECONDARY
                            }),
                        ),
                        Cell::from(""),
                    ])
                    .height(title_height)
                    .style(row_style)
                } else {
                    Row::new([title_cell, Cell::from(""), Cell::from("")])
                        .height(title_height)
                        .style(row_style)
                }
            })
            .collect();

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(
            rows,
            [
                Constraint::Min(10),
                Constraint::Length(if show_length { dur_col_w as u16 } else { 0 }),
                Constraint::Length(1),
            ],
        )
        .column_spacing(1)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, table_area, &mut state);

        layout
            .wide_music_track_hitmap
            .extend(items.iter().enumerate().skip(state.offset()).scan(
                table_area.y,
                |y, (index, item)| {
                    let title = album_track_title(item);
                    let height = wrap(&title, title_col_w.max(1)).len().max(1) as u16;
                    let rect = Rect {
                        x: table_area.x,
                        y: *y,
                        width: table_area.width,
                        height: height.min(table_area.bottom().saturating_sub(*y)),
                    };
                    *y = (*y).saturating_add(height);
                    (rect.height > 0).then_some((rect, index))
                },
            ));

        let visible_rows = table_area.height as usize;
        if !selected_region_gutter && n > visible_rows {
            let max_offset = n.saturating_sub(visible_rows);
            super::widgets::render_right_scrollbar(
                f,
                table_area,
                max_offset,
                state.offset(),
                palette::SCROLLBAR,
            );
        }
    }
}
