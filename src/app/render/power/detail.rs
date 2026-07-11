use super::super::super::ui_util::*;
use crate::app::layout::LayoutPower;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

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
            self.fetch_list_card_image_when_idle(
                primary_cache_key.clone(),
                item.id.clone(),
                item.series_id.clone(),
                &["Primary"],
            );
        }

        let (img_actual_w, img_height): (u16, u16) = {
            if self.list_image_renders_allowed() {
                if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                    let actual = state.size_for(
                        ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)),
                        ratatui::layout::Size {
                            width: IMG_COLS,
                            height: IMG_ROWS,
                        },
                    );
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

        if row < max_y && !item.overview.is_empty() {
            let overview = trunc_overview(&item.overview);
            let avail = max_y.saturating_sub(row) as usize;
            let shadow_lines = img_end_row.saturating_sub(row) as usize;
            let mut all_lines: Vec<String> = Vec::new();
            let overview_start = row;

            for paragraph in overview.lines() {
                let paragraph = if paragraph.trim().is_empty() {
                    " "
                } else {
                    paragraph.trim()
                };
                let line_idx = all_lines.len();
                let wrap_w = if line_idx < shadow_lines {
                    narrow_w.max(1)
                } else {
                    inner_w.max(1)
                };
                for wrapped in wrap(paragraph, wrap_w) {
                    all_lines.push(wrapped.into_owned());
                }
            }

            if avail > 0 {
                let clipped = all_lines.len() > avail;
                for (disp_idx, line_text) in all_lines.iter().take(avail).enumerate() {
                    let mut line = line_text.clone();
                    let (tw, tw16) = text_dims(row);
                    if clipped && disp_idx + 1 == avail {
                        let base = trunc_str(&line, tw.saturating_sub(1)).to_string();
                        line = format!("{base}\u{2026}");
                    }
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            trunc_str(&line, tw),
                            Style::default().fg(text_color),
                        ))),
                        Rect {
                            x: inner_x,
                            y: row,
                            width: tw16,
                            height: 1,
                        },
                    );
                    row += 1;
                    if row >= max_y {
                        break;
                    }
                }
            }
            if row < max_y && row > overview_start && !item.director.is_empty() {
                row += 1;
            }
        }

        if row < max_y && !item.director.is_empty() {
            let (tw, tw16) = text_dims(row);
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
        }

        if img_height > 0 {
            if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                type SImg = ratatui_image::StatefulImage<ratatui_image::protocol::StatefulProtocol>;
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(
                        ratatui_image::FilterType::Lanczos3,
                    ))),
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

    /// Renders the movie detail panel (title, metadata, overview, director) into `area`.
    /// Called instead of `render_power_list` when `power_detail_item` is Some.
    pub(super) fn render_power_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        // Clone so self is free for scroll-state writes below.
        let item = match self.libs[lib_idx].power_detail_item.clone() {
            Some(it) => it,
            None => return,
        };
        if area.height == 0 {
            return;
        }

        layout.cursor_screen_y = Some(area.y);

        let inner_x = area.x + 1;
        let inner_w = (area.width as usize).saturating_sub(2);
        let inner_w16 = area.width.saturating_sub(2);
        let max_y = area.y + area.height;
        let mut row = area.y;

        let title_color = if focused {
            palette::YELLOW
        } else {
            palette::SUBTLE
        };
        let text_color = if focused {
            palette::WHITE
        } else {
            palette::SUBTLE
        };

        // — Primary poster image (right-aligned in a bordered block, starts on second row) —
        const IMG_COLS: u16 = 28;
        const IMG_MAX_ROWS: u16 = 12;
        let img_start_row = area.y + 1; // row immediately after title

        // Fetch the Primary image using a key distinct from the backdrop key.
        let primary_cache_key = format!("{}:det_primary", item.id);
        if self.images_enabled() {
            self.fetch_list_card_image_when_idle(
                primary_cache_key.clone(),
                item.id.clone(),
                item.series_id.clone(),
                &["Primary"],
            );
        }

        // Pre-compute the *actual* rendered dimensions. size_for() respects aspect ratio so
        // the image may be narrower than IMG_COLS (e.g. a portrait poster). We need the real
        // width to position it flush-right and to compute the text shadow width.
        // The borrow on card_image_states ends at the closing } of this block.
        let (img_actual_w, img_height): (u16, u16) = {
            if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                let avail = ratatui::layout::Size {
                    width: IMG_COLS,
                    height: IMG_MAX_ROWS,
                };
                let actual = state.size_for(
                    ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)),
                    avail,
                );
                (actual.width, actual.height)
            } else {
                (0, 0)
            }
        };

        // Image is flush with the right edge; shadow extends 1 extra row below (bottom padding).
        let img_x = area.x + area.width.saturating_sub(img_actual_w);
        // img_end_row is exclusive: image rows + 1 blank padding row below.
        let img_end_row = img_start_row + img_height + 1;
        layout.inline_image_rect = if img_height > 0 {
            Some(Rect {
                x: img_x,
                y: img_start_row,
                width: img_actual_w,
                height: img_height + 1,
            })
        } else {
            None
        };

        // Narrow text width: leave 1-col gap to the left of the image.
        // img_x = area.x + area.width - img_actual_w; text spans [inner_x, inner_x + narrow_w16).
        // Last text col = inner_x + narrow_w16 - 1; gap col = img_x - 1; so narrow_w16 = img_x - inner_x - 1.
        // = (area.width - img_actual_w) - 1 - 1 = inner_w16 - img_actual_w - 1 + 1 ... simplify:
        // narrow_w16 = area.width - img_actual_w - 2 = inner_w16 - img_actual_w
        let narrow_w = inner_w.saturating_sub(img_actual_w as usize);
        let narrow_w16 = inner_w16.saturating_sub(img_actual_w);

        // Return the appropriate (char_width, u16_width) for a given absolute row.
        let text_dims = |r: u16| -> (usize, u16) {
            if img_height > 0 && r >= img_start_row && r < img_end_row {
                (narrow_w, narrow_w16)
            } else {
                (inner_w, inner_w16)
            }
        };

        // — Title (row 0 — full width, image hasn't started yet) —
        if row < max_y {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    trunc_str(&item.name, inner_w),
                    Style::default().fg(title_color),
                ))),
                Rect {
                    x: inner_x,
                    y: row,
                    width: inner_w16,
                    height: 1,
                },
            );
            row += 1;
        }

        // — Meta: genre  year  duration (SUBTLE) —
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

        // — Technical: video_info then audio_info on separate rows (MUTED) —
        for tech_str in [item.video_info.as_str(), item.audio_info.as_str()] {
            if row < max_y && !tech_str.is_empty() {
                let (tw, tw16) = text_dims(row);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        trunc_str(tech_str, tw),
                        Style::default().fg(palette::MUTED),
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

        // — Overview + Director (single scrollable block) —
        // Director flows naturally after the description with a blank separator;
        // nothing is pinned to the bottom.
        if (!item.overview.is_empty() || !item.director.is_empty()) && row < max_y {
            let avail = max_y.saturating_sub(row) as usize;
            let ov_start_y = row;

            // How many display rows at the top of this block still overlap with the image?
            let shadow_lines = img_end_row.saturating_sub(ov_start_y) as usize;

            // When the user has scrolled, lines with abs_line_idx >= shadow_lines may appear
            // in the on-screen rows that still overlap the image (disp_idx < shadow_lines).
            // Wrap using scroll + shadow_lines as the narrow boundary so that every line
            // that will appear next to the image on screen is wrapped at narrow width.
            let cur_scroll = self.libs[lib_idx].power_detail_scroll;
            let shadow_boundary = cur_scroll + shadow_lines;

            // Word-wrap the overview, switching from narrow to full width at the shadow boundary.
            let mut all_lines: Vec<String> = Vec::new();
            let mut cur = String::new();
            for word in item.overview.split_whitespace() {
                let line_idx = all_lines.len();
                let wrap_w = if line_idx < shadow_boundary {
                    narrow_w
                } else {
                    inner_w
                };
                let word_w = word.width();
                if cur.is_empty() {
                    cur.push_str(word);
                } else if cur.width() + 1 + word_w <= wrap_w {
                    cur.push(' ');
                    cur.push_str(word);
                } else {
                    all_lines.push(std::mem::take(&mut cur));
                    cur.push_str(word);
                }
            }
            if !cur.is_empty() {
                all_lines.push(cur);
            }

            // Director flows after the overview: blank gap then the director line.
            let director_line_idx: Option<usize> = if !item.director.is_empty() {
                all_lines.push(String::new()); // blank separator
                let idx = all_lines.len();
                all_lines.push(format!("Director: {}", item.director));
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
                    // Use disp_idx (on-screen position) to pick width: the first shadow_lines
                    // rows are next to the image regardless of scroll position.
                    let tw16 = if disp_idx < shadow_lines {
                        narrow_w16
                    } else {
                        inner_w16
                    };
                    let abs_line_idx = scroll + disp_idx;
                    let fg = if Some(abs_line_idx) == director_line_idx {
                        palette::MUTED
                    } else {
                        text_color
                    };
                    if !line_text.is_empty() {
                        f.render_widget(
                            Paragraph::new(Line::from(Span::styled(
                                line_text.clone(),
                                Style::default().fg(fg),
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

                if max_scroll > 0 {
                    let ov_area = Rect {
                        x: area.x,
                        y: ov_start_y,
                        width: area.width,
                        height: avail as u16,
                    };
                    let mut sb = ScrollbarState::new(max_scroll + 1).position(scroll);
                    f.render_stateful_widget(
                        Scrollbar::new(ScrollbarOrientation::VerticalRight)
                            .thumb_symbol("\u{2590}")
                            .track_symbol(Some(" "))
                            .begin_symbol(None)
                            .end_symbol(None)
                            .style(Style::default().fg(palette::SUBTLE)),
                        ov_area,
                        &mut sb,
                    );
                }
            }
        }

        // — Render Primary image last so it layers over text cleanly —
        // No border drawn; the 1-col left gap and 1-row bottom gap are handled via shadow math.
        if img_height > 0 {
            if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                type SImg = ratatui_image::StatefulImage<ratatui_image::protocol::StatefulProtocol>;
                let img_rect = Rect {
                    x: img_x,
                    y: img_start_row,
                    width: img_actual_w,
                    height: img_height,
                };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(
                        ratatui_image::FilterType::Lanczos3,
                    ))),
                    img_rect,
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

    fn render_power_detail_to_string(app: &mut App, layout: &mut LayoutPower) -> String {
        let backend = TestBackend::new(60, 16);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_detail(f, Rect::new(0, 0, 60, 16), 0, true, layout);
        })
        .unwrap();
        buffer_to_string(&term)
    }

    #[test]
    fn expanded_movie_detail_shows_director_without_enter_prompt() {
        let mut app = make_app_stub();

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let mut movie = make_item("Focused Movie", "Movie");
        movie.id = "movie-1".into();
        movie.overview = "A long-form overview for the expanded movie detail panel.".into();
        movie.director = "Jane Director".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![movie.clone()],
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
            power_detail_item: Some(movie),
            power_detail_scroll: 0,
        });

        let mut layout = LayoutPower::default();
        let out = render_power_detail_to_string(&mut app, &mut layout);

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
}
