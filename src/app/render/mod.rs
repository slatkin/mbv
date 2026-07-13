mod home;
pub mod indicators;
mod library;
mod overlays;
mod playlist;
pub(crate) mod power;

use super::ui_util::{fmt_duration, trunc_str};
use super::{layout::AppLayout, palette, App};
use crate::app::layout::LayoutPlayback;
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Tabs};
use ratatui::Frame;
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

fn daemon_endpoint_label(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.eq_ignore_ascii_case("local") {
        return None;
    }
    if let Some(tcp) = endpoint.strip_prefix("tcp://") {
        return tcp
            .rsplit_once(':')
            .map(|(host, _port)| host)
            .filter(|host| !host.is_empty())
            .map(str::to_string);
    }
    if let Some(path) = endpoint.strip_prefix("unix://") {
        return std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
    }
    std::path::Path::new(endpoint)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn server_url_label(server_url: &str) -> Option<String> {
    let value = server_url.trim();
    if value.is_empty() {
        return None;
    }
    let without_scheme = value
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(value);
    without_scheme
        .split('/')
        .next()
        .and_then(|host_port| host_port.split('@').next_back())
        .and_then(|host_port| host_port.split(':').next())
        .filter(|host| !host.is_empty())
        .map(str::to_string)
}

impl App {
    pub fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        // Guard against zero-dimension terminal (e.g. minimized or piped).
        // `self.layout` is left untouched here -- it still reflects the last
        // frame that rendered in full.
        if area.width == 0 || area.height == 0 {
            return;
        }
        if area.width != self.terminal_width || area.height != self.terminal_height {
            self.card_image_states.clear();
            self.card_image_loading.clear();
        }
        self.terminal_width = area.width;
        self.terminal_height = area.height;
        if self.clamp_power_left_width() {
            self.save_prefs();
        }

        // Every render sub-call below writes into this fresh, local value
        // instead of `self.layout` directly. It's swapped into `self.layout`
        // in one atomic assignment only once this pass completes in full, so
        // an early return partway through (like the guard above) can never
        // leave `self.layout` holding a mix of fields from two different
        // frames.
        let mut layout = AppLayout::default();
        // The library-table geometry caches are indexed by library tab index
        // and sized once (to `self.libs.len()`) by `rebuild_library_tabs_from_views`,
        // not rebuilt every frame -- each render pass only overwrites the slot
        // for the currently-viewed library. Carry the existing sizing/values
        // forward so `get_mut(lib_idx)` below still finds a slot to write into;
        // otherwise every library-tab index in a freshly-defaulted (empty) Vec
        // would be unwritable and this state would never survive a frame.
        layout.library.lib_scroll = self.layout.library.lib_scroll.clone();
        layout.library.lib_row_heights = self.layout.library.lib_row_heights.clone();
        layout.library.lib_table_area = self.layout.library.lib_table_area.clone();

        let active = self.player.status.lock().unwrap().active;
        let show_controls = active || self.connected_session_id.is_some();
        let in_power = self.tab_idx == 1 && self.queue_view == super::QUEUE_VIEW_POWER;
        // The 3-state panel toggle (`h`) only applies while something is playing/connected.
        let mode = self.panel_mode;
        let playing_panel = show_controls;
        let onerow = playing_panel && mode == crate::config::PanelMode::OneRow;
        // In power view always reserve the player rows (title + controls) so that
        // content doesn't shift when the player appears or disappears.
        let reserve_player_rows = in_power && mode == crate::config::PanelMode::OneRow;
        let tabs_h: u16 = 1;
        let spacer_h: u16 = 1;
        let status_bar_h: u16 = 1;
        // seek = full-width seekbar row; title = now-playing row; controls = blank spacer below it.
        // status_bar is the persistent bottom row (session/queue state, toast) --
        // unlike the other player rows it is not conditional on onerow/reserve_player_rows.
        let (seek_h, gap_h, title_h, controls_h): (u16, u16, u16, u16) =
            if onerow || reserve_player_rows {
                (1, 0, 1, 1)
            } else {
                (1, 0, 0, 0)
            };
        let [tabs_area, _spacer_area, seek_area, _gap_area, title_area, _controls_area, main_area, status_bar_area] =
            Layout::vertical([
                Constraint::Length(tabs_h),
                Constraint::Length(spacer_h),
                Constraint::Length(seek_h),
                Constraint::Length(gap_h),
                Constraint::Length(title_h),
                Constraint::Length(controls_h),
                Constraint::Min(0),
                Constraint::Length(status_bar_h),
            ])
            .areas(area);

        // Full-width seekbar row: live bar when playing, plain grey divider otherwise.
        if seek_h > 0 {
            if show_controls {
                self.render_seekbar(f, seek_area, &mut layout.playback);
            } else {
                layout.playback.seekbar_area = Rect::default();
                let bar = "\u{2594}".repeat(seek_area.width as usize);
                f.render_widget(
                    Paragraph::new(Span::styled(bar, Style::default().fg(palette::SEEK_TRACK))),
                    seek_area,
                );
            }
        } else {
            layout.playback.seekbar_area = Rect::default();
        }
        // Indicator-bar click regions are never set anymore; clear them every frame.
        layout.playback.ind_mu = Rect::default();
        layout.playback.ind_rc = Rect::default();

        {
            // Tabs occupy the space between the left margin and VOL (right).
            let tabs_x = tabs_area.x + super::TABBAR_LEFT_RESERVE;
            let tabs_w = tabs_area
                .width
                .saturating_sub(super::TABBAR_LEFT_RESERVE + super::TABBAR_RIGHT_RESERVE);
            layout.tabs_area = Rect {
                x: tabs_x,
                width: tabs_w,
                ..tabs_area
            };

            // Volume badge (right-aligned), in the key·value badge style:
            // dim "VOL" key + bold value colored by level.
            let volume = self.playback_display_target().displayed_volume(self);
            let vol_color = if volume > 100 {
                palette::RED
            } else if volume > 60 {
                palette::YELLOW
            } else {
                palette::PINE
            };
            let vol_spans = vec![
                Span::styled("VOL ", Style::default().fg(palette::MUTED)),
                Span::styled(
                    volume.to_string(),
                    Style::default().fg(vol_color).add_modifier(Modifier::BOLD),
                ),
            ];
            let vol_w: u16 = vol_spans.iter().map(|s| s.content.width() as u16).sum();
            let vol_rect = Rect {
                x: tabs_area.x + tabs_area.width.saturating_sub(vol_w),
                y: tabs_area.y,
                width: vol_w,
                height: 1,
            };
            layout.tabbar_vol_area = vol_rect;
            f.render_widget(Paragraph::new(Line::from(vol_spans)), vol_rect);

            let (vis_start, vis_end) = self.visible_tab_range(tabs_w);
            let has_left = vis_start > 0;
            let has_right = vis_end < self.tab_count();
            let ind_style = Style::default().fg(palette::WHITE);
            let left_w: u16 = if has_left { 2 } else { 0 };
            let right_w: u16 = if has_right { 2 } else { 0 };
            if has_left {
                f.render_widget(
                    Paragraph::new("« ").style(ind_style),
                    Rect {
                        x: tabs_x,
                        y: tabs_area.y,
                        width: 2,
                        height: 1,
                    },
                );
            }
            if has_right {
                f.render_widget(
                    Paragraph::new(" »").style(ind_style),
                    Rect {
                        x: tabs_x + tabs_w.saturating_sub(2),
                        y: tabs_area.y,
                        width: 2,
                        height: 1,
                    },
                );
            }
            let inner_tabs = Rect {
                x: tabs_x + left_w,
                y: tabs_area.y,
                width: tabs_w.saturating_sub(left_w + right_w),
                height: tabs_area.height,
            };
            // In power view, show Home + Libraries (no Queue); selection = power_left_tab.
            // Otherwise, show the full tab list with the normal tab_idx highlight.
            let tab_titles: Vec<Line> = if in_power {
                let names: Vec<String> = std::iter::once("Home".to_string())
                    .chain(self.libs.iter().map(|l| l.library.name.clone()))
                    .collect();
                let sel = self.power_left_tab;
                names
                    .into_iter()
                    .enumerate()
                    .map(|(i, n)| {
                        let n = n.to_uppercase();
                        if i == sel {
                            Line::from(vec![
                                Span::styled("▐", Style::default().fg(palette::PINE)),
                                Span::styled(
                                    format!(" {n}  "),
                                    Style::default()
                                        .fg(palette::WHITE)
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ])
                        } else {
                            Line::from(Span::styled(
                                format!("  {n}  "),
                                Style::default().fg(palette::SUBTLE),
                            ))
                        }
                    })
                    .collect()
            } else {
                let all_names: Vec<String> = std::iter::once("Home".to_string())
                    .chain(std::iter::once("Queue".to_string()))
                    .chain(self.libs.iter().map(|l| l.library.name.clone()))
                    .collect();
                let selected_tab = if self.tab_idx < vis_start || self.tab_idx >= vis_end {
                    usize::MAX
                } else {
                    self.tab_idx - vis_start
                };
                all_names[vis_start..vis_end]
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        let n = n.to_uppercase();
                        if i == selected_tab {
                            // Left-aligned active tab: the queue-row indicator (▐, pine) flush
                            // against the bold white label, no underline.
                            Line::from(vec![
                                Span::styled("▐", Style::default().fg(palette::PINE)),
                                Span::styled(
                                    format!(" {n}  "),
                                    Style::default()
                                        .fg(palette::WHITE)
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ])
                        } else {
                            Line::from(Span::styled(
                                format!("  {n}  "),
                                Style::default().fg(palette::SUBTLE),
                            ))
                        }
                    })
                    .collect()
            };
            f.render_widget(
                Tabs::new(tab_titles)
                    .select(usize::MAX)
                    .style(Style::default().fg(palette::SUBTLE))
                    .highlight_style(Style::default())
                    .divider(Span::raw(""))
                    .padding("", ""),
                inner_tabs,
            );
        }

        // Persistent bottom status bar: pill + session/queue segments, or the
        // toast overlay when a message is active. render_status_bar always runs
        // first so its click hitboxes (ind_mu/ind_rc) stay populated every frame,
        // even while a toast visually covers this row -- Clear wipes the leftover
        // glyphs before the toast draws over them so nothing bleeds through.
        self.render_status_bar(f, status_bar_area, &mut layout.playback);
        let show_toast =
            !self.status.is_empty() && (!self.system_notifications || self.notif_failed);
        if show_toast {
            f.render_widget(Clear, status_bar_area);
            f.render_widget(
                Paragraph::new(Self::toast_line(&self.status))
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(palette::TEXT).bg(palette::IRIS)),
                status_bar_area,
            );
        }

        let now_playing: Option<String> = if active {
            let idx = self.player.status.lock().unwrap().current_idx;
            self.playback_queue()
                .items
                .get(idx)
                .map(|i| i.playback_label())
        } else {
            None
        };
        if self.status_expires.is_some_and(|t| t <= Instant::now()) {
            self.status.clear();
            self.status_expires = None;
            self.force_clear = true;
        }
        let title_color = palette::FOAM;
        let now_playing_title: Option<(String, Color)> =
            if playing_panel && mode != crate::config::PanelMode::Hidden {
                if active {
                    now_playing.map(|t| (t, title_color))
                } else if let Some(ref state) = self.connected_session_state {
                    state.now_playing.clone().map(|t| (t, title_color))
                } else {
                    None
                }
            } else {
                None
            };
        if let Some((ref title, color)) = now_playing_title {
            // The one-row now-playing header: "▶ Title │ time … badges".
            self.render_title_row(f, title_area, title, color, &mut layout.playback);
        }
        if self.tab_idx == 0 {
            self.render_combined(f, main_area, &mut layout.home);
        } else if self.tab_idx == 1 && self.queue_view == super::QUEUE_VIEW_POWER {
            self.render_power_view(f, main_area, &mut layout.power);
        } else if self.tab_idx == 1 {
            self.render_queue_panel(f, main_area, &mut layout.queue);
        } else {
            self.render_library(
                f,
                main_area,
                self.tab_idx - self.lib_tab_offset(),
                None,
                &mut layout.library,
            );
        }

        self.render_context_menu(f, &mut layout);

        if self.show_sessions {
            self.render_sessions_overlay(f);
        }
        if self.show_playlists {
            self.render_playlists_panel(f);
        }
        if self.show_help {
            self.render_help_panel(f);
        }
        if self.show_settings {
            self.render_settings_panel(f, &mut layout);
            if self.multiselect_popup.is_some() {
                self.render_multiselect_popup(f);
            }
        }
        if self.save_playlist_dialog.is_some() {
            self.render_save_playlist_dialog(f);
        }
        if self.show_save_playlist_modal {
            self.render_dirty_playlist_modal(f);
        }

        // One atomic replace, reached only once the full pass above has
        // completed -- `self.layout` never observes a half-updated frame.
        self.layout = layout;
    }

    fn toast_line(s: &str) -> Line<'static> {
        let text_style = Style::default()
            .fg(palette::TEXT)
            .add_modifier(Modifier::BOLD);
        let yellow_style = Style::default()
            .fg(palette::YELLOW)
            .add_modifier(Modifier::BOLD);
        let open = s.find(['[', '(']);
        if let Some(i) = open {
            let close = s[i..].find([']', ')']).map(|j| i + j);
            if let Some(j) = close {
                let mut spans = vec![
                    Span::styled(s[..i].to_string(), text_style),
                    Span::styled(s[i..i + 1].to_string(), text_style),
                ];
                for c in s[i + 1..j].chars() {
                    spans.push(if c.is_uppercase() {
                        Span::styled(c.to_string(), yellow_style)
                    } else {
                        Span::styled(c.to_string(), text_style)
                    });
                }
                spans.push(Span::styled(s[j..j + 1].to_string(), text_style));
                if j + 1 < s.len() {
                    spans.push(Span::styled(s[j + 1..].to_string(), text_style));
                }
                return Line::from(spans);
            }
        }
        Line::from(Span::styled(s.to_string(), text_style))
    }

    pub(super) fn render_panel_shell(
        f: &mut Frame,
        full: Rect,
        width: u16,
        title: &str,
        hints: &str,
    ) -> Rect {
        let sidebar = Rect {
            x: full.x,
            y: full.y + 2,
            width: width.min(full.width),
            height: full.height.saturating_sub(2),
        };
        f.render_widget(Clear, sidebar);
        // Too short to fit a title row, a content row, and the 2-row footer;
        // bail out rather than let `footer_y = sidebar.y + sidebar.height - 2`
        // underflow below.
        if sidebar.height < 4 || sidebar.width == 0 {
            return sidebar;
        }
        f.render_widget(
            Block::default().style(Style::default().bg(palette::PANEL_BG)),
            sidebar,
        );
        for row in sidebar.y..sidebar.y + sidebar.height {
            f.render_widget(
                Paragraph::new(Span::styled(
                    "\u{2502}",
                    Style::default().fg(palette::OVERLAY),
                )),
                Rect {
                    x: sidebar.x + sidebar.width - 1,
                    y: row,
                    width: 1,
                    height: 1,
                },
            );
        }
        let inner_w = sidebar.width.saturating_sub(1);
        let ix = sidebar.x;
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                title.to_owned(),
                Style::default()
                    .fg(palette::TEXT)
                    .add_modifier(Modifier::BOLD),
            )]))
            .style(Style::default().bg(palette::FOCUSED)),
            Rect {
                x: sidebar.x,
                y: sidebar.y,
                width: sidebar.width.saturating_sub(1),
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(Span::raw(" ")).style(Style::default().bg(palette::FOCUSED)),
            Rect {
                x: sidebar.x + sidebar.width - 1,
                y: sidebar.y,
                width: 1,
                height: 1,
            },
        );
        let footer_y = sidebar.y + sidebar.height - 2;
        f.render_widget(
            Paragraph::new(Span::styled(
                "\u{2500}".repeat(inner_w as usize),
                Style::default().fg(palette::OVERLAY),
            )),
            Rect {
                x: ix,
                y: footer_y,
                width: inner_w,
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                trunc_str(hints, inner_w as usize),
                Style::default().fg(palette::TEXT),
            )]))
            .style(Style::default().bg(palette::FOCUSED)),
            Rect {
                x: ix,
                y: footer_y + 1,
                width: inner_w,
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(Span::raw(" ")).style(Style::default().bg(palette::FOCUSED)),
            Rect {
                x: sidebar.x + sidebar.width - 1,
                y: footer_y + 1,
                width: 1,
                height: 1,
            },
        );
        Rect {
            x: ix,
            y: sidebar.y + 1,
            width: inner_w,
            height: sidebar.height.saturating_sub(3),
        }
    }

    /// Overlay a thin scroll indicator on a sidebar's right border column when
    /// its content doesn't fit `content.height`. Reuses the existing border
    /// column instead of reserving a dedicated width for a scrollbar.
    ///
    /// The thumb position/length are computed directly (rather than via
    /// ratatui's `Scrollbar` widget) because that widget's math assumes
    /// `position` can reach `content_length - 1` (list-style scrolling to the
    /// last item); our `scroll` is a paragraph offset clamped to
    /// `total - visible`, which is smaller whenever `total > visible + 1` and
    /// left the thumb short of the track's bottom.
    pub(super) fn render_sidebar_scrollbar(
        f: &mut Frame,
        content: Rect,
        total: usize,
        scroll: usize,
    ) {
        let track_len = content.height as usize;
        let visible = track_len;
        if track_len == 0 || total <= visible {
            return;
        }
        let max_offset = total - visible;
        let thumb_len = (track_len * visible / total).clamp(1, track_len);
        let max_thumb_start = track_len - thumb_len;
        let thumb_start = scroll.min(max_offset) * max_thumb_start / max_offset;
        let x = content.x + content.width;
        for row in 0..track_len {
            let is_thumb = row >= thumb_start && row < thumb_start + thumb_len;
            let (sym, style) = if is_thumb {
                ("\u{2590}", Style::default().fg(palette::PINE))
            } else {
                ("\u{2502}", Style::default().fg(palette::OVERLAY))
            };
            f.render_widget(
                Paragraph::new(Span::styled(sym, style)),
                Rect {
                    x,
                    y: content.y + row as u16,
                    width: 1,
                    height: 1,
                },
            );
        }
    }

    /// Render one row in a sidebar panel list.
    /// `content_spans` should not include the indicator — it is prepended automatically.
    /// Returns the usable text width (content area minus indicator and space).
    pub(super) fn panel_row_text_width(content_width: u16) -> usize {
        content_width.saturating_sub(1) as usize // indicator char
    }

    pub(super) fn render_panel_row(
        f: &mut Frame,
        x: u16,
        y: u16,
        width: u16,
        selected: bool,
        spans: Vec<Span>,
    ) {
        let indicator = Span::styled(
            if selected { "\u{258c}" } else { " " },
            Style::default().fg(palette::PINE),
        );
        let mut all = vec![indicator];
        all.extend(spans);
        f.render_widget(
            Paragraph::new(Line::from(all)),
            Rect {
                x,
                y,
                width,
                height: 1,
            },
        );
    }

    /// Build the playback status indicator items (res/codec, audio lang, CC), space-separated.
    /// Returns None if the local player is not active.
    /// Callers wrap these in [ ... ] with whatever surrounding style they need.
    pub(super) fn build_status_indicator_spans(&self) -> Option<Vec<Span<'static>>> {
        let data = self.playback_indicator_target().indicator_data(self)?;
        Some(indicators::indicator_spans(
            self.indicator_style,
            &data,
            self.use_nerd_fonts,
        ))
    }

    /// One-line now-playing header: play/pause, next, title, and time on the
    /// left, with the status-indicator badges right-aligned. Records click
    /// regions for the play/pause and next glyphs into `layout` (see issue
    /// #112); next is greyed out (and, per `handle_mouse`, non-clickable)
    /// when `transport_prev_next_available()` says the queue is at that
    /// boundary.
    fn render_title_row(
        &mut self,
        f: &mut Frame,
        area: Rect,
        title: &str,
        title_color: Color,
        layout: &mut LayoutPlayback,
    ) {
        if area.height == 0 || area.width == 0 {
            layout.play_pause_area = Rect::default();
            layout.next_area = Rect::default();
            return;
        }

        let (pos_ticks, rt_ticks, paused) = self.playback_progress();
        let pos_str = fmt_duration(pos_ticks / TICKS_PER_SECOND);
        let dur_str = fmt_duration(rt_ticks / TICKS_PER_SECOND);

        let (glyph, gcolor): (&str, Color) = if paused {
            (
                if self.use_nerd_fonts {
                    "\u{f04c}"
                } else {
                    "||"
                },
                palette::YELLOW,
            )
        } else {
            (
                if self.use_nerd_fonts { "\u{f04b}" } else { ">" },
                palette::PINE,
            )
        };

        let next_glyph = if self.use_nerd_fonts {
            "\u{f051}"
        } else {
            ">>"
        };
        let next_gap = " ";
        let next_avail = self.transport_prev_next_available().1;
        let next_color = if next_avail {
            palette::WHITE
        } else {
            palette::MUTED
        };
        let right = self.build_status_indicator_spans().unwrap_or_default();

        // Left: glyph  next  title  │  elapsed / total
        // A running `x` cursor tracks where each clickable glyph lands in the
        // rendered `Line`, so `layout.*_area` exactly matches what's on screen
        // rather than an estimate.
        let mut left: Vec<Span> = Vec::new();
        let mut x = area.x;

        let glyph_text = format!("{glyph} ");
        let glyph_w = glyph_text.width() as u16;
        layout.play_pause_area = Rect {
            x,
            y: area.y,
            width: glyph_w,
            height: 1,
        };
        x += glyph_w;
        left.push(Span::styled(
            glyph_text,
            Style::default().fg(gcolor).add_modifier(Modifier::BOLD),
        ));

        let next_w = next_glyph.width() as u16;
        layout.next_area = Rect {
            x,
            y: area.y,
            width: next_w,
            height: 1,
        };
        left.push(Span::styled(next_glyph, Style::default().fg(next_color)));

        left.push(Span::raw(next_gap));

        let sep_text = " \u{2502} ";
        let time_text = format!("{pos_str} / {dur_str}");
        let post_time_gap = "  ";
        let right_w: u16 = right.iter().map(|s| s.content.width() as u16).sum();
        let fixed_w = glyph_w as usize
            + next_w as usize
            + next_gap.width()
            + sep_text.width()
            + time_text.width()
            + post_time_gap.width()
            + right_w as usize;
        let title_w = (area.width as usize).saturating_sub(fixed_w);
        let title_text = if title_w == 0 {
            String::new()
        } else {
            trunc_str(title, title_w)
        };
        left.push(Span::styled(
            title_text,
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        ));

        left.push(Span::styled(
            sep_text,
            Style::default().fg(palette::OVERLAY),
        ));

        left.push(Span::styled(
            time_text,
            Style::default().fg(palette::SUBTLE),
        ));

        left.push(Span::raw(post_time_gap));

        let left_w: u16 = left.iter().map(|s| s.content.width() as u16).sum();
        let gap = area.width.saturating_sub(left_w + right_w) as usize;

        let mut spans = left;
        spans.push(Span::raw(" ".repeat(gap)));
        spans.extend(right);
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Current playback position / runtime (ticks) and paused state, from the
    /// connected remote session if any, otherwise the local player.
    fn playback_progress(&self) -> (i64, i64, bool) {
        if let Some(ref remote) = self.connected_session_state {
            let elapsed_s = self.remote_pos_at.elapsed().as_secs_f64();
            let pos_s = (self.remote_pos_s as f64 + elapsed_s).min(remote.runtime_s as f64);
            // Some Emby clients always report IsPaused=true even while playing.
            // Trust the API position advancing as the authoritative "actually playing" signal.
            let api_active = self.remote_api_pos_advanced_at.elapsed().as_secs() < 22;
            let is_paused = remote.is_paused && !api_active;
            (
                (pos_s * TICKS_PER_SECOND as f64) as i64,
                remote.runtime_s * TICKS_PER_SECOND,
                is_paused,
            )
        } else {
            let s = self.player.status.lock().unwrap();
            (s.position_ticks, s.runtime_ticks, s.paused)
        }
    }

    fn remote_status_spans(
        &self,
        remote_state: super::RemoteSlotState,
        daemon_endpoint: &str,
    ) -> Vec<Span<'static>> {
        let remote_on = matches!(
            remote_state,
            super::RemoteSlotState::AttachedSession | super::RemoteSlotState::DirectRemote
        );
        let mut glyph_style = Style::default().bg(palette::PILL_BG).fg(if remote_on {
            palette::YELLOW
        } else {
            palette::SUBTLE
        });
        if remote_on {
            glyph_style = glyph_style.add_modifier(Modifier::BOLD);
        }

        let target = match remote_state {
            super::RemoteSlotState::Off => None,
            super::RemoteSlotState::AttachedSession => {
                self.connected_session_state.as_ref().and_then(|session| {
                    let device_name = session.device_name.trim();
                    if !device_name.is_empty() {
                        Some(device_name.to_string())
                    } else {
                        let host = session.host.trim();
                        (!host.is_empty()).then(|| host.to_string())
                    }
                })
            }
            super::RemoteSlotState::DirectRemote => self
                .direct_remote_label
                .clone()
                .or_else(|| daemon_endpoint_label(daemon_endpoint)),
            super::RemoteSlotState::LocalDaemon => None,
        };
        let label = match target {
            Some(target) => format!("  {target}"),
            None => "  local".to_string(),
        };
        let label_style = if remote_on {
            Style::default()
                .fg(palette::PINE)
                .bg(palette::PILL_BG)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::SUBTLE).bg(palette::PILL_BG)
        };

        vec![
            Span::styled("\u{1F5A7}", glyph_style),
            Span::styled(label, label_style),
        ]
    }

    fn playlist_status_spans(&self) -> Vec<Span<'static>> {
        let (label, on) = match &self.queue_source {
            crate::config::QueueSource::Playlist { name, .. } => (format!("  {name}"), true),
            _ => ("  none".to_string(), false),
        };
        let glyph_style = Style::default()
            .bg(palette::PILL_BG)
            .fg(if on { palette::FOAM } else { palette::YELLOW })
            .add_modifier(if on {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        let label_style = Style::default()
            .fg(if on { palette::FOAM } else { palette::SUBTLE })
            .bg(palette::PILL_BG);

        vec![
            Span::styled("\u{1F5AD}", glyph_style),
            Span::styled(label, label_style),
        ]
    }

    fn mute_status_spans(&self) -> Option<Vec<Span<'static>>> {
        self.playback_display_target()
            .displayed_mute(self)
            .then(|| {
                vec![Span::styled(
                    "muted",
                    Style::default()
                        .fg(palette::RED)
                        .bg(palette::PILL_BG)
                        .add_modifier(Modifier::BOLD),
                )]
            })
    }

    fn status_width(spans: &[Span]) -> u16 {
        spans.iter().map(|s| s.content.width() as u16).sum()
    }

    fn append_status(spans: &mut Vec<Span<'static>>, status: Vec<Span<'static>>) {
        if !spans.is_empty() {
            spans.push(Span::styled(
                " | ",
                Style::default().fg(palette::SUBTLE).bg(palette::PILL_BG),
            ));
        }
        spans.extend(status);
    }

    fn render_remote_status_hitbox(&self, layout: &mut LayoutPlayback, area: Rect) {
        if area.width == 0 {
            layout.ind_rc = Rect::default();
        } else {
            layout.ind_rc = Rect {
                x: area.x,
                y: area.y,
                width: 1,
                height: 1,
            };
        }
    }

    /// Persistent bottom status bar. Left side: connection, playlist, stay-alive,
    /// and mute status groups. Right side: queue source/save-state/scope detail.
    fn render_status_bar(&mut self, f: &mut Frame, area: Rect, layout: &mut LayoutPlayback) {
        let bar_style = Style::default().bg(palette::PILL_BG);
        f.render_widget(Block::default().style(bar_style), area);
        layout.ind_mu = Rect::default();

        let remote_state = self.remote_slot_state();
        let (daemon_endpoint, server_url) = {
            let cfg = &self.client.lock().unwrap().config;
            (cfg.daemon_client_endpoint.clone(), cfg.server_url.clone())
        };
        let remote_status = self.remote_status_spans(remote_state, &daemon_endpoint);
        let playlist_status = self.playlist_status_spans();
        self.render_remote_status_hitbox(layout, area);

        let alive_status: Option<Vec<Span>> = self.stay_alive_ctrl.is_some().then(|| {
            vec![Span::styled(
                "\u{2665}",
                Style::default().fg(palette::RED).bg(palette::PILL_BG),
            )]
        });
        let mute_status = self.mute_status_spans();

        // Left-segment overflow priority: mute drops first if the combined
        // left segment wouldn't fit in the row, then playlist, then remote.
        let remote_w = Self::status_width(&remote_status);
        let playlist_w = Self::status_width(&playlist_status);
        let alive_w: u16 = alive_status
            .as_ref()
            .map(|spans| Self::status_width(spans))
            .unwrap_or(0);
        let mute_w: u16 = mute_status
            .as_ref()
            .map(|spans| Self::status_width(spans))
            .unwrap_or(0);
        let available = area.width;
        let joined_width = |widths: &[u16]| -> u16 {
            let mut total = 0u16;
            let mut count = 0u16;
            for width in widths.iter().copied().filter(|w| *w > 0) {
                total = total.saturating_add(width);
                if count > 0 {
                    total = total.saturating_add(3);
                }
                count += 1;
            }
            total
        };
        let fits_all = joined_width(&[remote_w, playlist_w, alive_w, mute_w]) <= available;
        let fits_without_alive =
            !fits_all && joined_width(&[remote_w, playlist_w, mute_w]) <= available;
        let fits_without_mute = !fits_all
            && !fits_without_alive
            && joined_width(&[remote_w, playlist_w]) <= available;
        let fits_without_remote = !fits_all
            && !fits_without_alive
            && !fits_without_mute
            && joined_width(&[playlist_w, alive_w]) <= available;

        let mut spans: Vec<Span> = Vec::new();
        if let Some(alive) = alive_status {
            Self::append_status(&mut spans, alive);
        }
        if fits_all || fits_without_alive || fits_without_mute {
            Self::append_status(&mut spans, remote_status);
            Self::append_status(&mut spans, playlist_status);
        } else if fits_without_remote {
            Self::append_status(&mut spans, playlist_status);
        }
        if fits_all || fits_without_alive {
            if let Some(mute) = mute_status {
                let mute_x =
                    area.x + Self::status_width(&spans) + if spans.is_empty() { 0 } else { 3 };
                let mute_w = Self::status_width(&mute);
                Self::append_status(&mut spans, mute);
                layout.ind_mu = Rect {
                    x: mute_x,
                    y: area.y,
                    width: mute_w,
                    height: 1,
                };
            }
        }

        // `left_content_w` tracks how far the left segment actually extends after
        // the above priority drop, so the right-segment overlap check can compare
        // against the real left edge instead of a hardcoded constant.
        let label_w: u16 = spans.iter().map(|s| s.content.width() as u16).sum();
        let left_content_w: u16 = label_w;
        if !spans.is_empty() {
            let label_rect = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(Line::from(spans)).style(bar_style),
                label_rect,
            );
        }

        {
            let mut right_spans: Vec<Span> = Vec::new();
            let source_label: Option<(String, Color)> = match &self.queue_source {
                crate::config::QueueSource::Playlist { .. } => None,
                crate::config::QueueSource::Album if self.tab_idx == 1 => {
                    Some(("ALBUM".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Series if self.tab_idx == 1 => {
                    Some(("SERIES".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Shuffle if self.tab_idx == 1 => {
                    Some(("SHUFFLE".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Remote if self.tab_idx == 1 => {
                    Some(("REMOTE Q".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Collection { collection_type } if self.tab_idx == 1 => {
                    Some((collection_type.to_uppercase(), palette::MUTED))
                }
                crate::config::QueueSource::Unknown => None,
                _ => None,
            };
            let append_right = |right_spans: &mut Vec<Span<'static>>, span: Span<'static>| {
                if !right_spans.is_empty() {
                    right_spans.push(Span::styled(
                        " | ",
                        Style::default().fg(palette::SUBTLE).bg(palette::PILL_BG),
                    ));
                }
                right_spans.push(span);
            };
            if let Some((label, color)) = source_label {
                append_right(
                    &mut right_spans,
                    Span::styled(label, Style::default().fg(color).bg(palette::PILL_BG)),
                );
            }
            let autosave_on = self.tab_idx == 1 && self.queue_is_saved_playlist() && {
                let cfg = &self.client.lock().unwrap().config;
                cfg.save_playlist_on_consume || cfg.save_playlist_on_consume_audio
            };
            if self.queue_dirty {
                append_right(
                    &mut right_spans,
                    Span::styled(
                        "UNSAVED",
                        Style::default()
                            .fg(palette::YELLOW)
                            .bg(palette::PILL_BG)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
            } else if autosave_on {
                append_right(
                    &mut right_spans,
                    Span::styled(
                        "AUTOSAVE",
                        Style::default().fg(palette::PINE).bg(palette::PILL_BG),
                    ),
                );
            }
            if let Some(server) = server_url_label(&server_url) {
                append_right(
                    &mut right_spans,
                    Span::styled(
                        server,
                        Style::default().fg(palette::SUBTLE).bg(palette::PILL_BG),
                    ),
                );
            }
            // Remote queue scope is omitted here: the active queue is already
            // apparent from the queue UI.
            if !right_spans.is_empty() {
                let right_w: u16 = right_spans.iter().map(|s| s.content.width() as u16).sum();
                // Compare against `left_content_w` (pill + session label, from Task 2),
                // not a hardcoded pill-only width -- otherwise this check passes while
                // the right segment still overlaps a rendered session label (e.g.
                // " ATTACHED" / " REMOTE ALIVE") on narrow terminals.
                let left_end = area.x + left_content_w;
                let right_x = area.x + area.width.saturating_sub(right_w);
                if right_x > left_end {
                    let right_rect = Rect {
                        x: right_x,
                        y: area.y,
                        width: right_w,
                        height: 1,
                    };
                    f.render_widget(
                        Paragraph::new(Line::from(right_spans)).style(bar_style),
                        right_rect,
                    );
                }
                // else: terminal too narrow for both segments -- right segment drops
                // silently rather than overlapping the pill or the session label.
                // (Design doc's open question on narrow-terminal truncation: right
                // segment yields first.)
            }
        }
    }

    /// Full-width seekbar row: green up to the playhead, gray for the remainder.
    /// No knob — the green/gray boundary marks the position. Records the click region.
    fn render_seekbar(&mut self, f: &mut Frame, area: Rect, layout: &mut LayoutPlayback) {
        if area.height == 0 || area.width == 0 {
            layout.seekbar_area = Rect::default();
            return;
        }
        let (pos_ticks, rt_ticks, _paused) = self.playback_progress();
        let ratio = if rt_ticks > 0 {
            (pos_ticks as f64 / rt_ticks as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        layout.seekbar_area = area;
        let w = area.width as usize;
        let green_len = ((ratio * w as f64).round() as usize).min(w);
        let gray_len = w - green_len;
        let spans = vec![
            Span::styled(
                "\u{2594}".repeat(green_len),
                Style::default().fg(palette::PINE),
            ),
            Span::styled(
                "\u{2594}".repeat(gray_len),
                Style::default().fg(palette::SEEK_TRACK),
            ),
        ];
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
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

    #[test]
    fn title_row_next_area_matches_rendered_next_glyph_width_and_position() {
        let mut app = make_app_stub();
        app.use_nerd_fonts = false;
        let next_glyph = ">>";
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 2;
            st.current_idx = 0;
            st.runtime_ticks = 90 * TICKS_PER_SECOND;
        }

        let backend = TestBackend::new(60, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPlayback::default();
        term.draw(|f| {
            app.render_title_row(
                f,
                Rect::new(0, 0, 60, 1),
                "Title",
                palette::FOAM,
                &mut layout,
            );
        })
        .unwrap();

        let line = buffer_to_string(&term).lines().next().unwrap().to_string();
        let next_byte = line.find(next_glyph).unwrap();
        let next_x = line[..next_byte].width() as u16;

        assert_eq!(layout.next_area.x, next_x);
        assert_eq!(layout.next_area.width, next_glyph.width() as u16);
        assert!(
            line.starts_with("> >> Title"),
            "expected next glyph between play/pause and title:\n{line}"
        );
    }

    #[test]
    fn title_row_next_area_matches_nerd_font_glyph_width_and_position() {
        let mut app = make_app_stub();
        app.use_nerd_fonts = true;
        let next_glyph = "\u{f051}";
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 2;
            st.current_idx = 0;
            st.runtime_ticks = 90 * TICKS_PER_SECOND;
        }

        let backend = TestBackend::new(60, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPlayback::default();
        term.draw(|f| {
            app.render_title_row(
                f,
                Rect::new(0, 0, 60, 1),
                "Title",
                palette::FOAM,
                &mut layout,
            );
        })
        .unwrap();

        let line = buffer_to_string(&term).lines().next().unwrap().to_string();
        let next_byte = line.find(next_glyph).unwrap();
        let next_x = line[..next_byte].width() as u16;

        assert_eq!(layout.next_area.x, next_x);
        assert_eq!(layout.next_area.width, next_glyph.width() as u16);
    }

    #[test]
    fn title_row_truncates_long_title_before_transport_status_and_badges() {
        let mut app = make_app_stub();
        app.use_nerd_fonts = false;
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 2;
            st.current_idx = 0;
            st.position_ticks = 65 * TICKS_PER_SECOND;
            st.runtime_ticks = 90 * TICKS_PER_SECOND;
            st.video_height = 1080;
            st.audio_lang = "en".into();
        }

        let backend = TestBackend::new(50, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPlayback::default();
        term.draw(|f| {
            app.render_title_row(
                f,
                Rect::new(0, 0, 50, 1),
                "This is an extremely long title that would otherwise push controls away",
                palette::FOAM,
                &mut layout,
            );
        })
        .unwrap();

        let line = buffer_to_string(&term).lines().next().unwrap().to_string();

        assert!(
            line.contains('\u{2026}'),
            "expected long title to be truncated with ellipsis:\n{line}"
        );
        assert!(
            line.contains("1:05 / 1:30"),
            "expected time cluster to remain visible:\n{line}"
        );
        assert!(
            line.ends_with("RES 1080p  AUD en  SUB off"),
            "expected status badges to remain right-aligned:\n{line}"
        );
        assert!(layout.next_area.x + layout.next_area.width <= 50);
    }
}
