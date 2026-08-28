use super::super::super::types_overlay::OverlayRequest;
use super::super::super::types_settings::SettingsDestination;
use super::super::super::ui_util::{cycle_lang, next_subtitle_mode};
use super::super::super::App;
use super::super::super::{MultiSelectKind, SettingKey};
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
}
