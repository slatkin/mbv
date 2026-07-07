use super::super::super::ui_util::*;
use crate::app::layout::LayoutPower;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
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
            self.fetch_card_image(
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

        // — Play status block: blank / status / blank —
        {
            let playback = self.effective_playback_state();
            let now_playing_id: Option<String> = if playback.active {
                self.playback_queue()
                    .items
                    .get(playback.active_idx)
                    .map(|i| i.id.clone())
            } else {
                None
            };
            let is_playing = now_playing_id.as_deref() == Some(item.id.as_str());

            // blank row above
            if row < max_y {
                row += 1;
            }

            // status row
            if row < max_y {
                let (_tw, tw16) = text_dims(row);
                if is_playing {
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
                } else {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::styled("Press ", Style::default().fg(palette::SUBTLE)),
                            Span::styled(
                                "[ENTER]",
                                Style::default()
                                    .fg(palette::IRIS)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(" to play", Style::default().fg(palette::SUBTLE)),
                        ])),
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

            // blank row below
            if row < max_y {
                row += 1;
            }
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
