use super::super::super::ui_util::*;
use super::POWER_RENDER_FILTER;
use crate::app::layout::LayoutPower;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;

const IMG_COLS: u16 = 18;
const IMG_ROWS: u16 = 12;

/// Everything content-dependent about the compact movie-detail banner: the
/// meta line, the "Playing" indicator, and the overview + director text
/// wrapped to the banner's actual panel width. Computed once by
/// `App::compact_banner_layout` and consumed both to size the banner's row
/// budget in the list layout (`list::compact_banner_rows`, run *before* the
/// rest of the list's rows are positioned) and to actually render the
/// banner (`render_power_compact_detail`) -- the two-pass split this issue
/// (#263) introduces, kept in lockstep by sharing this one computation
/// instead of the row count and the render duplicating the wrapping logic.
pub(super) struct CompactBannerLayout {
    meta_line: Option<String>,
    show_playing: bool,
    /// Wrapped overview lines, plus (if there's a director) a blank
    /// separator line and a placeholder line at `director_line_idx` that
    /// renders as "Director: <name>" instead of plain text.
    lines: Vec<String>,
    director_line_idx: Option<usize>,
    img_actual_w: u16,
    img_height: u16,
}

impl CompactBannerLayout {
    /// Total rows the banner's content needs: meta line + "Playing" line (if
    /// present) + every wrapped overview/director line. No cap is applied --
    /// real Emby movie metadata is short by convention (#263), so unbounded
    /// growth here is intended, not a bug to guard against.
    pub(super) fn content_rows(&self) -> usize {
        self.meta_line.is_some() as usize + self.show_playing as usize + self.lines.len()
    }
}

impl App {
    pub(crate) fn power_selected_movie_item(
        &self,
        lib_idx: usize,
    ) -> Option<mbv_core::api::MediaItem> {
        let lib = self.libs.get(lib_idx)?;
        if lib.library.collection_type != "movies" {
            return None;
        }

        let item = if let Some(search) = &lib.search {
            let &idx = search.results.get(search.cursor)?;
            search.items.get(idx)?.clone()
        } else {
            let level = lib.nav_stack.last()?;
            level.items.get(level.cursor)?.clone()
        };

        if item.is_folder || item.item_type != "Movie" {
            None
        } else {
            Some(item)
        }
    }

    /// Computes the compact banner's content for `item`, given the panel
    /// width it will render into (i.e. the eventual `area.width` passed to
    /// `render_power_compact_detail`). Pure function of `item` + width aside
    /// from the image-state cache lookup/fetch-trigger, so calling it twice
    /// per frame (once to measure, once to render) is safe and idempotent.
    pub(super) fn compact_banner_layout(
        &mut self,
        item: &mbv_core::api::MediaItem,
        panel_width: u16,
    ) -> CompactBannerLayout {
        let inner_w = (panel_width as usize).saturating_sub(2);

        let primary_cache_key = format!("{}:cmp_primary", item.id);
        if self.images_enabled() {
            self.fetch_card_image(
                primary_cache_key.clone(),
                item.id.clone(),
                item.series_id.clone(),
                &["Primary"],
            );
        }

        let (img_actual_w, img_height): (u16, u16) = {
            if self.list_image_renders_allowed() {
                if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                    // `size_for` is `None` while resize+encode is in-flight on
                    // the worker thread; treat that the same as no image yet.
                    let actual = state
                        .size_for(
                            ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER)),
                            ratatui::layout::Size {
                                width: IMG_COLS,
                                height: IMG_ROWS,
                            },
                        )
                        .unwrap_or_default();
                    (actual.width, actual.height)
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            }
        };

        let narrow_w = inner_w.saturating_sub(img_actual_w as usize);

        let mut rows_before_overview = 0usize;

        let dur_str = if item.runtime_ticks > 0 {
            fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
        } else {
            String::new()
        };
        let year_str = if item.production_year > 0 {
            item.production_year.to_string()
        } else {
            String::new()
        };
        let meta = [item.genre.as_str(), year_str.as_str(), dur_str.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join("  ");
        let meta_line = if meta.is_empty() {
            None
        } else {
            rows_before_overview += 1;
            Some(meta)
        };

        let playback = self.effective_playback_state();
        let now_playing_id: Option<String> = if playback.active {
            self.playback_queue()
                .items
                .get(playback.active_idx)
                .map(|i| i.id.clone())
        } else {
            None
        };
        let show_playing = now_playing_id.as_deref() == Some(item.id.as_str());
        if show_playing {
            rows_before_overview += 1;
        }

        // Rows before the overview block sit above the poster image's
        // bottom edge too (as long as there aren't more of them than the
        // image is tall), so they narrow the wrap width the same way
        // overview/director lines do; `shadow_lines` counts how many of the
        // *upcoming* overview/director lines still fall within the image's
        // row span.
        let shadow_lines = (img_height as usize).saturating_sub(rows_before_overview);

        let mut lines: Vec<String> = Vec::new();
        let mut director_line_idx: Option<usize> = None;
        if !item.overview.is_empty() || !item.director.is_empty() {
            let cleaned_overview = clean_overview(&item.overview);
            for paragraph in cleaned_overview.lines() {
                let paragraph = if paragraph.trim().is_empty() {
                    " "
                } else {
                    paragraph.trim()
                };
                let line_idx = lines.len();
                let wrap_w = if line_idx < shadow_lines {
                    narrow_w.max(1)
                } else {
                    inner_w.max(1)
                };
                for wrapped in wrap(paragraph, wrap_w) {
                    lines.push(wrapped.into_owned());
                }
            }

            // Director flows after the overview: blank gap then the director
            // line (rendered specially so its "Director: " label keeps its
            // own style, matching the banner's previous look).
            if !item.director.is_empty() {
                if !lines.is_empty() {
                    lines.push(String::new());
                }
                director_line_idx = Some(lines.len());
                lines.push(String::new());
            }
        }

        CompactBannerLayout {
            meta_line,
            show_playing,
            lines,
            director_line_idx,
            img_actual_w,
            img_height,
        }
    }

    pub(crate) fn render_power_compact_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        let Some(item) = self.power_selected_movie_item(lib_idx) else {
            return;
        };
        if area.height == 0 || area.width < 3 {
            return;
        }

        layout.cursor_screen_y = Some(area.y);

        let content = self.compact_banner_layout(&item, area.width);

        let inner_x = area.x + 1;
        let inner_w = (area.width as usize).saturating_sub(2);
        let inner_w16 = area.width.saturating_sub(2);
        let mut row = area.y;
        let max_y = area.y + area.height;

        let text_color = if focused {
            palette::WHITE
        } else {
            palette::SUBTLE
        };

        let img_actual_w = content.img_actual_w;
        let img_height = content.img_height;
        let img_x = area.x + area.width.saturating_sub(img_actual_w);
        // No title row is drawn here anymore (it duplicated the selected list
        // row's title, already shown in green just above the banner), so the
        // poster starts flush with the banner's own top row instead of being
        // pushed down a row to make room for a redundant title.
        let img_y = area.y.min(area.y + area.height.saturating_sub(1));
        let img_end_row = img_y + img_height;
        layout.inline_image_rect = if img_height > 0 {
            Some(Rect {
                x: img_x,
                y: img_y,
                width: img_actual_w,
                height: img_height,
            })
        } else {
            None
        };

        let narrow_w = inner_w.saturating_sub(img_actual_w as usize);
        let narrow_w16 = inner_w16.saturating_sub(img_actual_w);
        let text_dims = |r: u16| -> (usize, u16) {
            if img_height > 0 && r < img_end_row {
                (narrow_w, narrow_w16)
            } else {
                (inner_w, inner_w16)
            }
        };

        if let Some(meta) = &content.meta_line {
            if row < max_y {
                let (tw, tw16) = text_dims(row);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        trunc_str(meta, tw),
                        Style::default().fg(palette::SUBTLE),
                    ))),
                    Rect {
                        x: inner_x,
                        y: row,
                        width: tw16,
                        height: 1,
                    },
                );
                row += 1;
            }
        }

        if content.show_playing && row < max_y {
            let (_tw, tw16) = text_dims(row);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "Playing",
                    Style::default()
                        .fg(palette::FOAM)
                        .add_modifier(Modifier::BOLD),
                ))),
                Rect {
                    x: inner_x,
                    y: row,
                    width: tw16,
                    height: 1,
                },
            );
            row += 1;
        }

        // — Overview + Director (#204, #263) —
        // The banner grows to fit this block's full wrapped height (computed
        // by `compact_banner_layout` and consumed by the list layout before
        // any of this renders, so `area` is already sized to fit every
        // line) instead of clipping or scrolling it.
        for (idx, line_text) in content.lines.iter().enumerate() {
            if row >= max_y {
                break;
            }
            let (tw, tw16) = text_dims(row);
            if Some(idx) == content.director_line_idx {
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("Director: ", Style::default().fg(palette::SUBTLE)),
                        Span::styled(
                            trunc_str(&item.director, tw),
                            Style::default().fg(palette::TEXT),
                        ),
                    ])),
                    Rect {
                        x: inner_x,
                        y: row,
                        width: tw16,
                        height: 1,
                    },
                );
            } else if !line_text.is_empty() {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        trunc_str(line_text, tw),
                        Style::default().fg(text_color),
                    ))),
                    Rect {
                        x: inner_x,
                        y: row,
                        width: tw16,
                        height: 1,
                    },
                );
            }
            row += 1;
        }

        if img_height > 0 {
            let primary_cache_key = format!("{}:cmp_primary", item.id);
            if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER))),
                    Rect {
                        x: img_x,
                        y: img_y,
                        width: img_actual_w,
                        height: img_height,
                    },
                    state,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::{BrowseLevel, LibraryTab};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let area = *buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn render_power_compact_detail_to_string(app: &mut App, layout: &mut LayoutPower) -> String {
        render_power_compact_detail_to_string_sized(app, layout, 60, 16)
    }

    fn render_power_compact_detail_to_string_sized(
        app: &mut App,
        layout: &mut LayoutPower,
        width: u16,
        height: u16,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_compact_detail(f, Rect::new(0, 0, width, height), 0, true, layout);
        })
        .unwrap();
        buffer_to_string(&term)
    }

    fn push_movie_lib(app: &mut App, movie: mbv_core::api::MediaItem) {
        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![movie],
                total_count: 1,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
        });
    }

    // Alt+M's full-screen detail view (`render_power_detail`) was removed in
    // #204: the compact banner (`render_power_compact_detail`) is now the
    // single movie-detail surface, so this exercises that instead. The
    // "enter prompt" assertions predate both surfaces having ever shown one;
    // kept as a regression guard.
    #[test]
    fn compact_movie_detail_shows_director_without_enter_prompt() {
        let mut app = make_app_stub();

        let mut movie = make_item("Focused Movie", "Movie");
        movie.id = "movie-1".into();
        movie.overview = "A long-form overview for the compact movie detail banner.".into();
        movie.director = "Jane Director".into();
        push_movie_lib(&mut app, movie);

        let mut layout = LayoutPower::default();
        let out = render_power_compact_detail_to_string(&mut app, &mut layout);

        assert!(
            out.contains("Director: Jane Director"),
            "expected director:\n{out}"
        );
        assert!(
            !out.contains("Press"),
            "enter prompt should be removed:\n{out}"
        );
        assert!(
            !out.contains("[ENTER]"),
            "enter prompt should be removed:\n{out}"
        );
    }

    // #263: a short overview must render fully with no scrollbar, using only
    // as many rows as its wrapped content actually needs.
    #[test]
    fn compact_movie_detail_shows_full_short_overview_with_no_scrollbar() {
        let mut app = make_app_stub();

        let mut movie = make_item("Focused Movie", "Movie");
        movie.id = "movie-1".into();
        movie.overview = "A short overview.".into();
        movie.director = "Jane Director".into();
        push_movie_lib(&mut app, movie);

        let mut layout = LayoutPower::default();
        let out = render_power_compact_detail_to_string(&mut app, &mut layout);

        assert!(
            out.contains("A short overview."),
            "expected full overview text:\n{out}"
        );
        assert!(
            out.contains("Director: Jane Director"),
            "expected director:\n{out}"
        );
        assert!(
            !out.contains('\u{2590}'),
            "no banner scrollbar should be drawn:\n{out}"
        );
    }

    // #263: a long overview (well past what any fixed-height budget could
    // show) must still render its full text and full director in one pass,
    // with no scrollbar and no truncation, given a tall enough panel.
    #[test]
    fn compact_movie_detail_shows_full_long_overview_with_no_scrollbar() {
        let mut app = make_app_stub();

        let mut movie = make_item("Focused Movie", "Movie");
        movie.id = "movie-1".into();
        movie.overview = "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. "
            .repeat(12);
        movie.director = "Very Distinctive Unique Director Name".into();
        push_movie_lib(&mut app, movie);

        let mut layout = LayoutPower::default();
        // Tall enough that the whole grown banner fits in the test buffer.
        let out = render_power_compact_detail_to_string_sized(&mut app, &mut layout, 60, 80);

        assert!(
            out.contains("Very Distinctive Unique Director Name"),
            "expected full director text with no scrolling:\n{out}"
        );
        assert!(
            !out.contains('\u{2590}'),
            "no banner scrollbar should be drawn:\n{out}"
        );
    }
}
