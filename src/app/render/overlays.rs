use std::time::{Duration, Instant};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph};
use super::super::App;
use super::super::palette;
use super::super::ui_util::{fmt_duration, trunc_str};
use super::super::settings::{setting_label, setting_value, settings_cursor_to_key, settings_total_rows};
use super::super::{
    MultiSelectKind, MultiSelectPopup, PendingQueueAction, SavePlaylistStage, SettingKey,
    SETTING_SECTIONS, SESSIONS_PANEL_W, PLAYLISTS_PANEL_W, HELP_PANEL_W, SETTINGS_PANEL_W,
};

impl App {
    pub(super) fn render_sessions_overlay(&self, f: &mut Frame) {
        let content = Self::render_panel_shell(
            f, f.area(), SESSIONS_PANEL_W,
            "Remote Sessions",
            "[↑↓]select [↵]connect [d]disc [r]refresh [Esc]close",
        );
        let ix = content.x;
        let inner_w = content.width;
        let iw = inner_w as usize;
        let list_y = content.y;
        let list_h = content.height;
        let list_area = content;

        if self.sessions_loading && self.sessions.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(" Loading\u{2026}", Style::default().fg(palette::SUBTLE))),
                list_area,
            );
            return;
        }
        if self.sessions.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(" No other active sessions", Style::default().fg(palette::SUBTLE))),
                list_area,
            );
            return;
        }

        const CARD_H: u16 = 3;
        const DIV_H:  u16 = 1;
        let entry_h = CARD_H + DIV_H;

        for (i, s) in self.sessions.iter().enumerate() {
            let entry_y = list_y + i as u16 * entry_h;
            if entry_y + CARD_H > list_y + list_h { break; }

            let selected = i == self.sessions_cursor;
            let is_connected = self.connected_session_id.as_deref() == Some(s.id.as_str());
            let name_color = if selected { palette::IRIS } else { palette::TEXT };
            let dim = Style::default().fg(palette::MUTED);

            // Indicator spans all 3 rows of the card.
            if selected {
                let bar: Vec<Line> = (0..CARD_H)
                    .map(|_| Line::from(Span::styled("\u{258c}", Style::default().fg(palette::IRIS))))
                    .collect();
                f.render_widget(Paragraph::new(bar), Rect { x: ix, y: entry_y, width: 1, height: CARD_H });
            }
            let text_x = ix + 2; // indicator + space
            let text_w = inner_w.saturating_sub(2) as usize;

            let badge = if is_connected { " \u{271A}" } else { "" };
            let name_max = text_w.saturating_sub(badge.len());
            let name_line = Line::from(vec![
                Span::styled(trunc_str(&s.device_name, name_max), Style::default().fg(name_color).add_modifier(Modifier::BOLD)),
                Span::styled(badge, Style::default().fg(palette::IRIS)),
            ]);
            f.render_widget(Paragraph::new(name_line), Rect { x: text_x, y: entry_y, width: inner_w.saturating_sub(2), height: 1 });

            let meta = format!("{} \u{b7} {}@{}", s.client, s.user_name, s.host);
            f.render_widget(
                Paragraph::new(Span::styled(trunc_str(&meta, text_w), dim.fg(palette::SUBTLE))),
                Rect { x: text_x, y: entry_y + 1, width: inner_w.saturating_sub(2), height: 1 },
            );

            let state_icon = if s.now_playing.is_some() {
                if s.is_paused { "\u{23f8}" } else { "\u{25b6}" }
            } else { "\u{25a0}" };
            let time = if s.now_playing.is_some() {
                format!(" {}/{}", fmt_duration(s.position_s), fmt_duration(s.runtime_s))
            } else { String::new() };
            let title = s.now_playing.as_deref().unwrap_or("idle");
            let playing = format!("{} {}{}", state_icon, trunc_str(title, text_w.saturating_sub(11)), time);
            f.render_widget(
                Paragraph::new(Span::styled(trunc_str(&playing, text_w), dim)),
                Rect { x: text_x, y: entry_y + 2, width: inner_w.saturating_sub(2), height: 1 },
            );

            if entry_y + entry_h <= list_y + list_h {
                f.render_widget(
                    Paragraph::new(Span::styled("\u{2500}".repeat(iw), Style::default().fg(palette::OVERLAY))),
                    Rect { x: ix, y: entry_y + CARD_H, width: inner_w, height: 1 },
                );
            }
        }
    }

    pub(super) fn render_playlists_panel(&mut self, f: &mut Frame) {
        let (title, hint) = if self.playlists_open.is_some() {
            let name = self.playlists_open.as_ref().map(|p| p.name.as_str()).unwrap_or("Playlist");
            (name.to_string(), "[↑↓]select [↵]play [←]back [Esc]close".to_string())
        } else {
            ("Playlists".to_string(), "[↑↓]select [↵]play [→]browse [r]refresh [Esc]close".to_string())
        };

        let content = Self::render_panel_shell(f, f.area(), PLAYLISTS_PANEL_W, &title, &hint);
        let ix = content.x;
        let iw = content.width as usize;
        let list_h = content.height as usize;

        if self.playlists_open.is_some() {
            if self.playlists_open_loading && self.playlists_open_items.is_empty() {
                f.render_widget(
                    Paragraph::new(Span::styled(" Loading\u{2026}", Style::default().fg(palette::SUBTLE))),
                    content,
                );
                return;
            }
            if self.playlists_open_items.is_empty() {
                f.render_widget(
                    Paragraph::new(Span::styled(" Playlist is empty", Style::default().fg(palette::SUBTLE))),
                    content,
                );
                return;
            }

            let item_lines = |label: &str| -> usize {
                let text_w = iw.saturating_sub(6);
                if label.len() <= text_w { 1 } else { 2 }
            };

            while self.playlists_open_scroll > self.playlists_open_cursor {
                self.playlists_open_scroll = self.playlists_open_cursor;
            }
            loop {
                if self.playlists_open_scroll >= self.playlists_open_cursor { break; }
                let lines_to_cursor: usize = self.playlists_open_items
                    [self.playlists_open_scroll..=self.playlists_open_cursor]
                    .iter().map(|i| item_lines(&i.display_name())).sum();
                if lines_to_cursor <= list_h { break; }
                self.playlists_open_scroll += 1;
            }

            let mut y = 0usize;
            for (vi, item) in self.playlists_open_items[self.playlists_open_scroll..].iter().enumerate() {
                if y >= list_h { break; }
                let abs_idx = self.playlists_open_scroll + vi;
                let selected = abs_idx == self.playlists_open_cursor;
                let fg = if selected { palette::IRIS } else { palette::TEXT };
                let num_str = format!("{:>2}. ", abs_idx + 1);
                let text_w = Self::panel_row_text_width(content.width).saturating_sub(num_str.len());
                let indent = " ".repeat(2 + num_str.len()); // indicator + space + num
                let label = item.display_name();
                let (line1, line2) = if label.len() <= text_w {
                    (label, String::new())
                } else {
                    let wrap_at = label[..text_w].rfind(' ').unwrap_or(text_w);
                    (label[..wrap_at].to_string(), label[wrap_at..].trim_start().to_string())
                };
                let row_y = content.y + y as u16;
                Self::render_panel_row(f, ix, row_y, content.width, selected, vec![
                    Span::styled(num_str, Style::default().fg(palette::MUTED)),
                    Span::styled(line1, Style::default().fg(fg)),
                ]);
                y += 1;
                if !line2.is_empty() && y < list_h {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(&indent),
                            Span::styled(trunc_str(&line2, text_w), Style::default().fg(palette::SUBTLE)),
                        ])),
                        Rect { x: ix, y: row_y + 1, width: content.width, height: 1 });
                    y += 1;
                }
            }
            return;
        }

        if self.playlists_loading && self.playlists.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(" Loading\u{2026}", Style::default().fg(palette::SUBTLE))),
                content,
            );
            return;
        }
        if self.playlists.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(" No playlists found", Style::default().fg(palette::SUBTLE))),
                content,
            );
            return;
        }

        if self.playlists_cursor < self.playlists_scroll {
            self.playlists_scroll = self.playlists_cursor;
        } else if self.playlists_cursor >= self.playlists_scroll + list_h {
            self.playlists_scroll = self.playlists_cursor + 1 - list_h;
        }

        for (vi, pl) in self.playlists[self.playlists_scroll..].iter().enumerate() {
            if vi >= list_h { break; }
            let abs_idx = self.playlists_scroll + vi;
            let selected = abs_idx == self.playlists_cursor;
            let fg = if selected { palette::IRIS } else { palette::TEXT };
            let count_str = if pl.total_count > 0 { format!(" ({})", pl.total_count) } else { String::new() };
            let name_max = Self::panel_row_text_width(content.width).saturating_sub(count_str.len());
            let row_y = content.y + vi as u16;
            Self::render_panel_row(f, ix, row_y, content.width, selected, vec![
                Span::styled(trunc_str(&pl.name, name_max), Style::default().fg(fg).add_modifier(Modifier::BOLD)),
                Span::styled(count_str, Style::default().fg(palette::MUTED)),
            ]);
        }
    }

    pub(super) fn render_help_panel(&mut self, f: &mut Frame) {
        let content = Self::render_panel_shell(
            f, f.area(), HELP_PANEL_W,
            "Keyboard Shortcuts",
            "[↑↓]scroll [Esc]close",
        );
        let w = content.width as usize;
        let key_w = 20usize;

        let mk = |key: &str, desc: &str| -> Line<'static> {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{:<kw$}", key, kw = key_w),
                    Style::default().fg(palette::TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc.to_owned(), Style::default().fg(palette::SUBTLE)),
            ])
        };
        let section = |label: &str| -> Line<'static> {
            let dash_count = w.saturating_sub(2 + label.len() + 1);
            Line::from(vec![
                Span::raw("  "),
                Span::styled(label.to_owned(), Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" {}", "─".repeat(dash_count)),
                    Style::default().fg(palette::OVERLAY),
                ),
            ])
        };
        let blank = || Line::from("");

        let show_log = self.show_log_tab;

        let sec_global = vec![
            blank(),
            section("GLOBAL"),
            mk("F1",               "Help"),
            mk("F2",               "Settings"),
            mk("F3",               "Remote sessions"),
            mk("F4",               "Playlists"),
            mk("F5",               "Refresh current view"),
            mk("Tab",              "Cycle menu"),
            mk("1 – 9",            "Jump to tab"),
            mk("↑ / ↓",            "Move cursor"),
            mk("PgUp / PgDn",      "Page scroll"),
            mk("Home / End",       "First / last"),
            mk("Enter",            "Select / Play / Open"),
            mk("o",                "Context menu"),
            mk("c",                "Clear Queue (confirms)"),
            mk("q",                "Quit"),
        ];
        let sec_playback = vec![
            blank(),
            section("PLAYBACK"),
            mk("Space",            "Pause / Resume"),
            mk("< / >",            "Seek ±5 seconds"),
            mk("Alt+Enter",        "Stop"),
            mk("- / +",            "Volume down / up"),
            mk("a",                "Cycle audio track"),
            mk("z",                "Enable subtitles"),
            mk("h",                "Hide / show playback panel"),
        ];
        let sec_queue = vec![
            blank(),
            section("QUEUE"),
            mk(".",                "Jump to playing item"),
            mk("i",                "Go to item in library"),
            mk("Del",              "Remove from Queue"),
            mk("v",                "Toggle view"),
        ];
        let sec_home = vec![
            blank(),
            section("HOME"),
            mk("Alt+↑ / ↓",        "Switch sections"),
            mk("Ctrl+W",           "Toggle watched"),
            mk("Ctrl+Q",           "Add to Queue"),
        ];
        let sec_library = vec![
            blank(),
            section("LIBRARY"),
            mk("Esc / Backspace",  "Go back"),
            mk("/",                "Search library"),
            mk("Ctrl+W",           "Toggle watched"),
            mk("Ctrl+S",           "Shuffle and play selection"),
            mk("Ctrl+P",           "Play all (recursive)"),
            mk("Ctrl+Q",           "Add to Queue"),
        ];
        let sec_log = if show_log {
            vec![
                blank(),
                section("LOG"),
                mk("Alt+L",            "Open Log"),
                mk("← / →",            "Switch pane (Sources / Log)"),
                mk("↑ / ↓",            "Scroll log / navigate sources"),
                mk("PgUp / PgDn",      "Page scroll"),
                mk("Space",            "Toggle source on/off"),
                mk("c",                "Copy log to clipboard"),
            ]
        } else {
            vec![]
        };

        let is_log = show_log && self.tab_idx == self.log_tab_idx();
        let is_lib = self.tab_idx >= self.lib_tab_offset() && self.tab_idx < self.log_tab_idx();
        let is_queue = self.tab_idx == 1;
        let is_home = self.tab_idx == 0;

        let mut ordered: Vec<Vec<Line>> = Vec::new();
        if is_home {
            ordered.push(sec_home); ordered.push(sec_global); ordered.push(sec_playback);
            ordered.push(sec_queue); ordered.push(sec_library); ordered.push(sec_log);
        } else if is_queue {
            ordered.push(sec_queue); ordered.push(sec_global); ordered.push(sec_playback);
            ordered.push(sec_home); ordered.push(sec_library); ordered.push(sec_log);
        } else if is_lib {
            ordered.push(sec_library); ordered.push(sec_global); ordered.push(sec_playback);
            ordered.push(sec_queue); ordered.push(sec_home); ordered.push(sec_log);
        } else if is_log {
            ordered.push(sec_log); ordered.push(sec_global); ordered.push(sec_playback);
            ordered.push(sec_queue); ordered.push(sec_home); ordered.push(sec_library);
        } else {
            ordered.push(sec_global); ordered.push(sec_playback); ordered.push(sec_queue);
            ordered.push(sec_home); ordered.push(sec_library); ordered.push(sec_log);
        }

        let mut lines: Vec<Line> = ordered.into_iter().flatten().collect();
        lines.push(blank());

        let total = lines.len();
        let visible = content.height as usize;
        self.help_scroll = self.help_scroll.min(total.saturating_sub(visible) as u16);
        f.render_widget(Paragraph::new(lines).scroll((self.help_scroll, 0)), content);
    }

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
            SettingKey::HiddenLibraries => { self.open_multiselect_popup(MultiSelectKind::HiddenLibraries); return; }
            SettingKey::HiddenLatest    => { self.open_multiselect_popup(MultiSelectKind::HiddenLatest);    return; }
            SettingKey::MyLanguages     => { self.open_multiselect_popup(MultiSelectKind::MyLanguages);     return; }
            SettingKey::LogOut => { self.confirm_logout = true; }
            SettingKey::ImageProtocol => {
                let now_none = {
                    let mut c = self.client.lock().unwrap();
                    c.config.image_protocol = match c.config.image_protocol.as_deref() {
                        None               => Some("halfblocks".into()),
                        Some("halfblocks") => Some("sixel".into()),
                        Some("sixel")      => Some("kitty".into()),
                        Some("kitty")      => Some("iterm2".into()),
                        Some("iterm2")     => Some("auto".into()),
                        _                  => None,
                    };
                    c.config.image_protocol.is_none()
                };
                self.image_protocol_enabled = !now_none;
                if now_none {
                    self.home_card_view = false;
                    self.playlist_view = 0;
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
                // Client-only: update local config (saved to config.toml) + player Arc only.
                // Never pushed to the Emby server.
                let new_mode = {
                    let mut c = self.client.lock().unwrap();
                    c.config.subtitle_mode = super::super::ui_util::next_subtitle_mode(&c.config.subtitle_mode).to_string();
                    c.config.subtitle_mode.clone()
                };
                self.player.subtitle_prefs.lock().unwrap().mode = new_mode;
                self.push_subtitle_prefs();
            }
            SettingKey::SubtitleLanguage => {
                let new_lang = {
                    let mut c = self.client.lock().unwrap();
                    let new = super::super::ui_util::cycle_lang(&c.config.my_languages, &c.config.subtitle_lang);
                    c.config.subtitle_lang = new.clone();
                    new
                };
                self.player.subtitle_prefs.lock().unwrap().subtitle_lang = new_lang;
                self.push_subtitle_prefs();
            }
            SettingKey::AudioLanguage => {
                let new_lang = {
                    let mut c = self.client.lock().unwrap();
                    let new = super::super::ui_util::cycle_lang(&c.config.my_languages, &c.config.audio_lang);
                    c.config.audio_lang = new.clone();
                    new
                };
                self.player.subtitle_prefs.lock().unwrap().audio_lang = new_lang;
                self.push_subtitle_prefs();
            }
            _ => {
                let mut c = self.client.lock().unwrap();
                match key {
                    SettingKey::DaemonModeOnExit => c.config.daemon_mode_on_exit = !c.config.daemon_mode_on_exit,
                    SettingKey::StartOnQueue     => c.config.start_on_queue = !c.config.start_on_queue,
                    SettingKey::AlwaysPlayNext   => c.config.always_play_next = !c.config.always_play_next,
                    SettingKey::ConsumeVideos    => c.config.consume_videos = !c.config.consume_videos,
                    SettingKey::SavePlaylistOnConsume => c.config.save_playlist_on_consume = !c.config.save_playlist_on_consume,
                    SettingKey::AlwaysSkipIntro  => c.config.always_skip_intro = !c.config.always_skip_intro,
                    SettingKey::ShowAudioWindow  => c.config.show_audio_window = !c.config.show_audio_window,
                    SettingKey::UseMpvConfig     => c.config.use_mpv_config = !c.config.use_mpv_config,
                    SettingKey::NoScripts        => c.config.no_scripts = !c.config.no_scripts,
                    SettingKey::Autoload         => c.config.autoload = !c.config.autoload,
                    SettingKey::ShowSysTrayIcon  => c.config.show_systray_icon = !c.config.show_systray_icon,
                    _ => {}
                }
            }
        }
        self.settings_save_at = Some(Instant::now() + Duration::from_millis(500));
    }

    pub(crate) fn open_multiselect_popup(&mut self, kind: MultiSelectKind) {
        if matches!(kind, MultiSelectKind::MyLanguages) {
            const ALL_LANGS: &[&str] = &[
                "English", "French", "German", "Spanish", "Italian", "Portuguese",
                "Japanese", "Korean", "Chinese", "Russian", "Arabic", "Dutch",
                "Swedish", "Norwegian", "Danish", "Finnish", "Polish", "Czech", "Turkish",
            ];
            let my_langs = self.client.lock().unwrap().config.my_languages.clone();
            let items = ALL_LANGS.iter().map(|&name| {
                let selected = my_langs.iter().any(|l| l == name);
                (name.to_lowercase(), name.to_string(), selected)
            }).collect();
            self.multiselect_popup = Some(MultiSelectPopup { kind, items, cursor: 0 });
            return;
        }
        let client = self.client.lock().unwrap();
        let all = match kind {
            MultiSelectKind::HiddenLibraries => client.get_views().unwrap_or_default(),
            MultiSelectKind::HiddenLatest    => client.get_user_views().unwrap_or_default(),
            MultiSelectKind::MyLanguages     => unreachable!(),
        };
        let hidden_list = match kind {
            MultiSelectKind::HiddenLibraries => &client.config.hidden_libraries,
            MultiSelectKind::HiddenLatest    => &client.config.hidden_latest,
            MultiSelectKind::MyLanguages     => unreachable!(),
        };
        let items: Vec<(String, String, bool)> = all.iter()
            .filter(|v| v.collection_type != "playlists")
            .map(|v| {
                let lower = v.name.to_lowercase();
                let is_hidden = hidden_list.contains(&lower);
                (lower, v.name.clone(), is_hidden)
            }).collect();
        drop(client);
        self.multiselect_popup = Some(MultiSelectPopup { kind, items, cursor: 0 });
    }

    pub(crate) fn close_multiselect_popup(&mut self) {
        let Some(popup) = self.multiselect_popup.take() else { return; };

        if matches!(popup.kind, MultiSelectKind::MyLanguages) {
            let selected: Vec<String> = popup.items.iter()
                .filter(|(_, _, is_sel)| *is_sel)
                .map(|(_, name, _)| name.clone())
                .collect();
            {
                let mut c = self.client.lock().unwrap();
                // If user removed a language that was chosen for subtitle/audio, clear it to "any"
                if !selected.is_empty() {
                    if !c.config.subtitle_lang.is_empty() && !selected.contains(&c.config.subtitle_lang) {
                        c.config.subtitle_lang = String::new();
                    }
                    if !c.config.audio_lang.is_empty() && !selected.contains(&c.config.audio_lang) {
                        c.config.audio_lang = String::new();
                    }
                }
                c.config.my_languages = selected;
            }
            let cfg = self.client.lock().unwrap().config.clone();
            {
                let mut p = self.player.subtitle_prefs.lock().unwrap();
                p.subtitle_lang = cfg.subtitle_lang.clone();
                p.audio_lang    = cfg.audio_lang.clone();
            }
            crate::config::save_config_settings(&cfg);
            return;
        }

        let hidden: Vec<String> = popup.items.iter()
            .filter(|(_, _, is_hidden)| *is_hidden)
            .map(|(lower, _, _)| lower.clone())
            .collect();
        {
            let mut c = self.client.lock().unwrap();
            match popup.kind {
                MultiSelectKind::HiddenLibraries => c.config.hidden_libraries = hidden.clone(),
                MultiSelectKind::HiddenLatest    => c.config.hidden_latest    = hidden.clone(),
                MultiSelectKind::MyLanguages     => unreachable!(),
            }
        }
        match popup.kind {
            MultiSelectKind::HiddenLibraries => self.hidden_libraries = hidden,
            MultiSelectKind::HiddenLatest    => self.hidden_latest    = hidden,
            MultiSelectKind::MyLanguages     => unreachable!(),
        }
        let cfg = self.client.lock().unwrap().config.clone();
        crate::config::save_config_settings(&cfg);
        let _ = self.fetch_home();
    }

    pub(super) fn render_save_playlist_dialog(&mut self, f: &mut Frame) {
        let Some(ref dialog) = self.save_playlist_dialog else { return; };
        let full = f.area();
        let w: u16 = 52;
        let h: u16 = 7;
        let x = full.x + full.width.saturating_sub(w) / 2;
        let y = full.y + full.height.saturating_sub(h) / 2;
        let rect = Rect { x, y, width: w, height: h };
        f.render_widget(Clear, rect);
        let block = Block::default()
            .title(Span::styled(" Save as Playlist ", Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        match &dialog.stage {
            SavePlaylistStage::EnterName => {
                let label = "Name: ";
                let cursor = "\u{258f}";
                let max_input = inner.width as usize - label.len() - cursor.len() - 2;
                let visible: String = dialog.input.chars().rev().take(max_input).collect::<String>().chars().rev().collect();
                let input_line = format!("{}{}{}", label, visible, cursor);
                let hint = "Enter to save · Esc to cancel";
                let input_y = inner.y + (inner.height.saturating_sub(3)) / 2;
                let hint_y = input_y + 2;
                f.render_widget(
                    Paragraph::new(Span::styled(input_line, Style::default().fg(palette::WHITE))),
                    Rect { x: inner.x + 1, y: input_y, width: inner.width.saturating_sub(2), height: 1 },
                );
                f.render_widget(
                    Paragraph::new(Span::styled(hint, Style::default().fg(palette::SUBTLE))),
                    Rect { x: inner.x + 1, y: hint_y, width: inner.width.saturating_sub(2), height: 1 },
                );
            }
            SavePlaylistStage::ConfirmOverwrite { .. } => {
                let name = trunc_str(&dialog.input, inner.width as usize - 4);
                let line1 = format!("\"{}\" already exists.", name);
                let line2 = "Press y to overwrite · Esc to go back";
                let base_y = inner.y + (inner.height.saturating_sub(3)) / 2;
                f.render_widget(
                    Paragraph::new(Span::styled(line1, Style::default().fg(palette::WHITE))),
                    Rect { x: inner.x + 1, y: base_y, width: inner.width.saturating_sub(2), height: 1 },
                );
                f.render_widget(
                    Paragraph::new(Span::styled(line2, Style::default().fg(palette::SUBTLE))),
                    Rect { x: inner.x + 1, y: base_y + 2, width: inner.width.saturating_sub(2), height: 1 },
                );
            }
        }
    }

    pub(super) fn render_dirty_playlist_modal(&self, f: &mut Frame) {
        let name = trunc_str(self.queue_playlist_name(), 36);
        let is_quit = matches!(self.pending_queue_action, Some(PendingQueueAction::Quit));
        let full = f.area();
        let w: u16 = 56;
        let h: u16 = 7;
        let x = full.x + full.width.saturating_sub(w) / 2;
        let y = full.y + full.height.saturating_sub(h) / 2;
        let rect = Rect { x, y, width: w, height: h };
        f.render_widget(Clear, rect);
        let block = Block::default()
            .title(Span::styled(" Unsaved Playlist Changes ", Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::YELLOW));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let line1 = format!("Save changes to \"{}\"?", name);
        let line2 = if is_quit {
            "[s]Save  [d]Discard & quit  [Esc]Cancel"
        } else {
            "[s]Save  [d]Discard  [Esc]Cancel"
        };
        let base_y = inner.y + (inner.height.saturating_sub(3)) / 2;
        f.render_widget(
            Paragraph::new(Span::styled(line1, Style::default().fg(palette::WHITE))),
            Rect { x: inner.x + 1, y: base_y, width: inner.width.saturating_sub(2), height: 1 },
        );
        f.render_widget(
            Paragraph::new(Span::styled(line2, Style::default().fg(palette::SUBTLE))),
            Rect { x: inner.x + 1, y: base_y + 2, width: inner.width.saturating_sub(2), height: 1 },
        );
    }

    pub(super) fn render_multiselect_popup(&mut self, f: &mut Frame) {
        let Some(ref popup) = self.multiselect_popup else { return; };
        let title = match popup.kind {
            MultiSelectKind::HiddenLibraries => " Hidden Libraries ",
            MultiSelectKind::HiddenLatest    => " Hidden Latest ",
            MultiSelectKind::MyLanguages     => " My Languages ",
        };
        let max_name = popup.items.iter().map(|(_, n, _)| n.len()).max().unwrap_or(0);
        let inner_w = ((max_name + 6) as u16).clamp(36, 60);
        let width = inner_w + 2;
        let content_h = popup.items.len() as u16 + 1;
        let area = f.area();
        let height = (content_h + 2).min(area.height.saturating_sub(2));
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let rect = Rect { x, y, width, height };

        f.render_widget(Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS))
            .title(Span::styled(title, Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let hint = "Space toggle  \u{b7}  Esc / Enter close";
        f.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(palette::MUTED))),
            Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 },
        );

        let list_area = Rect {
            x: inner.x, y: inner.y + 1,
            width: inner.width, height: inner.height.saturating_sub(1),
        };
        let list_h = list_area.height as usize;
        let cursor = popup.cursor;
        let scroll = if cursor >= list_h { cursor + 1 - list_h } else { 0 };

        let lines: Vec<Line> = popup.items.iter().enumerate()
            .skip(scroll).take(list_h)
            .map(|(i, (_, name, is_hidden))| {
                let focused = i == cursor;
                let arrow = if focused { "\u{25b8} " } else { "  " };
                let check = if *is_hidden { "[x]" } else { "[ ]" };
                let check_style = if focused { Style::default().fg(palette::FOAM) } else { Style::default().fg(palette::MUTED) };
                let name_style = if focused { Style::default().fg(palette::TEXT) } else { Style::default().fg(palette::SUBTLE) };
                Line::from(vec![
                    Span::raw(arrow),
                    Span::styled(check, check_style),
                    Span::raw(" "),
                    Span::styled(name.clone(), name_style),
                ])
            }).collect();
        f.render_widget(Paragraph::new(lines), list_area);
    }

    pub(super) fn render_settings_panel(&mut self, f: &mut Frame) {
        let content = Self::render_panel_shell(
            f, f.area(), SETTINGS_PANEL_W,
            "Settings",
            "[↑↓]navigate [Space/\u{21b5}]toggle [Esc]close",
        );
        let cfg = self.client.lock().unwrap().config.clone();

        let cursor = self.settings_cursor;
        let confirm_logout = self.confirm_logout;
        let label_w = 30usize;
        let w = content.width as usize;

        let data_sections = &SETTING_SECTIONS[..SETTING_SECTIONS.len() - 1];

        let mut lines: Vec<Line> = vec![Line::from("")];
        let mut cursor_line = 0usize;
        let mut item_idx = 0usize;
        let mut line_of_cursor: Vec<usize> = Vec::new();

        for (sec_name, keys) in data_sections {
            let dash_count = w.saturating_sub(2 + sec_name.len() + 1);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled((*sec_name).to_owned(), Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" {}", "\u{2500}".repeat(dash_count)), Style::default().fg(palette::OVERLAY)),
            ]));
            for &key in *keys {
                line_of_cursor.push(lines.len());
                if item_idx == cursor { cursor_line = lines.len(); }
                let focused = item_idx == cursor;
                let indicator = if focused { "\u{258c}" } else { " " };
                let label = setting_label(key);
                let val = setting_value(key, &cfg);
                let label_style = if focused { Style::default().fg(palette::TEXT) } else { Style::default().fg(palette::MUTED) };
                lines.push(Line::from(vec![
                    Span::styled(indicator, Style::default().fg(palette::IRIS)),
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
        if cursor == logout_cursor_idx { cursor_line = lines.len(); }
        let focused = cursor == logout_cursor_idx;
        let indicator_color = if focused { palette::RED } else { palette::IRIS };
        let (logout_text, logout_style) = if confirm_logout && focused {
            ("Log out? Press y to confirm", Style::default().fg(palette::RED))
        } else if focused {
            ("Log out", Style::default().fg(palette::RED))
        } else {
            ("Log out", Style::default().fg(palette::MUTED))
        };
        lines.push(Line::from(vec![
            Span::styled(if focused { "\u{258c}" } else { " " }, Style::default().fg(indicator_color)),
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
        self.settings_line_of_cursor = line_of_cursor;

        f.render_widget(Paragraph::new(lines).scroll((self.settings_scroll as u16, 0)), content);
    }

    pub(super) fn render_context_menu(&mut self, f: &mut Frame) {
        let Some(ref menu) = self.context_menu else {
            self.context_menu_rect = None;
            return;
        };
        let width = (menu.items.iter().map(|s| s.len()).max().unwrap_or(4) + 4) as u16;
        let height = menu.items.len() as u16 + 2;
        let full = f.area();
        let x = menu.x.min(full.width.saturating_sub(width));
        let y = menu.y.min(full.height.saturating_sub(height));
        let rect = Rect { x, y, width, height };
        self.context_menu_rect = Some(rect);
        f.render_widget(Clear, rect);
        f.render_widget(
            Block::default()
                .borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::IRIS)),
            rect,
        );
        let list_items: Vec<ListItem> = menu.items.iter().enumerate().map(|(i, &label)| {
            let style = if i == menu.cursor {
                Style::default().fg(palette::BASE).bg(palette::IRIS)
            } else {
                Style::default().fg(palette::TEXT)
            };
            ListItem::new(format!(" {label} ")).style(style)
        }).collect();
        let inner = Rect { x: x + 1, y: y + 1, width: width - 2, height: height - 2 };
        f.render_widget(List::new(list_items), inner);
    }
}
