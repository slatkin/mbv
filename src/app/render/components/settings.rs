#[cfg(test)]
use super::super::super::layout::AppLayout;
#[cfg(test)]
use super::super::super::palette;
#[cfg(test)]
use super::super::super::settings::{setting_label, setting_value, settings_total_rows};
use super::super::super::types_overlay::OverlayRequest;
use super::super::super::types_settings::SettingsDestination;
#[cfg(test)]
use super::super::super::types_settings::{ServiceEntry, SERVICE_ENTRIES};
use super::super::super::ui_util::{cycle_lang, next_subtitle_mode};
use super::super::super::App;
use super::super::super::{MultiSelectKind, SettingKey};
#[cfg(test)]
use super::super::super::{SETTINGS_PANEL_W, SETTING_SECTIONS};
#[cfg(test)]
use super::chrome;
#[cfg(test)]
use ratatui::style::{Modifier, Style};
#[cfg(test)]
use ratatui::text::{Line, Span};
#[cfg(test)]
use ratatui::widgets::Paragraph;
#[cfg(test)]
use ratatui::Frame;
use std::time::{Duration, Instant};

impl App {
    pub(crate) fn close_settings(&mut self) {
        if self.settings_save_at.take().is_some() {
            let cfg = self.config.lock().unwrap().clone();
            crate::config::save_config_with_ui(&cfg, &self.ui_config_snapshot());
        }
        self.request_sidebar_dismiss(crate::app::SidebarId::Settings);
        self.settings_destination = SettingsDestination::Main;
    }

    pub(crate) fn handle_settings_activate(&mut self, key: SettingKey) {
        match key {
            SettingKey::Services => {
                self.open_services_settings();
                return;
            }
            SettingKey::HiddenLibraries => {
                self.pending_overlay = Some(OverlayRequest::OpenMultiselect(
                    MultiSelectKind::HiddenLibraries,
                ));
                return;
            }
            SettingKey::HiddenLatest => {
                self.pending_overlay = Some(OverlayRequest::OpenMultiselect(
                    MultiSelectKind::HiddenLatest,
                ));
                return;
            }
            SettingKey::MyLanguages => {
                self.pending_overlay = Some(OverlayRequest::OpenMultiselect(
                    MultiSelectKind::MyLanguages,
                ));
                return;
            }
            SettingKey::FeedViewLibraries => {
                self.pending_overlay = Some(OverlayRequest::OpenMultiselect(
                    MultiSelectKind::FeedViewLibraries,
                ));
                return;
            }
            SettingKey::LibraryRoutes => {
                self.pending_overlay = Some(OverlayRequest::OpenLibraryRoutes);
                return;
            }
            SettingKey::ManageFeeds => {
                self.pending_overlay = Some(OverlayRequest::OpenFeedsManage);
                return;
            }
            SettingKey::LogOut => {
                self.confirm_logout = true;
            }
            SettingKey::ImageProtocol => {
                self.image_protocol = match self.image_protocol.as_deref() {
                    None => Some("halfblocks".into()),
                    Some("halfblocks") => Some("sixel".into()),
                    Some("sixel") => Some("kitty".into()),
                    Some("kitty") => Some("iterm2".into()),
                    Some("iterm2") => Some("auto".into()),
                    _ => None,
                };
                self.image_protocol_enabled = self.image_protocol.is_some();
            }
            SettingKey::SystemNotifications => {
                let new_val = {
                    let mut c = self.config.lock().unwrap();
                    c.system_notifications = !c.system_notifications;
                    c.system_notifications
                };
                self.system_notifications = new_val;
            }
            SettingKey::SubtitleMode => {
                let new_mode = {
                    let mut c = self.config.lock().unwrap();
                    c.subtitle_mode = next_subtitle_mode(&c.subtitle_mode).to_string();
                    c.subtitle_mode.clone()
                };
                self.player.subtitle_prefs.lock().unwrap().mode = new_mode;
                self.push_subtitle_prefs();
            }
            SettingKey::SubtitleLanguage => {
                let new_lang = {
                    let mut c = self.config.lock().unwrap();
                    let new = cycle_lang(&c.my_languages, &c.subtitle_lang);
                    c.subtitle_lang = new.clone();
                    new
                };
                self.player.subtitle_prefs.lock().unwrap().subtitle_lang = new_lang;
                self.push_subtitle_prefs();
            }
            SettingKey::AudioLanguage => {
                let new_lang = {
                    let mut c = self.config.lock().unwrap();
                    let new = cycle_lang(&c.my_languages, &c.audio_lang);
                    c.audio_lang = new.clone();
                    new
                };
                self.player.subtitle_prefs.lock().unwrap().audio_lang = new_lang;
                self.push_subtitle_prefs();
            }
            _ => {
                let mut c = self.config.lock().unwrap();
                match key {
                    SettingKey::StayAlive => c.stay_alive = !c.stay_alive,
                    SettingKey::AutoReconnect => c.auto_reconnect = !c.auto_reconnect,
                    SettingKey::SavePlaylistOnQuit => {
                        c.save_playlist_on_quit = !c.save_playlist_on_quit
                    }
                    SettingKey::AlwaysPlayNext => c.always_play_next = !c.always_play_next,
                    SettingKey::ConsumeVideos => c.consume_videos = !c.consume_videos,
                    SettingKey::ConsumeAudio => c.consume_audio = !c.consume_audio,
                    SettingKey::SavePlaylistOnConsume => {
                        c.save_playlist_on_consume = !c.save_playlist_on_consume
                    }
                    SettingKey::SavePlaylistOnConsumeAudio => {
                        c.save_playlist_on_consume_audio = !c.save_playlist_on_consume_audio
                    }
                    SettingKey::AlwaysSkipIntro => c.always_skip_intro = !c.always_skip_intro,
                    SettingKey::ShowAudioWindow => c.show_audio_window = !c.show_audio_window,
                    SettingKey::UseMpvConfig => c.use_mpv_config = !c.use_mpv_config,
                    SettingKey::NoScripts => c.no_scripts = !c.no_scripts,
                    SettingKey::Autoload => c.autoload = !c.autoload,
                    SettingKey::ShowSysTrayIcon => c.show_systray_icon = !c.show_systray_icon,
                    _ => {}
                }
            }
        }
        if key == SettingKey::AutoReconnect {
            self.persist_roaming_settings();
            if self.config.lock().unwrap().auto_reconnect {
                self.persist_current_auto_reconnect_target();
            }
        }
        self.settings_save_at = Some(Instant::now() + Duration::from_millis(500));
    }

    #[cfg(test)]
    pub(in crate::app::render) fn render_settings_panel(
        &mut self,
        f: &mut Frame,
        layout: &mut AppLayout,
        area: Option<ratatui::layout::Rect>,
    ) {
        if matches!(self.settings_destination, SettingsDestination::Services) {
            self.render_services_panel(f, layout, area);
            return;
        }
        let panel = area.is_some();
        let content = match area {
            Some(area) => {
                chrome::render_panel_shell_at(f, area, "SETTINGS", "[Space]toggle [Esc]close", true)
            }
            None => chrome::render_panel_shell(
                f,
                f.area(),
                SETTINGS_PANEL_W,
                "SETTINGS",
                "[Space]toggle [Esc]close",
            ),
        };
        let content = if panel {
            content
        } else {
            ratatui::layout::Rect {
                x: content.x.saturating_add(2),
                y: content.y.saturating_add(1),
                width: content.width.saturating_sub(4),
                height: content.height.saturating_sub(2),
            }
        };

        let cfg = self.config.lock().unwrap().clone();
        let ui = self.ui_config_snapshot();

        let cursor = self.settings_cursor;
        let confirm_logout = self.confirm_logout;

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
                        .fg(palette::TEXT_METADATA)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            for &key in *keys {
                line_of_cursor.push(lines.len());
                if item_idx == cursor {
                    cursor_line = lines.len();
                }
                let focused = item_idx == cursor;
                let label = setting_label(key);
                let val = setting_value(key, &cfg, &ui);
                let label_style = if focused {
                    Style::default().fg(palette::TEXT_PRIMARY)
                } else {
                    Style::default().fg(palette::PLAYBACK_META_FG)
                };
                let val_w = (content.width as usize).saturating_sub(label.len());
                lines.push(Line::from(vec![
                    Span::styled(label, label_style),
                    Span::styled(
                        format!("{:>w$}", val, w = val_w),
                        Style::default().fg(palette::ACCENT),
                    ),
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
        let (logout_text, logout_style) = if confirm_logout && focused {
            (
                "Log out? Press y to confirm",
                Style::default().fg(palette::STATUS_ERROR),
            )
        } else if focused {
            ("Log out", Style::default().fg(palette::STATUS_ERROR))
        } else {
            ("Log out", Style::default().fg(palette::PLAYBACK_META_FG))
        };
        lines.push(Line::from(Span::styled(logout_text, logout_style)));

        let visible = content.height as usize;
        if cursor_line < self.settings_scroll {
            self.settings_scroll = cursor_line;
        } else if cursor_line >= self.settings_scroll + visible {
            self.settings_scroll = cursor_line + 1 - visible;
        }
        let total = lines.len();
        self.settings_scroll = self.settings_scroll.min(total.saturating_sub(visible));

        f.render_widget(
            Paragraph::new(lines).scroll((self.settings_scroll as u16, 0)),
            content,
        );
        chrome::render_sidebar_scrollbar(f, content, total, self.settings_scroll);
    }

    #[cfg(test)]
    fn render_services_panel(
        &mut self,
        f: &mut Frame,
        layout: &mut AppLayout,
        area: Option<ratatui::layout::Rect>,
    ) {
        if self.emby_setup_form.is_some() {
            self.render_emby_setup_panel(f, layout, area);
            return;
        }
        if self.audiobookshelf_setup_form.is_some() {
            self.render_audiobookshelf_setup_panel(f, layout, area);
            return;
        }
        let content = match area {
            Some(area) => {
                chrome::render_panel_shell_at(f, area, "SERVICES", "[↵]select [Esc]back", true)
            }
            None => chrome::render_panel_shell(
                f,
                f.area(),
                SETTINGS_PANEL_W,
                "SERVICES",
                "[↵]select [Esc]back",
            ),
        };
        let cursor = self.services_cursor;
        let mut lines = Vec::with_capacity(SERVICE_ENTRIES.len());
        let mut line_of_cursor = Vec::with_capacity(SERVICE_ENTRIES.len());
        for (index, entry) in SERVICE_ENTRIES.iter().copied().enumerate() {
            line_of_cursor.push(lines.len());
            let focused = index == cursor;
            let marker = if focused { "▸ " } else { "  " };
            let name = Self::service_entry_name(entry);
            let state = self.service_state_label(entry);
            let context = self.service_context(entry);
            let action = self.service_action_label(entry);
            let detail = if context.is_empty() {
                format!("{state} · {action}")
            } else {
                format!("{state} · {context} · {action}")
            };
            let name_style = if focused {
                Style::default()
                    .fg(palette::TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT_SECONDARY)
            };
            lines.push(Line::from(vec![
                Span::raw(marker),
                Span::styled(name, name_style),
                Span::raw("  "),
                Span::styled(
                    detail,
                    Style::default().fg(if entry == ServiceEntry::Audiobookshelf {
                        palette::TEXT_MUTED
                    } else {
                        palette::ACCENT
                    }),
                ),
            ]));
        }
        f.render_widget(Paragraph::new(lines), content);
    }

    #[cfg(test)]
    fn render_emby_setup_panel(
        &mut self,
        f: &mut Frame,
        _layout: &mut AppLayout,
        area: Option<ratatui::layout::Rect>,
    ) {
        let content = match area {
            Some(area) => {
                chrome::render_panel_shell_at(f, area, "EMBY SETUP", "[↵]submit [Esc]back", true)
            }
            None => chrome::render_panel_shell(
                f,
                f.area(),
                SETTINGS_PANEL_W,
                "EMBY SETUP",
                "[↵]submit [Esc]back",
            ),
        };
        let Some(form) = self.emby_setup_form.as_ref() else {
            return;
        };
        let labels = ["Server URL", "Username", "Password"];
        let mut lines = Vec::with_capacity(8);
        for (idx, label) in labels.iter().enumerate() {
            let focused = form.focus == idx;
            lines.push(Line::from(Span::styled(
                *label,
                Style::default()
                    .fg(if focused {
                        palette::TEXT_METADATA
                    } else {
                        palette::TEXT_SECONDARY
                    })
                    .add_modifier(if focused {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )));
            let value = if idx == 2 {
                "•".repeat(form.fields[idx].chars().count())
            } else {
                form.fields[idx].clone()
            };
            let cursor = if focused && !form.busy { "▏" } else { "" };
            lines.push(Line::from(Span::styled(
                format!("  {value}{cursor}"),
                Style::default().fg(palette::TEXT_PRIMARY),
            )));
        }
        lines.push(Line::from(""));
        let status = if form.busy {
            "Working…"
        } else {
            form.error.as_str()
        };
        lines.push(Line::from(Span::styled(
            status,
            Style::default().fg(if form.busy {
                palette::TEXT_MUTED
            } else {
                palette::STATUS_ERROR
            }),
        )));
        f.render_widget(Paragraph::new(lines), content);
    }

    #[cfg(test)]
    fn render_audiobookshelf_setup_panel(
        &mut self,
        f: &mut Frame,
        _layout: &mut AppLayout,
        area: Option<ratatui::layout::Rect>,
    ) {
        let content = match area {
            Some(area) => chrome::render_panel_shell_at(
                f,
                area,
                "AUDIOBOOKSHELF SETUP",
                "[↵]submit [Esc]back",
                true,
            ),
            None => chrome::render_panel_shell(
                f,
                f.area(),
                SETTINGS_PANEL_W,
                "AUDIOBOOKSHELF SETUP",
                "[↵]submit [Esc]back",
            ),
        };
        let Some(form) = self.audiobookshelf_setup_form.as_ref() else {
            return;
        };
        let labels = ["Server URL", "API key"];
        let mut lines = Vec::with_capacity(6);
        for (idx, label) in labels.iter().enumerate() {
            let focused = form.focus == idx;
            lines.push(Line::from(Span::styled(
                *label,
                Style::default()
                    .fg(if focused {
                        palette::TEXT_METADATA
                    } else {
                        palette::TEXT_SECONDARY
                    })
                    .add_modifier(if focused {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )));
            let value = if idx == 1 {
                "•".repeat(form.fields[idx].chars().count())
            } else {
                form.fields[idx].clone()
            };
            lines.push(Line::from(Span::styled(
                format!("  {value}{}", if focused && !form.busy { "▏" } else { "" }),
                Style::default().fg(palette::TEXT_PRIMARY),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            if form.busy {
                "Working…"
            } else {
                form.error.as_str()
            },
            Style::default().fg(if form.busy {
                palette::TEXT_MUTED
            } else {
                palette::STATUS_ERROR
            }),
        )));
        f.render_widget(Paragraph::new(lines), content);
    }
}
