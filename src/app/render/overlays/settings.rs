use super::super::super::layout::AppLayout;
use super::super::super::palette;
use super::super::super::settings::{
    setting_label, setting_value, settings_cursor_to_key, settings_total_rows,
};
use super::super::super::ui_util::{cycle_lang, next_subtitle_mode};
use super::super::super::App;
use super::super::super::{MultiSelectKind, SettingKey, SETTINGS_PANEL_W, SETTING_SECTIONS};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::time::{Duration, Instant};

impl App {
    pub(crate) fn close_settings(&mut self) {
        if self.settings_save_at.take().is_some() {
            let cfg = self.client.lock().unwrap().config.clone();
            crate::config::save_config_settings(&cfg);
        }
        self.show_settings = false;
    }

    pub(crate) fn handle_settings_activate(&mut self) {
        let key = settings_cursor_to_key(self.settings_cursor);
        match key {
            SettingKey::HiddenLibraries => {
                self.open_multiselect_popup(MultiSelectKind::HiddenLibraries);
                return;
            }
            SettingKey::HiddenLatest => {
                self.open_multiselect_popup(MultiSelectKind::HiddenLatest);
                return;
            }
            SettingKey::MyLanguages => {
                self.open_multiselect_popup(MultiSelectKind::MyLanguages);
                return;
            }
            SettingKey::FeedViewLibraries => {
                self.open_multiselect_popup(MultiSelectKind::FeedViewLibraries);
                return;
            }
            SettingKey::LogOut => {
                self.confirm_logout = true;
            }
            SettingKey::ImageProtocol => {
                let now_none = {
                    let mut c = self.client.lock().unwrap();
                    c.config.image_protocol = match c.config.image_protocol.as_deref() {
                        None => Some("halfblocks".into()),
                        Some("halfblocks") => Some("sixel".into()),
                        Some("sixel") => Some("kitty".into()),
                        Some("kitty") => Some("iterm2".into()),
                        Some("iterm2") => Some("auto".into()),
                        _ => None,
                    };
                    c.config.image_protocol.is_none()
                };
                self.image_protocol_enabled = !now_none;
                if now_none {
                    self.home_card_view = false;
                    self.queue_view = 0;
                    self.save_prefs();
                }
            }
            SettingKey::ShowLogTab => {
                let new_val = {
                    let mut c = self.client.lock().unwrap();
                    c.config.show_log_tab = !c.config.show_log_tab;
                    c.config.show_log_tab
                };
                self.show_log_tab = new_val;
            }
            SettingKey::SystemNotifications => {
                let new_val = {
                    let mut c = self.client.lock().unwrap();
                    c.config.system_notifications = !c.config.system_notifications;
                    c.config.system_notifications
                };
                self.system_notifications = new_val;
            }
            SettingKey::SubtitleMode => {
                let new_mode = {
                    let mut c = self.client.lock().unwrap();
                    c.config.subtitle_mode =
                        next_subtitle_mode(&c.config.subtitle_mode).to_string();
                    c.config.subtitle_mode.clone()
                };
                self.player.subtitle_prefs.lock().unwrap().mode = new_mode;
                self.push_subtitle_prefs();
            }
            SettingKey::SubtitleLanguage => {
                let new_lang = {
                    let mut c = self.client.lock().unwrap();
                    let new = cycle_lang(&c.config.my_languages, &c.config.subtitle_lang);
                    c.config.subtitle_lang = new.clone();
                    new
                };
                self.player.subtitle_prefs.lock().unwrap().subtitle_lang = new_lang;
                self.push_subtitle_prefs();
            }
            SettingKey::AudioLanguage => {
                let new_lang = {
                    let mut c = self.client.lock().unwrap();
                    let new = cycle_lang(&c.config.my_languages, &c.config.audio_lang);
                    c.config.audio_lang = new.clone();
                    new
                };
                self.player.subtitle_prefs.lock().unwrap().audio_lang = new_lang;
                self.push_subtitle_prefs();
            }
            _ => {
                let mut c = self.client.lock().unwrap();
                match key {
                    SettingKey::DaemonModeOnExit => {
                        c.config.daemon_mode_on_exit = !c.config.daemon_mode_on_exit
                    }
                    SettingKey::StartOnQueue => c.config.start_on_queue = !c.config.start_on_queue,
                    SettingKey::AlwaysPlayNext => {
                        c.config.always_play_next = !c.config.always_play_next
                    }
                    SettingKey::ConsumeVideos => c.config.consume_videos = !c.config.consume_videos,
                    SettingKey::ConsumeAudio => c.config.consume_audio = !c.config.consume_audio,
                    SettingKey::SavePlaylistOnConsume => {
                        c.config.save_playlist_on_consume = !c.config.save_playlist_on_consume
                    }
                    SettingKey::SavePlaylistOnConsumeAudio => {
                        c.config.save_playlist_on_consume_audio =
                            !c.config.save_playlist_on_consume_audio
                    }
                    SettingKey::AlwaysSkipIntro => {
                        c.config.always_skip_intro = !c.config.always_skip_intro
                    }
                    SettingKey::ShowAudioWindow => {
                        c.config.show_audio_window = !c.config.show_audio_window
                    }
                    SettingKey::UseMpvConfig => c.config.use_mpv_config = !c.config.use_mpv_config,
                    SettingKey::NoScripts => c.config.no_scripts = !c.config.no_scripts,
                    SettingKey::Autoload => c.config.autoload = !c.config.autoload,
                    SettingKey::ShowSysTrayIcon => {
                        c.config.show_systray_icon = !c.config.show_systray_icon
                    }
                    _ => {}
                }
            }
        }
        self.settings_save_at = Some(Instant::now() + Duration::from_millis(500));
    }

    pub(in crate::app::render) fn render_settings_panel(
        &mut self,
        f: &mut Frame,
        layout: &mut AppLayout,
    ) {
        let content = Self::render_panel_shell(
            f,
            f.area(),
            SETTINGS_PANEL_W,
            "SETTINGS",
            "[Space]toggle [Esc]close",
        );
        let cfg = self.client.lock().unwrap().config.clone();

        let cursor = self.settings_cursor;
        let confirm_logout = self.confirm_logout;
        let label_w = 28usize;

        let data_sections = &SETTING_SECTIONS[..SETTING_SECTIONS.len() - 1];

        let mut lines: Vec<Line> = vec![];
        let mut cursor_line = 0usize;
        let mut item_idx = 0usize;
        let mut line_of_cursor: Vec<usize> = Vec::new();

        for (sec_name, keys) in data_sections {
            lines.push(Line::from(vec![
                Span::raw(""),
                Span::styled(
                    (*sec_name).to_owned(),
                    Style::default()
                        .fg(palette::IRIS)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            for &key in *keys {
                line_of_cursor.push(lines.len());
                if item_idx == cursor {
                    cursor_line = lines.len();
                }
                let focused = item_idx == cursor;
                let indicator = if focused { "▌" } else { " " };
                let label = setting_label(key);
                let val = setting_value(key, &cfg);
                let label_style = if focused {
                    Style::default().fg(palette::TEXT)
                } else {
                    Style::default().fg(palette::MUTED)
                };
                lines.push(Line::from(vec![
                    Span::styled(indicator, Style::default().fg(palette::PINE)),
                    Span::raw(" "),
                    Span::styled(format!("{:<lw$}", label, lw = label_w), label_style),
                    Span::styled(val, Style::default().fg(palette::FOAM)),
                ]));
                item_idx += 1;
            }
            lines.push(Line::from(""));
        }

        let logout_cursor_idx = settings_total_rows() - 1;
        line_of_cursor.push(lines.len());
        if cursor == logout_cursor_idx {
            cursor_line = lines.len();
        }
        let focused = cursor == logout_cursor_idx;
        let indicator_color = if focused { palette::RED } else { palette::PINE };
        let (logout_text, logout_style) = if confirm_logout && focused {
            (
                "Log out? Press y to confirm",
                Style::default().fg(palette::RED),
            )
        } else if focused {
            ("Log out", Style::default().fg(palette::RED))
        } else {
            ("Log out", Style::default().fg(palette::MUTED))
        };
        lines.push(Line::from(vec![
            Span::styled(
                if focused { "▌" } else { " " },
                Style::default().fg(indicator_color),
            ),
            Span::raw(" "),
            Span::styled(logout_text, logout_style),
        ]));

        let visible = content.height as usize;
        if cursor_line < self.settings_scroll {
            self.settings_scroll = cursor_line;
        } else if cursor_line >= self.settings_scroll + visible {
            self.settings_scroll = cursor_line + 1 - visible;
        }
        let total = lines.len();
        self.settings_scroll = self.settings_scroll.min(total.saturating_sub(visible));
        layout.settings_line_of_cursor = line_of_cursor;

        f.render_widget(
            Paragraph::new(lines).scroll((self.settings_scroll as u16, 0)),
            content,
        );
        Self::render_sidebar_scrollbar(f, content, total, self.settings_scroll);
    }
}
