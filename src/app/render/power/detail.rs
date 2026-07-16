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

    pub(crate) fn render_power_compact_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        const IMG_COLS: u16 = 18;
        const IMG_ROWS: u16 = 12;

        let Some(item) = self.power_selected_movie_item(lib_idx) else {
            return;
        };
        if area.height == 0 || area.width < 3 {
            return;
        }

        layout.cursor_screen_y = Some(area.y);
        layout.detail_max_scroll = 0;
        layout.detail_page_h = 0;

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

        if row < max_y {
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
            let (tw, tw16) = text_dims(row);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    trunc_str(&meta, tw),
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

        let playback = self.effective_playback_state();
        let now_playing_id: Option<String> = if playback.active {
            self.playback_queue()
                .items
                .get(playback.active_idx)
                .map(|i| i.id.clone())
        } else {
            None
        };
        if now_playing_id.as_deref() == Some(item.id.as_str()) && row < max_y {
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

        // — Overview + Director (single scrollable block, #204) —
        // Both used to be truncated independently (overview pre-cut to 400
        // chars, then both hard-clipped to the banner's fixed row count).
        // Now the banner is the *only* movie-detail surface (the old
        // Alt+M full-screen view was removed), so it reuses that view's
        // scroll mechanism instead: the full overview and director text are
        // always reachable via `power_detail_scroll`, driven by
        // `Up`/`Down`/`PageUp`/`PageDown` in `input.rs` whenever this block
        // overflows (`layout.detail_max_scroll > 0`).
        if row < max_y && (!item.overview.is_empty() || !item.director.is_empty()) {
            let avail = max_y.saturating_sub(row) as usize;
            let ov_start_y = row;
            let shadow_lines = img_end_row.saturating_sub(ov_start_y) as usize;

            // When the user has scrolled, lines with index >= shadow_lines may appear
            // in the on-screen rows that still overlap the image. Wrap using
            // scroll + shadow_lines as the narrow boundary so every line that
            // could appear next to the image at the current scroll position is
            // wrapped at narrow width; `text_dims(row)` (keyed off the actual
            // on-screen row) picks the final width when rendering.
            let cur_scroll = self.libs[lib_idx].power_detail_scroll;
            let shadow_boundary = cur_scroll + shadow_lines;

            let cleaned_overview = clean_overview(&item.overview);
            let mut all_lines: Vec<String> = Vec::new();
            for paragraph in cleaned_overview.lines() {
                let paragraph = if paragraph.trim().is_empty() {
                    " "
                } else {
                    paragraph.trim()
                };
                let line_idx = all_lines.len();
                let wrap_w = if line_idx < shadow_boundary {
                    narrow_w.max(1)
                } else {
                    inner_w.max(1)
                };
                for wrapped in wrap(paragraph, wrap_w) {
                    all_lines.push(wrapped.into_owned());
                }
            }

            // Director flows after the overview: blank gap then the director
            // line (rendered specially below so its "Director: " label keeps
            // its own style, matching the banner's previous look).
            let director_line_idx: Option<usize> = if !item.director.is_empty() {
                if !all_lines.is_empty() {
                    all_lines.push(String::new());
                }
                let idx = all_lines.len();
                all_lines.push(String::new());
                Some(idx)
            } else {
                None
            };

            let total = all_lines.len();
            let max_scroll = total.saturating_sub(avail);
            let scroll = self.libs[lib_idx].power_detail_scroll.min(max_scroll);
            self.libs[lib_idx].power_detail_scroll = scroll;
            layout.detail_max_scroll = max_scroll;
            layout.detail_page_h = avail.max(1);

            if avail > 0 {
                for (disp_idx, line_text) in all_lines.iter().skip(scroll).take(avail).enumerate() {
                    if row >= max_y {
                        break;
                    }
                    let (tw, tw16) = text_dims(row);
                    let abs_line_idx = scroll + disp_idx;
                    if Some(abs_line_idx) == director_line_idx {
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

                // Scroll indicator (#204 review fix): the old truncate-with-
                // "…" behavior at least hinted more text existed; scrolling
                // needs an explicit visual cue instead, matching every other
                // scrollable power-view surface (album/episode/home/list/
                // queue all call this for their own overflowing content).
                if max_scroll > 0 {
                    let ov_area = Rect {
                        x: area.x,
                        y: ov_start_y,
                        width: area.width,
                        height: avail as u16,
                    };
                    super::render_power_scrollbar(f, ov_area, max_scroll, scroll);
                }
            }
        }

        if img_height > 0 {
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
        let backend = TestBackend::new(60, 16);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_compact_detail(f, Rect::new(0, 0, 60, 16), 0, true, layout);
        })
        .unwrap();
        buffer_to_string(&term)
    }

    // Alt+M's full-screen detail view (`render_power_detail`) was removed in
    // #204: the compact banner (`render_power_compact_detail`) is now the
    // single movie-detail surface, so this exercises that instead. The
    // "enter prompt" assertions predate both surfaces having ever shown one;
    // kept as a regression guard.
    #[test]
    fn compact_movie_detail_shows_director_without_enter_prompt() {
        let mut app = make_app_stub();

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let mut movie = make_item("Focused Movie", "Movie");
        movie.id = "movie-1".into();
        movie.overview = "A long-form overview for the compact movie detail banner.".into();
        movie.director = "Jane Director".into();

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
            power_detail_scroll: 0,

            album_track_focus: None,
        });

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

    #[test]
    fn compact_movie_detail_scrolls_to_reveal_full_overview_and_director() {
        let mut app = make_app_stub();

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let mut movie = make_item("Focused Movie", "Movie");
        movie.id = "movie-1".into();
        // Well over the old 400-char trunc_overview() cap, and long enough
        // to overflow the banner's fixed content-row budget.
        movie.overview = "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. "
            .repeat(12);
        movie.director = "Very Distinctive Unique Director Name".into();

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
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        let mut layout = LayoutPower::default();
        let top_out = render_power_compact_detail_to_string(&mut app, &mut layout);
        assert!(
            !top_out.contains("Very Distinctive Unique Director Name"),
            "director should not fit on screen before scrolling:\n{top_out}"
        );
        assert!(
            layout.detail_max_scroll > 0,
            "long overview should overflow the banner and report a scrollable range"
        );

        app.libs[0].power_detail_scroll = layout.detail_max_scroll;
        let mut layout = LayoutPower::default();
        let scrolled_out = render_power_compact_detail_to_string(&mut app, &mut layout);
        assert!(
            scrolled_out.contains("Very Distinctive Unique Director Name"),
            "expected full director text after scrolling to the end:\n{scrolled_out}"
        );
    }
}
