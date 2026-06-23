mod home;
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
        if area.width != self.terminal_width || area.height != self.terminal_height {
            self.card_image_states.clear();
            self.card_image_loading.clear();
        }
        self.terminal_width = area.width;
        self.terminal_height = area.height;

        let active = self.player.status.lock().unwrap().active;
        let show_controls = active || self.connected_session_id.is_some();
        let in_presentation = self.tab_idx == 1 && self.playlist_view == 2;
        let in_power       = self.tab_idx == 1 && self.playlist_view == super::PLAYLIST_VIEW_POWER;
        let status_h:   u16 = if show_controls && !in_presentation && !in_power && self.show_playback_panel { 1 } else { 0 };
        let controls_h: u16 = if show_controls && !in_presentation && !in_power && self.show_playback_panel { 2 } else { 0 };
        let [tabs_area, gap_area, title_area, controls_area, status_area, main_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(controls_h),
            Constraint::Length(status_h),
            Constraint::Min(0),
        ]).areas(area);

        const VOL_W: u16 = 9; // " Vol XXX%"
        let right_w = VOL_W + 1; // +1 so the trailing % aligns with the last ] on the divider row

        {
            let dash_style = Style::default().fg(palette::MUTED);
            let bracket = Style::default().fg(palette::WHITE);

            let pst = self.player.status.lock().unwrap();
            let active = pst.active;

            let res_h = pst.video_height;
            let res_str = if res_h > 0 { format!("{}p", res_h) } else { "--".to_string() };
            let res_color = if res_h > 0 { palette::FOAM } else { palette::MUTED };

            let sub_color = if pst.sub_id != 0 { palette::RED } else { palette::MUTED };

            let raw_lang = pst.audio_lang.to_lowercase();
            let (au_text, au_color): (String, Color) = if raw_lang.is_empty() {
                ("x".into(), palette::MUTED)
            } else {
                (raw_lang.chars().take(2).collect(), palette::YELLOW)
            };

            let (pb_text, pb_color): (&str, Color) = if pst.active && !pst.paused {
                if self.use_nerd_fonts { ("\u{f04b}", palette::PINE) } else { (">", palette::PINE) }
            } else if pst.active && pst.paused {
                if self.use_nerd_fonts { ("\u{f04c}", palette::YELLOW) } else { ("||", palette::YELLOW) }
            } else {
                if self.use_nerd_fonts { ("\u{f04d}", palette::MUTED) } else { (" ", palette::MUTED) }
            };
            drop(pst);

            let mu_color = if self.mute_on { palette::RED } else { palette::MUTED };

            let (rc_text, rc_color): (&str, Color) = if self.connected_session_id.is_some() {
                ("↯", palette::PINE)
            } else {
                ("↯", palette::MUTED)
            };

            let ind_w = |text: &str| -> u16 { 1 + text.width() as u16 + 1 + 1 }; // "[X]─"
            let playback_ind_w = if active {
                ind_w(&res_str) + ind_w(&au_text) + ind_w("字")
            } else { 0 };
            let dash_count = gap_area.width.saturating_sub(
                playback_ind_w + ind_w(rc_text) + ind_w("m") + ind_w(pb_text)
            ) as usize;
            let mut spans: Vec<Span> = vec![Span::styled("─".repeat(dash_count), dash_style)];
            if active {
                spans.extend([
                    Span::styled("[", bracket),
                    Span::styled(res_str.clone(), Style::default().fg(res_color).add_modifier(Modifier::BOLD)),
                    Span::styled("]", bracket),
                    Span::styled("─", dash_style),
                    Span::styled("[", bracket),
                    Span::styled(au_text.clone(), Style::default().fg(au_color).add_modifier(Modifier::BOLD)),
                    Span::styled("]", bracket),
                    Span::styled("─", dash_style),
                    Span::styled("[", bracket),
                    Span::styled("字", Style::default().fg(sub_color).add_modifier(Modifier::BOLD)),
                    Span::styled("]", bracket),
                    Span::styled("─", dash_style),
                ]);
            }
            spans.extend([
                Span::styled("[", bracket),
                Span::styled(rc_text.to_string(), if self.connected_session_id.is_some() { Style::default().fg(rc_color).add_modifier(Modifier::BOLD) } else { Style::default().fg(rc_color) }),
                Span::styled("]", bracket),
                Span::styled("─", dash_style),
                Span::styled("[", bracket),
                Span::styled("m", Style::default().fg(mu_color).add_modifier(Modifier::BOLD)),
                Span::styled("]", bracket),
                Span::styled("─", dash_style),
                Span::styled("[", bracket),
                Span::styled(pb_text.to_string(), Style::default().fg(pb_color).add_modifier(Modifier::BOLD)),
                Span::styled("]", bracket),
                Span::styled("─", dash_style),
            ]);
            f.render_widget(Paragraph::new(Line::from(spans)), gap_area);
        }
        let vol_area = Rect {
            x: tabs_area.x + tabs_area.width.saturating_sub(right_w),
            y: tabs_area.y, width: VOL_W, height: 1,
        };
        self.layout_tabbar_vol_area = vol_area;
        self.render_volume_bar(f, vol_area);
        let tabs_area = Rect { width: tabs_area.width.saturating_sub(right_w), ..tabs_area };
        self.layout_tabs_area = tabs_area;

        let (vis_start, vis_end) = self.visible_tab_range(tabs_area.width);
        let has_left  = vis_start > 0;
        let has_right = vis_end < self.tab_count();
        let ind_style = Style::default().fg(palette::WHITE);
        let left_w:  u16 = if has_left  { 2 } else { 0 };
        let right_w: u16 = if has_right { 2 } else { 0 };
        if has_left {
            f.render_widget(
                Paragraph::new("« ").style(ind_style),
                Rect { x: tabs_area.x, y: tabs_area.y, width: 2, height: 1 },
            );
        }
        if has_right {
            f.render_widget(
                Paragraph::new(" »").style(ind_style),
                Rect { x: tabs_area.x + tabs_area.width - 2, y: tabs_area.y, width: 2, height: 1 },
            );
        }
        let inner_tabs = Rect {
            x: tabs_area.x + left_w,
            y: tabs_area.y,
            width: tabs_area.width.saturating_sub(left_w + right_w),
            height: tabs_area.height,
        };
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
        let tab_titles: Vec<Span> = all_names[vis_start..vis_end]
            .iter().enumerate().map(|(i, n)| {
                if i == selected_tab {
                    Span::styled(format!("  {n}  "), Style::default().fg(palette::WHITE).bg(palette::IRIS).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled(format!("  {n}  "), Style::default().fg(palette::YELLOW))
                }
            }).collect();
        f.render_widget(
            Tabs::new(tab_titles)
                .select(usize::MAX)
                .style(Style::default().fg(palette::SUBTLE))
                .highlight_style(Style::default())
                .divider(Span::raw(""))
                .padding("", ""),
            inner_tabs,
        );

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
        let now_playing_title: Option<(String, Color)> = if show_controls && !in_presentation && self.show_playback_panel {
            if active {
                now_playing.map(|t| (t, palette::FOAM))
            } else if let Some(ref state) = self.connected_session_state {
                state.now_playing.clone().map(|t| (t, palette::FOAM))
            } else {
                None
            }
        } else {
            None
        };
        if let Some((ref title, color)) = now_playing_title {
            let render_title_area = if in_power {
                let left_w = ((area.width as u32 * 2 / 5) as u16).clamp(20, 60);
                let divider_x = area.x + left_w;
                Rect { x: divider_x + 1, width: area.width.saturating_sub(left_w + 1), ..title_area }
            } else {
                title_area
            };
            f.render_widget(
                Paragraph::new(title.as_str())
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
                render_title_area,
            );
        }
        let is_library_tab = self.tab_idx >= self.lib_tab_offset()
            && self.tab_idx != self.log_tab_idx();
        let panel_and_library = show_controls && !in_presentation && self.show_playback_panel && is_library_tab;

        if show_controls && !in_presentation && self.show_playback_panel {
            // Always draw the dashes; render_library overlays breadcrumb text
            // on top when this is a library tab.
            f.render_widget(
                Paragraph::new(Span::styled(
                    "─".repeat(area.width as usize),
                    Style::default().fg(palette::MUTED),
                )),
                status_area,
            );
            self.render_playback_controls(f, controls_area);
        } else if !in_presentation {
            self.layout_seekbar_area = Rect::default();
            self.layout_button_area  = Rect::default();
            self.layout_tracks_area  = Rect::default();
            self.layout_vol_area     = Rect::default();
            self.layout_sub_area     = Rect::default();
            self.layout_audio_area   = Rect::default();
        }

        if self.tab_idx == 0 {
            self.render_combined(f, main_area);
        } else if self.tab_idx == 1 && self.playlist_view == super::PLAYLIST_VIEW_POWER {
            self.render_power_view(f, main_area);
        } else if self.tab_idx == 1 {
            self.render_playlist_panel(f, main_area);
        } else if self.tab_idx == self.log_tab_idx() {
            self.render_log(f, main_area);
        } else {
            let crumb_area = if panel_and_library { Some(status_area) } else { None };
            self.render_library(f, main_area, self.tab_idx - self.lib_tab_offset(), crumb_area);
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

    fn render_volume_bar(&self, f: &mut Frame, area: Rect) {
        let (volume, _volume_max) = if let Some(ref remote) = self.connected_session_state {
            (remote.volume, 100)
        } else {
            let s = self.player.status.lock().unwrap();
            if s.active { (if s.muted { 0 } else { s.volume }, s.volume_max) }
            else { (self.ui_volume as i64, 100) }
        };
        let color = if volume > 100 { palette::RED }
            else if volume > 60 { palette::YELLOW }
            else { palette::PINE };
        let line = Line::from(vec![
            Span::styled(" Vol ", Style::default().fg(Color::Rgb(230, 230, 230))),
            Span::styled(format!("{:>3}%", volume), Style::default().fg(color)),
        ]);
        f.render_widget(Paragraph::new(line), area);
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
        f.render_widget(Block::default().style(Style::default().bg(palette::BASE)), sidebar);
        for row in sidebar.y..sidebar.y + sidebar.height {
            f.render_widget(
                Paragraph::new(Span::styled("\u{2502}", Style::default().fg(palette::OVERLAY))),
                Rect { x: sidebar.x + sidebar.width - 1, y: row, width: 1, height: 1 },
            );
        }
        let inner_w = sidebar.width.saturating_sub(2);
        let ix = sidebar.x + 1;
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
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
        content_width.saturating_sub(2) as usize // indicator char + space
    }

    pub(super) fn render_panel_row(f: &mut Frame, x: u16, y: u16, width: u16, selected: bool, spans: Vec<Span>) {
        let indicator = Span::styled(
            if selected { "\u{258c}" } else { " " },
            Style::default().fg(palette::IRIS),
        );
        let mut all = vec![indicator, Span::raw(" ")];
        all.extend(spans);
        f.render_widget(Paragraph::new(Line::from(all)), Rect { x, y, width, height: 1 });
    }

    fn render_playback_controls(&mut self, f: &mut Frame, area: Rect) {
        if area.height == 0 { return; }

        let inner_w = area.width.min(100);
        let inner_x = area.x + (area.width.saturating_sub(inner_w)) / 2;
        let area = Rect { x: inner_x, y: area.y, width: inner_w, height: area.height };

        let (position_ticks, runtime_ticks, paused) = if let Some(ref remote) = self.connected_session_state {
            let pos_ticks = {
                let elapsed_s = self.remote_pos_at.elapsed().as_secs_f64();
                let pos_s = (self.remote_pos_s as f64 + elapsed_s).min(remote.runtime_s as f64);
                (pos_s * crate::api::TICKS_PER_SECOND as f64) as i64
            };
            (
                pos_ticks,
                remote.runtime_s * crate::api::TICKS_PER_SECOND,
                remote.is_paused,
            )
        } else {
            let s = self.player.status.lock().unwrap();
            (s.position_ticks, s.runtime_ticks, s.paused)
        };

        let pos_s = position_ticks / TICKS_PER_SECOND;
        let dur_s = runtime_ticks / TICKS_PER_SECOND;

        let pos_str = fmt_duration(pos_s);
        let dur_str = fmt_duration(dur_s);

        const BTNS_W: u16 = 30;
        let btn_row_y = area.y + 1;

        self.layout_seekbar_area = Rect { x: area.x, y: area.y, width: area.width, height: 1 };
        self.layout_tracks_area  = Rect::default();
        self.layout_vol_area     = Rect::default();
        self.layout_sub_area     = Rect::default();
        self.layout_audio_area   = Rect::default();

        let ratio = if runtime_ticks > 0 {
            (position_ticks as f64 / runtime_ticks as f64).clamp(0.0, 1.0)
        } else { 0.0 };
        let seek_rect = Rect { x: area.x, y: area.y, width: area.width, height: 1 };
        let bar_w = seek_rect.width as usize;
        let filled = (ratio * bar_w as f64).round() as usize;
        let unfilled = bar_w.saturating_sub(filled);
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::styled("\u{2501}".repeat(filled),   Style::default().fg(palette::IRIS)),
            Span::styled("\u{2500}".repeat(unfilled), Style::default().fg(palette::IRIS_DIM)),
        ])), seek_rect);

        let time_style = Style::default().fg(palette::MUTED);
        let elapsed_w = pos_str.chars().count() as u16;
        let total_w   = dur_str.chars().count() as u16;
        f.render_widget(
            Paragraph::new(Span::styled(pos_str, time_style)),
            Rect { x: area.x, y: btn_row_y, width: elapsed_w.min(area.width), height: 1 },
        );
        let total_x = area.x + area.width.saturating_sub(total_w);
        f.render_widget(
            Paragraph::new(Span::styled(dur_str, time_style)),
            Rect { x: total_x, y: btn_row_y, width: total_w.min(area.width), height: 1 },
        );

        if self.use_nerd_fonts {
            let btn_style = Style::default().fg(Color::Rgb(203, 212, 241));
            let pp_icon = if !paused { "\u{F03E4}" } else { "\u{F040A}" };
            let btn_icons = ["\u{F04AE}", "\u{F04A}", pp_icon, "\u{F04DB}", "\u{F04E}", "\u{F04AD}"];
            let mut btn_spans: Vec<Span> = Vec::new();
            for icon in btn_icons.iter() {
                btn_spans.push(Span::styled(format!("  {icon}  "), btn_style));
            }
            let btn_x = area.x + area.width.saturating_sub(BTNS_W) / 2;
            self.layout_button_area = Rect { x: btn_x, y: btn_row_y, width: BTNS_W, height: 1 };
            f.render_widget(
                Paragraph::new(Line::from(btn_spans)).alignment(Alignment::Center),
                Rect { x: area.x, y: btn_row_y, width: area.width, height: 1 },
            );
        } else {
            self.layout_button_area = Rect::default();
        }
    }
}
