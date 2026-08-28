use super::super::super::palette;
use super::super::super::ui_util::trunc_str;
#[cfg(test)]
use super::super::super::App;
#[cfg(test)]
use super::super::super::PLAYLISTS_PANEL_W;
use super::chrome;
use crate::app::render::components::modal_frame::render_modal_frame;
use mbv_core::api::EmbyItem;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub(in crate::app) fn render_save_playlist_content(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    input: &str,
    rename: bool,
) {
    let title_text = if rename {
        " Rename Playlist "
    } else {
        " Save as Playlist "
    };
    let inner = render_modal_frame(
        f,
        dim_backdrop_active,
        title_text,
        52,
        7,
        palette::SURFACE_FOCUSED,
    );
    let label = "Name: ";
    let cursor = "▏";
    let max_input = inner.width as usize - label.len() - cursor.len() - 2;
    let visible: String = input
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
            Style::default().fg(palette::TEXT_STRONG),
        )),
        Rect {
            x: inner.x + 1,
            y: input_y,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            hint,
            Style::default().fg(palette::TEXT_SECONDARY),
        )),
        Rect {
            x: inner.x + 1,
            y: hint_y,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );
}

#[derive(Default)]
pub(in crate::app) struct PlaylistsRenderGeometry {
    pub panel_area: Rect,
    pub content_area: Rect,
    pub playlist_rows: Vec<(Rect, usize)>,
    pub open_rows: Vec<(Rect, usize)>,
}

impl PlaylistsRenderGeometry {
    pub(in crate::app) fn hit_test(
        &self,
        position: ratatui::layout::Position,
    ) -> Option<(bool, usize)> {
        self.open_rows
            .iter()
            .find(|(rect, _)| rect.contains(position))
            .map(|(_, index)| (true, *index))
            .or_else(|| {
                self.playlist_rows
                    .iter()
                    .find(|(rect, _)| rect.contains(position))
                    .map(|(_, index)| (false, *index))
            })
    }
}

pub(in crate::app) fn render_playlists_content(
    frame: &mut Frame,
    area: Rect,
    panel_area: Option<Rect>,
    playlists: &[EmbyItem],
    playlists_cursor: &mut usize,
    playlists_scroll: &mut usize,
    playlists_loading: bool,
    playlists_open: Option<&EmbyItem>,
    open_items: &[EmbyItem],
    open_cursor: &mut usize,
    open_scroll: &mut usize,
    open_loading: bool,
    loaded_id: Option<&str>,
    geometry: &mut PlaylistsRenderGeometry,
) {
    *geometry = PlaylistsRenderGeometry::default();
    let (title, hint) = if let Some(playlist) = playlists_open {
        (
            playlist.name.to_uppercase(),
            "[↵]play [←]back [Esc]close".to_string(),
        )
    } else {
        (
            "PLAYLISTS".to_string(),
            "[↵]play [→]browse [n]rename [d]delete [r]refresh [Esc]close".to_string(),
        )
    };
    let panel = panel_area.unwrap_or(area);
    geometry.panel_area = panel;
    let content = chrome::render_panel_shell_at(frame, panel, &title, &hint, true);
    geometry.content_area = content;
    if playlists_open.is_some() {
        render_open_playlist_content(
            frame,
            content,
            open_items,
            open_cursor,
            open_scroll,
            open_loading,
            geometry,
        );
        return;
    }
    if playlists_loading && playlists.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Loading…",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            content,
        );
        return;
    }
    if playlists.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " No playlists found",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            content,
        );
        return;
    }
    if *playlists_cursor < *playlists_scroll {
        *playlists_scroll = *playlists_cursor;
    } else if *playlists_cursor >= *playlists_scroll + content.height as usize {
        *playlists_scroll = (*playlists_cursor)
            .saturating_add(1)
            .saturating_sub(content.height as usize);
    }
    for (visible, playlist) in playlists[*playlists_scroll..].iter().enumerate() {
        if visible >= content.height as usize {
            break;
        }
        let index = *playlists_scroll + visible;
        let selected = index == *playlists_cursor;
        let loaded = loaded_id.is_some_and(|id| id == playlist.id);
        let fg = if selected {
            palette::ACCENT_ACTIVE
        } else if loaded {
            palette::TEXT_ACCENT_MUTED
        } else {
            palette::TEXT_PRIMARY
        };
        let count = if playlist.total_count > 0 {
            format!(" ({})", playlist.total_count)
        } else {
            String::new()
        };
        let row = Rect {
            x: content.x,
            y: content.y + visible as u16,
            width: content.width,
            height: 1,
        };
        chrome::render_panel_row(
            frame,
            content.x,
            row.y,
            content.width,
            selected,
            vec![
                Span::styled(
                    trunc_str(
                        &playlist.name,
                        chrome::panel_row_text_width(content.width).saturating_sub(count.len()),
                    ),
                    Style::default().fg(fg).add_modifier(Modifier::BOLD),
                ),
                Span::styled(count, Style::default().fg(palette::TEXT_MUTED)),
            ],
        );
        geometry.playlist_rows.push((row, index));
    }
    chrome::render_sidebar_scrollbar(frame, content, playlists.len(), *playlists_scroll);
}

fn render_open_playlist_content(
    frame: &mut Frame,
    content: Rect,
    items: &[EmbyItem],
    cursor: &mut usize,
    scroll: &mut usize,
    loading: bool,
    geometry: &mut PlaylistsRenderGeometry,
) {
    if loading && items.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Loading…",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            content,
        );
        return;
    }
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Playlist is empty",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            content,
        );
        return;
    }
    *cursor = (*cursor).min(items.len() - 1);
    *scroll = (*scroll).min(*cursor);
    let item_lines = |item: &EmbyItem| {
        usize::from(item.display_name().len() > (content.width as usize).saturating_sub(6)) + 1
    };
    while *scroll < *cursor {
        let lines = items[*scroll..=*cursor]
            .iter()
            .map(item_lines)
            .sum::<usize>();
        if lines <= content.height as usize {
            break;
        }
        *scroll += 1;
    }
    let mut y = 0usize;
    for (visible, item) in items[*scroll..].iter().enumerate() {
        if y >= content.height as usize {
            break;
        }
        let index = *scroll + visible;
        let selected = index == *cursor;
        let fg = if selected {
            palette::ACCENT_ACTIVE
        } else {
            palette::TEXT_PRIMARY
        };
        let num = format!("{:>2}. ", index + 1);
        let text_width = chrome::panel_row_text_width(content.width).saturating_sub(num.len());
        let label = item.display_name();
        let (line1, line2) = if label.len() <= text_width {
            (label, String::new())
        } else {
            let split = label[..text_width].rfind(' ').unwrap_or(text_width);
            (
                label[..split].to_string(),
                label[split..].trim_start().to_string(),
            )
        };
        let indent = " ".repeat(2 + num.len());
        let row_y = content.y + y as u16;
        let height = 1 + usize::from(!line2.is_empty());
        let target = Rect {
            x: content.x,
            y: row_y,
            width: content.width,
            height: (height as u16).min(content.bottom().saturating_sub(row_y)),
        };
        chrome::render_panel_row(
            frame,
            content.x,
            row_y,
            content.width,
            selected,
            vec![
                Span::styled(num, Style::default().fg(palette::TEXT_MUTED)),
                Span::styled(line1, Style::default().fg(fg)),
            ],
        );
        if !line2.is_empty() && y + 1 < content.height as usize {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw(indent),
                    Span::styled(
                        trunc_str(&line2, text_width),
                        Style::default().fg(palette::TEXT_SECONDARY),
                    ),
                ])),
                Rect {
                    x: content.x,
                    y: row_y + 1,
                    width: content.width,
                    height: 1,
                },
            );
        }
        geometry.open_rows.push((target, index));
        y += height;
    }
    let total = items.iter().map(item_lines).sum::<usize>();
    let before = items[..*scroll].iter().map(item_lines).sum::<usize>();
    chrome::render_sidebar_scrollbar(frame, content, total, before);
}

#[cfg(test)]
impl App {
    pub(in crate::app::render) fn render_playlists_panel(
        &mut self,
        f: &mut Frame,
        area: Option<Rect>,
    ) {
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
                "[↵]play [→]browse [n]rename [d]delete [r]refresh [Esc]close".to_string(),
            )
        };

        let content = match area {
            Some(area) => chrome::render_panel_shell_at(f, area, &title, &hint, true),
            None => chrome::render_panel_shell(f, f.area(), PLAYLISTS_PANEL_W, &title, &hint),
        };
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
                    Style::default().fg(palette::TEXT_SECONDARY),
                )),
                content,
            );
            return;
        }
        if self.playlists.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " No playlists found",
                    Style::default().fg(palette::TEXT_SECONDARY),
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
                palette::ACCENT_ACTIVE
            } else if is_loaded {
                palette::TEXT_ACCENT_MUTED
            } else {
                palette::TEXT_PRIMARY
            };
            let count_str = if pl.total_count > 0 {
                format!(" ({})", pl.total_count)
            } else {
                String::new()
            };
            let name_max =
                chrome::panel_row_text_width(content.width).saturating_sub(count_str.len());
            let row_y = content.y + vi as u16;
            chrome::render_panel_row(
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
                    Span::styled(count_str, Style::default().fg(palette::TEXT_MUTED)),
                ],
            );
        }
        chrome::render_sidebar_scrollbar(f, content, self.playlists.len(), self.playlists_scroll);
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
                    Style::default().fg(palette::TEXT_SECONDARY),
                )),
                content,
            );
            return;
        }
        if self.playlists_open_items.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " Playlist is empty",
                    Style::default().fg(palette::TEXT_SECONDARY),
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
                palette::ACCENT_ACTIVE
            } else {
                palette::TEXT_PRIMARY
            };
            let num_str = format!("{:>2}. ", abs_idx + 1);
            let text_w = chrome::panel_row_text_width(content.width).saturating_sub(num_str.len());
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
            chrome::render_panel_row(
                f,
                ix,
                row_y,
                content.width,
                selected,
                vec![
                    Span::styled(num_str, Style::default().fg(palette::TEXT_MUTED)),
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
                            Style::default().fg(palette::TEXT_SECONDARY),
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
        chrome::render_sidebar_scrollbar(f, content, total_lines, lines_before_scroll);
    }
}
