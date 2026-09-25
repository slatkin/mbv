use super::super::super::App;
use super::super::super::{MultiSelectKind, SettingKey};
use crate::app::infra::ui_util::{cycle_lang, next_subtitle_mode};
use crate::app::state::types::overlay::OverlayRequest;
use crate::app::state::types::settings::SettingsDestination;
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
        if self.open_settings_destination(key) {
            return;
        }
        self.apply_settings_activation(key);
        if key == SettingKey::AutoReconnect && self.config.lock().unwrap().auto_reconnect {
            self.persist_current_auto_reconnect_target();
        }
        self.settings_save_at = Some(Instant::now() + Duration::from_millis(500));
    }

    fn open_settings_destination(&mut self, key: SettingKey) -> bool {
        match key {
            SettingKey::Services => {
                self.open_services_settings();
            }
            SettingKey::Keys => {
                self.open_keys_settings();
            }
            SettingKey::HiddenLibraries => {
                self.pending_overlay = Some(OverlayRequest::OpenMultiselect(
                    MultiSelectKind::HiddenLibraries,
                ));
            }
            SettingKey::MyLanguages => {
                self.pending_overlay = Some(OverlayRequest::OpenMultiselect(
                    MultiSelectKind::MyLanguages,
                ));
            }
            SettingKey::FeedViewLibraries => {
                self.pending_overlay = Some(OverlayRequest::OpenMultiselect(
                    MultiSelectKind::FeedViewLibraries,
                ));
            }
            SettingKey::LibraryRoutes => {
                self.pending_overlay = Some(OverlayRequest::OpenLibraryRoutes);
            }
            SettingKey::ManageFeeds => {
                self.pending_overlay = Some(OverlayRequest::OpenFeedsManage);
            }
            _ => return false,
        }
        true
    }

    fn apply_settings_activation(&mut self, key: SettingKey) {
        match key {
            SettingKey::LogOut => self.confirm_logout = true,
            SettingKey::ImageProtocol => self.cycle_image_protocol(),
            SettingKey::SystemNotifications => {
                let new_val = {
                    let mut c = self.config.lock().unwrap();
                    c.system_notifications = !c.system_notifications;
                    c.system_notifications
                };
                self.system_notifications = new_val;
            }
            SettingKey::MouseSupport => {
                let new_val = {
                    let mut c = self.config.lock().unwrap();
                    c.mouse_support = !c.mouse_support;
                    c.mouse_support
                };
                // Effect handoff: the run loop applies the live flip on the
                // session stdout; persistence rides the debounced
                // `settings_save_at` below.
                self.mouse_capture_pending = Some(new_val);
            }
            SettingKey::SubtitleMode => self.apply_subtitle_mode(),
            SettingKey::SubtitleLanguage => self.cycle_subtitle_language(),
            SettingKey::AudioLanguage => self.cycle_audio_language(),
            _ => self.toggle_config_setting(key),
        }
    }

    fn cycle_image_protocol(&mut self) {
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

    fn apply_subtitle_mode(&mut self) {
        let new_mode = {
            let mut c = self.config.lock().unwrap();
            c.subtitle_mode = next_subtitle_mode(&c.subtitle_mode).to_string();
            c.subtitle_mode.clone()
        };
        self.player.subtitle_prefs.lock().unwrap().mode = new_mode;
        self.push_subtitle_prefs();
    }

    fn cycle_subtitle_language(&mut self) {
        let new_lang = {
            let mut c = self.config.lock().unwrap();
            let new = cycle_lang(&c.my_languages, &c.subtitle_lang);
            c.subtitle_lang = new.clone();
            new
        };
        self.player.subtitle_prefs.lock().unwrap().subtitle_lang = new_lang;
        self.push_subtitle_prefs();
    }

    fn cycle_audio_language(&mut self) {
        let new_lang = {
            let mut c = self.config.lock().unwrap();
            let new = cycle_lang(&c.my_languages, &c.audio_lang);
            c.audio_lang = new.clone();
            new
        };
        self.player.subtitle_prefs.lock().unwrap().audio_lang = new_lang;
        self.push_subtitle_prefs();
    }

    fn toggle_config_setting(&mut self, key: SettingKey) {
        let mut c = self.config.lock().unwrap();
        match key {
            SettingKey::StayAlive => c.stay_alive = !c.stay_alive,
            SettingKey::AutoReconnect => c.auto_reconnect = !c.auto_reconnect,
            SettingKey::SavePlaylistOnQuit => c.save_playlist_on_quit = !c.save_playlist_on_quit,
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
