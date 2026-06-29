mod home;
pub mod indicators;
mod library;
mod log;
mod overlays;
mod playlist;
mod power;

use std::time::Instant;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Tabs};
use unicode_width::UnicodeWidthStr;
use crate::api::TICKS_PER_SECOND;
use super::{App, palette};
use super::ui_util::{fmt_duration, trunc_str};

impl App {
    pub fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        // Guard against zero-dimension terminal (e.g. minimized or piped).
        if area.width == 0 || area.height == 0 { return; }
        if area.width != self.terminal_width || area.height != self.terminal_height {
            self.card_image_states.clear();
            self.card_image_loading.clear();
        }
        self.terminal_width = area.width;
        self.terminal_height = area.height;

        let active = self.player.status.lock().unwrap().active;
        let show_controls = active || self.connected_session_id.is_some();
        let in_power = self.tab_idx == 1 && self.playlist_view == super::PLAYLIST_VIEW_POWER;
        // The 3-state panel toggle (`h`) only applies while something is playing/connected.
        let mode = self.panel_mode;
        let playing_panel = show_controls;
        let onerow = playing_panel && mode == crate::config::PanelMode::OneRow;
        // In power view always reserve the player rows (title + controls) so that
        // content doesn't shift when the player appears or disappears.
        let reserve_player_rows = in_power && mode == crate::config::PanelMode::OneRow;
        let tabs_h:  u16 = 1;
        let spacer_h: u16 = 0;
        // seek = full-width seekbar row; title = now-playing row; controls = blank spacer below it. (status is unused.)
        let (seek_h, gap_h, title_h, controls_h, status_h): (u16, u16, u16, u16, u16) = if onerow || reserve_player_rows {
            (1, 0, 1, 1, 0)
        } else {
            (1, 0, 0, 0, 0)
        };
        let [tabs_area, _spacer_area, seek_area, _gap_area, title_area, _controls_area, _status_area, main_area] = Layout::vertical([
            Constraint::Length(tabs_h),
            Constraint::Length(spacer_h),
            Constraint::Length(seek_h),
            Constraint::Length(gap_h),
            Constraint::Length(title_h),
            Constraint::Length(controls_h),
            Constraint::Length(status_h),
            Constraint::Min(0),
        ]).areas(area);

        // Full-width seekbar row: live bar when playing, plain grey divider otherwise.
        if seek_h > 0 {
            if show_controls {
                self.render_seekbar(f, seek_area);
            } else {
                self.layout_seekbar_area = Rect::default();
                let bar = "\u{2500}".repeat(seek_area.width as usize);
                f.render_widget(
                    Paragraph::new(Span::styled(bar, Style::default().fg(palette::SEEK_TRACK))),
                    seek_area,
                );
            }
        } else {
            self.layout_seekbar_area = Rect::default();
        }
        // Indicator-bar click regions are never set anymore; clear them every frame.
        self.layout_ind_pb  = Rect::default();
        self.layout_ind_mu  = Rect::default();
        self.layout_ind_rc  = Rect::default();
        self.layout_ind_sub = Rect::default();
        self.layout_ind_au  = Rect::default();

        {
            // Control pill (m ⇌ ≡) on the far left of the tab bar.
            self.render_control_pill(f, tabs_area);

            // Tabs occupy the space between the control pill (left) and VOL (right).
            let tabs_x = tabs_area.x + super::TABBAR_LEFT_RESERVE;
            let tabs_w = tabs_area.width.saturating_sub(super::TABBAR_LEFT_RESERVE + super::TABBAR_RIGHT_RESERVE);
            self.layout_tabs_area = Rect { x: tabs_x, width: tabs_w, ..tabs_area };

            // Volume badge (right-aligned), in the key·value badge style:
            // dim "VOL" key + bold value colored by level.
            let volume = if let Some(ref remote) = self.connected_session_state {
                remote.volume
            } else {
                let s = self.player.status.lock().unwrap();
                if s.active { if s.muted { 0 } else { s.volume } } else { self.ui_volume as i64 }
            };
            let vol_color = if volume > 100 { palette::RED } else if volume > 60 { palette::YELLOW } else { palette::PINE };
            let vol_spans = vec![
                Span::styled("VOL ", Style::default().fg(palette::MUTED)),
                Span::styled(volume.to_string(), Style::default().fg(vol_color).add_modifier(Modifier::BOLD)),
            ];
            let vol_w: u16 = vol_spans.iter().map(|s| s.content.width() as u16).sum();
            let vol_rect = Rect { x: tabs_area.x + tabs_area.width.saturating_sub(vol_w), y: tabs_area.y, width: vol_w, height: 1 };
            self.layout_tabbar_vol_area = vol_rect;
            f.render_widget(Paragraph::new(Line::from(vol_spans)), vol_rect);

        let (vis_start, vis_end) = self.visible_tab_range(tabs_w);
        let has_left  = vis_start > 0;
        let has_right = vis_end < self.tab_count();
        let ind_style = Style::default().fg(palette::WHITE);
        let left_w:  u16 = if has_left  { 2 } else { 0 };
        let right_w: u16 = if has_right { 2 } else { 0 };
        if has_left {
            f.render_widget(
                Paragraph::new("« ").style(ind_style),
                Rect { x: tabs_x, y: tabs_area.y, width: 2, height: 1 },
            );
        }
        if has_right {
            f.render_widget(
                Paragraph::new(" »").style(ind_style),
                Rect { x: tabs_x + tabs_w.saturating_sub(2), y: tabs_area.y, width: 2, height: 1 },
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
            names.into_iter().enumerate().map(|(i, n)| {
                let n = n.to_uppercase();
                if i == sel {
                    Line::from(vec![
                        Span::styled("▐", Style::default().fg(palette::PINE)),
                        Span::styled(format!(" {n}  "), Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)),
                    ])
                } else {
                    Line::from(Span::styled(format!("  {n}  "), Style::default().fg(palette::SUBTLE)))
                }
            }).collect()
        } else {
            let all_names: Vec<String> = std::iter::once("Home".to_string())
                .chain(std::iter::once("Queue".to_string()))
                .chain(self.libs.iter().map(|l| l.library.name.clone()))
                .chain(self.show_log_tab.then(|| "Log".to_string()))
                .collect();
            let selected_tab = if (!self.show_log_tab && self.tab_idx == self.log_tab_idx()) || self.tab_idx < vis_start || self.tab_idx >= vis_end {
                usize::MAX
            } else {
                self.tab_idx - vis_start
            };
            all_names[vis_start..vis_end].iter().enumerate().map(|(i, n)| {
                let n = n.to_uppercase();
                if i == selected_tab {
                    // Left-aligned active tab: the queue-row indicator (▐, pine) flush
                    // against the bold white label, no underline.
                    Line::from(vec![
                        Span::styled("▐", Style::default().fg(palette::PINE)),
                        Span::styled(format!(" {n}  "), Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)),
                    ])
                } else {
                    Line::from(Span::styled(format!("  {n}  "), Style::default().fg(palette::SUBTLE)))
                }
            }).collect()
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

        let now_playing: Option<String> = if active {
            let idx = self.player.status.lock().unwrap().current_idx;
            self.player_tab.items.get(idx).map(|i| i.playback_label())
        } else {
            None
        };
        if self.status_expires.is_some_and(|t| t <= Instant::now()) {
            self.status.clear();
            self.status_expires = None;
            self.force_clear = true;
        }
        let title_color = palette::FOAM;
        let now_playing_title: Option<(String, Color)> = if playing_panel && mode != crate::config::PanelMode::Hidden {
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
            self.render_title_row(f, title_area, title, color);
        }
        // These control regions no longer exist (expanded view removed).
        self.layout_tracks_area  = Rect::default();
        self.layout_vol_area     = Rect::default();
        self.layout_sub_area     = Rect::default();
        self.layout_audio_area   = Rect::default();
        self.layout_button_area  = Rect::default();

        if self.tab_idx == 0 {
            self.render_combined(f, main_area);
        } else if self.tab_idx == 1 && self.playlist_view == super::PLAYLIST_VIEW_POWER {
            self.render_power_view(f, main_area);
        } else if self.tab_idx == 1 {
            self.render_playlist_panel(f, main_area);
        } else if self.tab_idx == self.log_tab_idx() {
            self.render_log(f, main_area);
        } else {
            self.render_library(f, main_area, self.tab_idx - self.lib_tab_offset(), None);
        }

        if !self.status.is_empty() && (!self.system_notifications || self.notif_failed) {
            let toast_rect = Rect { x: area.x, y: area.y + area.height - 3, width: area.width, height: 3 };
            f.render_widget(Clear, toast_rect);
            f.render_widget(
                Paragraph::new(Self::toast_line(&self.status))
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(palette::TEXT).bg(palette::IRIS))
                    .block(Block::default().style(Style::default().fg(palette::TEXT).bg(palette::IRIS)).padding(ratatui::widgets::Padding::vertical(1))),
                toast_rect,
            );
        }

        self.render_context_menu(f);

        if self.show_sessions  { self.render_sessions_overlay(f); }
        if self.show_playlists { self.render_playlists_panel(f); }
        if self.show_help      { self.render_help_panel(f); }
        if self.show_settings  {
            self.render_settings_panel(f);
            if self.multiselect_popup.is_some() { self.render_multiselect_popup(f); }
        }
        if self.save_playlist_dialog.is_some() { self.render_save_playlist_dialog(f); }
        if self.show_save_playlist_modal { self.render_dirty_playlist_modal(f); }
    }

    fn toast_line(s: &str) -> Line<'static> {
        let text_style   = Style::default().fg(palette::TEXT).add_modifier(Modifier::BOLD);
        let yellow_style = Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD);
        let open = s.find(['[', '(']);
        if let Some(i) = open {
            let close = s[i..].find([']', ')']).map(|j| i + j);
            if let Some(j) = close {
                let mut spans = vec![
                    Span::styled(s[..i].to_string(),     text_style),
                    Span::styled(s[i..i+1].to_string(),  text_style),
                ];
                for c in s[i+1..j].chars() {
                    spans.push(if c.is_uppercase() {
                        Span::styled(c.to_string(), yellow_style)
                    } else {
                        Span::styled(c.to_string(), text_style)
                    });
                }
                spans.push(Span::styled(s[j..j+1].to_string(), text_style));
                if j + 1 < s.len() {
                    spans.push(Span::styled(s[j+1..].to_string(), text_style));
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
        let sidebar = Rect { x: full.x, y: full.y, width: width.min(full.width), height: full.height };
        f.render_widget(Clear, sidebar);
        f.render_widget(Block::default().style(Style::default().bg(palette::PANEL_BG)), sidebar);
        for row in sidebar.y..sidebar.y + sidebar.height {
            f.render_widget(
                Paragraph::new(Span::styled("\u{2502}", Style::default().fg(palette::OVERLAY))),
                Rect { x: sidebar.x + sidebar.width - 1, y: row, width: 1, height: 1 },
            );
        }
        let inner_w = sidebar.width.saturating_sub(1);
        let ix = sidebar.x;
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(title.to_owned(), Style::default().fg(palette::TEXT).add_modifier(Modifier::BOLD)),
            ])).style(Style::default().bg(palette::FOCUSED)),
            Rect { x: sidebar.x, y: sidebar.y, width: sidebar.width.saturating_sub(1), height: 1 },
        );
        f.render_widget(
            Paragraph::new(Span::raw(" ")).style(Style::default().bg(palette::FOCUSED)),
            Rect { x: sidebar.x + sidebar.width - 1, y: sidebar.y, width: 1, height: 1 },
        );
        let footer_y = sidebar.y + sidebar.height - 2;
        f.render_widget(
            Paragraph::new(Span::styled("\u{2500}".repeat(inner_w as usize), Style::default().fg(palette::OVERLAY))),
            Rect { x: ix, y: footer_y, width: inner_w, height: 1 },
        );
        f.render_widget(
            Paragraph::new(Span::styled(trunc_str(hints, inner_w as usize), Style::default().fg(palette::MUTED))),
            Rect { x: ix, y: footer_y + 1, width: inner_w, height: 1 },
        );
        Rect { x: ix, y: sidebar.y + 1, width: inner_w, height: sidebar.height.saturating_sub(3) }
    }

    /// Render one row in a sidebar panel list.
    /// `content_spans` should not include the indicator — it is prepended automatically.
    /// Returns the usable text width (content area minus indicator and space).
    pub(super) fn panel_row_text_width(content_width: u16) -> usize {
        content_width.saturating_sub(1) as usize // indicator char
    }

    pub(super) fn render_panel_row(f: &mut Frame, x: u16, y: u16, width: u16, selected: bool, spans: Vec<Span>) {
        let indicator = Span::styled(
            if selected { "\u{258c}" } else { " " },
            Style::default().fg(palette::PINE),
        );
        let mut all = vec![indicator];
        all.extend(spans);
        f.render_widget(Paragraph::new(Line::from(all)), Rect { x, y, width, height: 1 });
    }


    /// Build the playback status indicator items (res/codec, audio lang, CC), space-separated.
    /// Returns None if the local player is not active.
    /// Callers wrap these in [ ... ] with whatever surrounding style they need.
    pub(super) fn build_status_indicator_spans(&self) -> Option<Vec<Span<'static>>> {
        let pst = self.player.status.lock().unwrap();
        if !pst.active { return None; }
        let video_is_image = pst.video_is_image;
        let res_h = pst.video_height;
        let is_audio_only = video_is_image;
        let res_str = if video_is_image || res_h == 0 {
            if pst.audio_codec.is_empty() { "--".to_string() } else { pst.audio_codec.to_uppercase() }
        } else {
            format!("{}p", res_h)
        };
        let res_dim = res_str == "--";
        let raw_lang = pst.audio_lang.to_lowercase();
        let (au_text, audio_dim): (String, bool) = if raw_lang.is_empty() {
            ("x".into(), true)
        } else {
            (raw_lang.chars().take(2).collect(), false)
        };
        let sub_id = pst.sub_id;
        let raw_sub_lang = pst.sub_lang.to_lowercase();
        drop(pst);
        let sub_label: String = if sub_id == 0 {
            "off".into()
        } else if !raw_sub_lang.is_empty() {
            raw_sub_lang.chars().take(3).collect()
        } else {
            "CC".into()
        };
        let data = indicators::IndicatorData {
            res_label: res_str,
            res_dim,
            audio_label: au_text,
            audio_dim,
            audio_only: is_audio_only,
            sub_label,
        };
        Some(indicators::indicator_spans(self.indicator_style, &data, self.use_nerd_fonts))
    }

    /// One-line now-playing header: `▶ Title │ elapsed / total` on the left,
    /// the status-indicator badges right-aligned. Mirrors the design handoff.
    fn render_title_row(&mut self, f: &mut Frame, area: Rect, title: &str, title_color: Color) {
        if area.height == 0 || area.width == 0 { return; }

        let (pos_ticks, rt_ticks, paused) = self.playback_progress();
        let pos_str = fmt_duration(pos_ticks / TICKS_PER_SECOND);
        let dur_str = fmt_duration(rt_ticks / TICKS_PER_SECOND);

        let (glyph, gcolor): (&str, Color) = if paused {
            (if self.use_nerd_fonts { "\u{f04c}" } else { "||" }, palette::YELLOW)
        } else {
            (if self.use_nerd_fonts { "\u{f04b}" } else { ">" }, palette::PINE)
        };

        // Left: glyph  title  │  elapsed / total
        let mut left: Vec<Span> = Vec::new();
        left.push(Span::styled(format!("{glyph} "), Style::default().fg(gcolor).add_modifier(Modifier::BOLD)));
        left.push(Span::styled(title.to_string(), Style::default().fg(title_color).add_modifier(Modifier::BOLD)));
        left.push(Span::styled(" \u{2502} ", Style::default().fg(palette::OVERLAY)));
        left.push(Span::styled(format!("{pos_str} / {dur_str}"), Style::default().fg(palette::SUBTLE)));

        // Right: status-indicator badges.
        let right = self.build_status_indicator_spans().unwrap_or_default();

        let left_w:  u16 = left.iter().map(|s| s.content.width() as u16).sum();
        let right_w: u16 = right.iter().map(|s| s.content.width() as u16).sum();
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
            ((pos_s * TICKS_PER_SECOND as f64) as i64, remote.runtime_s * TICKS_PER_SECOND, remote.is_paused)
        } else {
            let s = self.player.status.lock().unwrap();
            (s.position_ticks, s.runtime_ticks, s.paused)
        }
    }

    /// Control pill on the far left of the tab bar: `  m ⇌ ≡  ` on an always-green
    /// background. Each icon is its assigned color when ON, or reverse-video
    /// (dark on green) when OFF. `m` mute and `⇌` remote are clickable.
    fn render_control_pill(&mut self, f: &mut Frame, tabs_area: Rect) {
        let bg = palette::PILL_BG;
        let mute_on = self.mute_on;
        let connected = self.connected_session_id.is_some();
        let is_playlist = matches!(&self.queue_source, crate::config::QueueSource::Playlist { .. });
        let icon = |on: bool, on_color: Color| {
            // OFF: no explicit foreground (terminal default bleeds through).
            Style::default().bg(bg).fg(if on { on_color } else { Color::Reset }).add_modifier(Modifier::BOLD)
        };
        let pad = Style::default().bg(bg);
        let (x, y) = (tabs_area.x, tabs_area.y);
        // Layout: "  m ⇌ ≡  " — m at x+2, ⇌ at x+4, ≡ at x+6.
        self.layout_ind_mu = Rect { x: x + 2, y, width: 1, height: 1 };
        self.layout_ind_rc = Rect { x: x + 4, y, width: 1, height: 1 };
        let spans = vec![
            Span::styled("  ", pad),
            Span::styled("m", icon(mute_on, palette::RED)),
            Span::styled(" ", pad),
            Span::styled("\u{21CC}", icon(connected, palette::YELLOW)),
            Span::styled(" ", pad),
            Span::styled("\u{2261}", icon(is_playlist, palette::FOAM)),
            Span::styled("  ", pad),
        ];
        f.render_widget(Paragraph::new(Line::from(spans)), Rect { x, y, width: 9, height: 1 });
    }

    /// Full-width seekbar row: green up to the playhead, gray for the remainder.
    /// No knob — the green/gray boundary marks the position. Records the click region.
    fn render_seekbar(&mut self, f: &mut Frame, area: Rect) {
        if area.height == 0 || area.width == 0 { self.layout_seekbar_area = Rect::default(); return; }
        let (pos_ticks, rt_ticks, _paused) = self.playback_progress();
        let ratio = if rt_ticks > 0 { (pos_ticks as f64 / rt_ticks as f64).clamp(0.0, 1.0) } else { 0.0 };
        self.layout_seekbar_area = area;
        let w = area.width as usize;
        let green_len = ((ratio * w as f64).round() as usize).min(w);
        let gray_len = w - green_len;
        let spans = vec![
            Span::styled("\u{2500}".repeat(green_len), Style::default().fg(palette::PINE)),
            Span::styled("\u{2500}".repeat(gray_len), Style::default().fg(palette::SEEK_TRACK)),
        ];
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    
}
