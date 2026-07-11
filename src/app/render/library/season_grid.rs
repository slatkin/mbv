use super::super::super::layout::LayoutLibrary;
use super::super::super::palette;
use super::super::super::ui_util::trunc_str;
use super::super::super::App;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

impl App {
    pub(super) fn render_season_grid(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        layout: &mut LayoutLibrary,
    ) {
        if lib_idx >= self.libs.len() {
            return;
        }
        const COLS: usize = 4;
        const TEXT_ROWS: u16 = 3;
        const H_GAP: u16 = 2;
        const V_GAP: u16 = 1;

        let (items, cursor) = {
            let lvl = match self.libs[lib_idx].nav_stack.last() {
                Some(l) => l,
                None => return,
            };
            (lvl.items.clone(), lvl.cursor)
        };
        let n = items.len();

        if n == 0 {
            f.render_widget(
                Paragraph::new("  (empty)").style(Style::default().fg(palette::MUTED)),
                area,
            );
            return;
        }

        let total_rows = n.div_ceil(COLS);
        let scrollbar_w: u16 = 1;

        let cell_w = area
            .width
            .saturating_sub(scrollbar_w + H_GAP * (COLS as u16 - 1))
            / COLS as u16;

        let img_h: u16 = self
            .image_picker
            .as_ref()
            .map(|p| {
                let fs = p.font_size();
                ((cell_w as f32 * fs.width as f32 * 1.5) / fs.height as f32).floor() as u16
            })
            .unwrap_or(8)
            .min(8);

        let cell_h = img_h + TEXT_ROWS;
        let cell_step_h = cell_h + V_GAP;
        let n_visible_rows = 2;

        let cursor_row = cursor / COLS;
        let scroll_row = {
            let prev = layout.lib_scroll.get(lib_idx).copied().unwrap_or(0);
            let s = prev
                .min(cursor_row)
                .max(cursor_row.saturating_sub(n_visible_rows - 1));
            if let Some(v) = layout.lib_scroll.get_mut(lib_idx) {
                *v = s;
            }
            s
        };

        let images_enabled = self.images_enabled();

        if images_enabled {
            let first = scroll_row * COLS;
            let last = ((scroll_row + n_visible_rows) * COLS).min(n);
            let ids: Vec<String> = items[first..last].iter().map(|i| i.id.clone()).collect();
            for id in ids {
                let key = format!("{}:lib", id);
                self.fetch_list_card_image_when_idle(key, id, String::new(), &["Primary"]);
            }
        }

        let total_grid_w = COLS as u16 * cell_w + (COLS as u16 - 1) * H_GAP;
        let x_off = area.x + area.width.saturating_sub(scrollbar_w + total_grid_w) / 2;

        let total_grid_h = n_visible_rows as u16 * cell_step_h;
        let y_off = area.y + area.height.saturating_sub(total_grid_h) / 2;

        for row in 0..n_visible_rows {
            let abs_row = scroll_row + row;
            if abs_row >= total_rows {
                break;
            }
            let row_y = y_off + row as u16 * cell_step_h;
            if row_y >= area.y + area.height {
                break;
            }

            for col in 0..COLS {
                let idx = abs_row * COLS + col;
                if idx >= n {
                    break;
                }
                let item = &items[idx];
                let selected = idx == cursor;
                let cell_x = x_off + col as u16 * (cell_w + H_GAP);

                if images_enabled {
                    let key = format!("{}:lib", item.id);
                    let avail = ratatui::layout::Size {
                        width: cell_w,
                        height: img_h,
                    };
                    let actual = self
                        .card_image_states
                        .get_mut(&key)
                        .and_then(|s| s.as_mut())
                        .map(|s| {
                            s.size_for(
                                ratatui_image::Resize::Fit(Some(
                                    ratatui_image::FilterType::Lanczos3,
                                )),
                                avail,
                            )
                        });
                    if let Some(actual) = actual {
                        let ix = cell_x + (cell_w.saturating_sub(actual.width)) / 2;
                        let iy = row_y + (img_h.saturating_sub(actual.height)) / 2;
                        let img_rect = Rect {
                            x: ix,
                            y: iy,
                            width: actual.width,
                            height: actual.height.min((area.y + area.height).saturating_sub(iy)),
                        };
                        if let Some(Some(state)) = self.card_image_states.get_mut(&key) {
                            type SImg = ratatui_image::StatefulImage<
                                ratatui_image::protocol::StatefulProtocol,
                            >;
                            f.render_stateful_widget(
                                SImg::default().resize(ratatui_image::Resize::Fit(Some(
                                    ratatui_image::FilterType::Lanczos3,
                                ))),
                                img_rect,
                                state,
                            );
                        }
                    }
                }

                let name_y = row_y + img_h + 1;
                if name_y < area.y + area.height {
                    let style = if selected {
                        Style::default()
                            .fg(palette::IRIS)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette::TEXT)
                    };
                    f.render_widget(
                        Paragraph::new(ratatui::text::Span::styled(
                            trunc_str(&item.name, cell_w as usize),
                            style,
                        ))
                        .alignment(Alignment::Center),
                        Rect {
                            x: cell_x,
                            y: name_y,
                            width: cell_w,
                            height: 1,
                        },
                    );
                }

                let meta_y = row_y + img_h + 2;
                if meta_y < area.y + area.height {
                    let mut parts: Vec<String> = Vec::new();
                    if item.total_count > 0 {
                        parts.push(format!("{} eps", item.total_count));
                    }
                    if item.production_year > 0 {
                        parts.push(format!("{}", item.production_year));
                    }
                    let meta = trunc_str(&parts.join("  "), cell_w as usize);
                    f.render_widget(
                        Paragraph::new(ratatui::text::Span::styled(
                            meta,
                            Style::default().fg(palette::SUBTLE),
                        ))
                        .alignment(Alignment::Center),
                        Rect {
                            x: cell_x,
                            y: meta_y,
                            width: cell_w,
                            height: 1,
                        },
                    );
                }
            }
        }

        if total_rows > n_visible_rows {
            let mut state = ScrollbarState::new(total_rows).position(scroll_row);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("▐")
                    .track_symbol(Some(" "))
                    .begin_symbol(None)
                    .end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                area,
                &mut state,
            );
        }
    }
}
