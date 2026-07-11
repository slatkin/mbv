use super::super::super::palette;
use super::super::super::ui_util::trunc_str;
use super::super::super::App;
use super::super::super::{SavePlaylistStage, PLAYLISTS_PANEL_W};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

impl App {
    pub(in crate::app::render) fn render_playlists_panel(&mut self, f: &mut Frame) {
        let (title, hint) = if self.playlists_open.is_some() {
            let name = self
                .playlists_open
                .as_ref()
                .map(|p| p.name.as_str())
                .unwrap_or("Playlist");
            (
                name.to_uppercase(),
                "[↵]play [←]back [Esc]close".to_string(),
            )
        } else {
            (
                "PLAYLISTS".to_string(),
                "[↵]play [→]browse [r]refresh [Esc]close".to_string(),
            )
        };

        let content = Self::render_panel_shell(f, f.area(), PLAYLISTS_PANEL_W, &title, &hint);
        let ix = content.x;
        let iw = content.width as usize;
        let list_h = content.height as usize;

        if self.playlists_open.is_some() {
            self.render_open_playlist_panel(f, content, ix, iw, list_h);
            return;
        }

        if self.playlists_loading && self.playlists.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " Loading…",
                    Style::default().fg(palette::SUBTLE),
                )),
                content,
            );
            return;
        }
        if self.playlists.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " No playlists found",
                    Style::default().fg(palette::SUBTLE),
                )),
                content,
            );
            return;
        }

        if self.playlists_cursor < self.playlists_scroll {
            self.playlists_scroll = self.playlists_cursor;
        } else if self.playlists_cursor >= self.playlists_scroll + list_h {
            self.playlists_scroll = self.playlists_cursor + 1 - list_h;
        }

        let loaded_id: Option<&str> = if let crate::config::QueueSource::Playlist {
            id: Some(ref id),
            ..
        } = self.queue_source
        {
            Some(id.as_str())
        } else {
            None
        };

        for (vi, pl) in self.playlists[self.playlists_scroll..].iter().enumerate() {
            if vi >= list_h {
                break;
            }
            let abs_idx = self.playlists_scroll + vi;
            let selected = abs_idx == self.playlists_cursor;
            let is_loaded = loaded_id.map(|id| id == pl.id.as_str()).unwrap_or(false);
            let fg = if selected {
                palette::IRIS
            } else if is_loaded {
                palette::FOAM
            } else {
                palette::TEXT
            };
            let count_str = if pl.total_count > 0 {
                format!(" ({})", pl.total_count)
            } else {
                String::new()
            };
            let name_max =
                Self::panel_row_text_width(content.width).saturating_sub(count_str.len());
            let row_y = content.y + vi as u16;
            Self::render_panel_row(
                f,
                ix,
                row_y,
                content.width,
                selected,
                vec![
                    Span::styled(
                        trunc_str(&pl.name, name_max),
                        Style::default().fg(fg).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(count_str, Style::default().fg(palette::MUTED)),
                ],
            );
        }
        Self::render_sidebar_scrollbar(f, content, self.playlists.len(), self.playlists_scroll);
    }

    fn render_open_playlist_panel(
        &mut self,
        f: &mut Frame,
        content: Rect,
        ix: u16,
        iw: usize,
        list_h: usize,
    ) {
        if self.playlists_open_loading && self.playlists_open_items.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " Loading…",
                    Style::default().fg(palette::SUBTLE),
                )),
                content,
            );
            return;
        }
        if self.playlists_open_items.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " Playlist is empty",
                    Style::default().fg(palette::SUBTLE),
                )),
                content,
            );
            return;
        }

        // Clamp against the current item count: a background reload (e.g.
        // LibEvent::PlaylistItemsLoaded) can replace playlists_open_items with a
        // shorter list while the cursor/scroll are still positioned in the old,
        // longer one, which would otherwise panic the slices below.
        let max_idx = self.playlists_open_items.len() - 1;
        self.playlists_open_cursor = self.playlists_open_cursor.min(max_idx);
        self.playlists_open_scroll = self.playlists_open_scroll.min(max_idx);

        let item_lines = |label: &str| -> usize {
            let text_w = iw.saturating_sub(6);
            if label.len() <= text_w {
                1
            } else {
                2
            }
        };

        while self.playlists_open_scroll > self.playlists_open_cursor {
            self.playlists_open_scroll = self.playlists_open_cursor;
        }
        loop {
            if self.playlists_open_scroll >= self.playlists_open_cursor {
                break;
            }
            let lines_to_cursor: usize = self.playlists_open_items
                [self.playlists_open_scroll..=self.playlists_open_cursor]
                .iter()
                .map(|i| item_lines(&i.display_name()))
                .sum();
            if lines_to_cursor <= list_h {
                break;
            }
            self.playlists_open_scroll += 1;
        }

        let mut y = 0usize;
        for (vi, item) in self.playlists_open_items[self.playlists_open_scroll..]
            .iter()
            .enumerate()
        {
            if y >= list_h {
                break;
            }
            let abs_idx = self.playlists_open_scroll + vi;
            let selected = abs_idx == self.playlists_open_cursor;
            let fg = if selected {
                palette::IRIS
            } else {
                palette::TEXT
            };
            let num_str = format!("{:>2}. ", abs_idx + 1);
            let text_w = Self::panel_row_text_width(content.width).saturating_sub(num_str.len());
            let indent = " ".repeat(2 + num_str.len());
            let label = item.display_name();
            let (line1, line2) = if label.len() <= text_w {
                (label, String::new())
            } else {
                let wrap_at = label[..text_w].rfind(' ').unwrap_or(text_w);
                (
                    label[..wrap_at].to_string(),
                    label[wrap_at..].trim_start().to_string(),
                )
            };
            let row_y = content.y + y as u16;
            Self::render_panel_row(
                f,
                ix,
                row_y,
                content.width,
                selected,
                vec![
                    Span::styled(num_str, Style::default().fg(palette::MUTED)),
                    Span::styled(line1, Style::default().fg(fg)),
                ],
            );
            y += 1;
            if !line2.is_empty() && y < list_h {
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::raw(&indent),
                        Span::styled(
                            trunc_str(&line2, text_w),
                            Style::default().fg(palette::SUBTLE),
                        ),
                    ])),
                    Rect {
                        x: ix,
                        y: row_y + 1,
                        width: content.width,
                        height: 1,
                    },
                );
                y += 1;
            }
        }

        let total_lines: usize = self
            .playlists_open_items
            .iter()
            .map(|i| item_lines(&i.display_name()))
            .sum();
        let lines_before_scroll: usize = self.playlists_open_items[..self.playlists_open_scroll]
            .iter()
            .map(|i| item_lines(&i.display_name()))
            .sum();
        Self::render_sidebar_scrollbar(f, content, total_lines, lines_before_scroll);
    }

    pub(in crate::app::render) fn render_save_playlist_dialog(&mut self, f: &mut Frame) {
        let Some(ref dialog) = self.save_playlist_dialog else {
            return;
        };
        let full = f.area();
        let w: u16 = 52;
        let h: u16 = 7;
        let x = full.x + full.width.saturating_sub(w) / 2;
        let y = full.y + full.height.saturating_sub(h) / 2;
        let rect = Rect {
            x,
            y,
            width: w,
            height: h,
        };
        f.render_widget(Clear, rect);
        let block = Block::default()
            .title(Span::styled(
                " Save as Playlist ",
                Style::default()
                    .fg(palette::IRIS)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        match &dialog.stage {
            SavePlaylistStage::EnterName => {
                let label = "Name: ";
                let cursor = "▏";
                let max_input = inner.width as usize - label.len() - cursor.len() - 2;
                let visible: String = dialog
                    .input
                    .chars()
                    .rev()
                    .take(max_input)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                let input_line = format!("{}{}{}", label, visible, cursor);
                let hint = "Enter to save · Esc to cancel";
                let input_y = inner.y + (inner.height.saturating_sub(3)) / 2;
                let hint_y = input_y + 2;
                f.render_widget(
                    Paragraph::new(Span::styled(
                        input_line,
                        Style::default().fg(palette::WHITE),
                    )),
                    Rect {
                        x: inner.x + 1,
                        y: input_y,
                        width: inner.width.saturating_sub(2),
                        height: 1,
                    },
                );
                f.render_widget(
                    Paragraph::new(Span::styled(hint, Style::default().fg(palette::SUBTLE))),
                    Rect {
                        x: inner.x + 1,
                        y: hint_y,
                        width: inner.width.saturating_sub(2),
                        height: 1,
                    },
                );
            }
            SavePlaylistStage::ConfirmOverwrite { .. } => {
                let name = trunc_str(&dialog.input, inner.width as usize - 4);
                let line1 = format!("\"{}\" already exists.", name);
                let line2 = "Press y to overwrite · Esc to go back";
                let base_y = inner.y + (inner.height.saturating_sub(3)) / 2;
                f.render_widget(
                    Paragraph::new(Span::styled(line1, Style::default().fg(palette::WHITE))),
                    Rect {
                        x: inner.x + 1,
                        y: base_y,
                        width: inner.width.saturating_sub(2),
                        height: 1,
                    },
                );
                f.render_widget(
                    Paragraph::new(Span::styled(line2, Style::default().fg(palette::SUBTLE))),
                    Rect {
                        x: inner.x + 1,
                        y: base_y + 2,
                        width: inner.width.saturating_sub(2),
                        height: 1,
                    },
                );
            }
        }
    }

    pub(in crate::app::render) fn render_dirty_playlist_modal(&self, f: &mut Frame) {
        let name = trunc_str(self.queue_playlist_name(), 36);
        let full = f.area();
        let w: u16 = 56;
        let h: u16 = 7;
        let x = full.x + full.width.saturating_sub(w) / 2;
        let y = full.y + full.height.saturating_sub(h) / 2;
        let rect = Rect {
            x,
            y,
            width: w,
            height: h,
        };
        f.render_widget(Clear, rect);
        let block = Block::default()
            .title(Span::styled(
                " Unsaved Playlist Changes ",
                Style::default()
                    .fg(palette::YELLOW)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::YELLOW));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let line1 = format!("Save changes to \"{}\"?", name);
        let line2 = "[s]Save  [d]Discard  [Esc]Cancel";
        let base_y = inner.y + (inner.height.saturating_sub(3)) / 2;
        f.render_widget(
            Paragraph::new(Span::styled(line1, Style::default().fg(palette::WHITE))),
            Rect {
                x: inner.x + 1,
                y: base_y,
                width: inner.width.saturating_sub(2),
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(Span::styled(line2, Style::default().fg(palette::SUBTLE))),
            Rect {
                x: inner.x + 1,
                y: base_y + 2,
                width: inner.width.saturating_sub(2),
                height: 1,
            },
        );
    }
}
