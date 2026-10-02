use super::super::super::App;
use super::notify::ToastSeverity;
use mbv_ui_model::context_menu::MultiSelectKind;
use mbv_ui_model::overlay::OverlayRequest;
use mbv_ui_model::settings::SettingKey;
use mbv_ui_model::settings::SettingsDestination;
use mbv_ui_model::ui_util::{cycle_lang, next_subtitle_mode};
use std::time::{Duration, Instant};

impl App {
    pub(crate) fn close_settings(&mut self) {
        self.flush_settings_save();
        self.request_sidebar_dismiss(mbv_ui_model::overlay::SidebarId::Settings);
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
        if matches!(
            key,
            SettingKey::StayAlive | SettingKey::ConsumeVideos | SettingKey::ConsumeAudio
        ) {
            self.flush_settings_save();
        }
    }

    pub(crate) fn flush_settings_save(&mut self) {
        if self.settings_save_at.take().is_some() {
            let cfg = self.config.lock().unwrap().clone();
            crate::config::save_config_with_ui(&cfg, &self.ui_config_snapshot());
        }
    }

    /// Change one Panel-destination row (design D6, change
    /// `pin-mbv-in-pinwin`): `Side` cycles, every numeric row steps by one.
    pub(crate) fn handle_panel_setting_activate(&mut self, key: SettingKey) {
        if key == SettingKey::PanelSide {
            self.apply_panel_setting(key, 0);
        } else {
            self.apply_panel_setting(key, 1);
        }
    }

    /// Step one Panel-destination row by `delta` (the component scales Shift
    /// steps to ±10).
    pub(crate) fn handle_panel_setting_step(&mut self, key: SettingKey, delta: i32) {
        self.apply_panel_setting(key, delta);
    }

    /// Apply and persist one Panel value change: the running panel validates
    /// the candidate layout first, and only an accepted layout is saved. A
    /// rejection keeps the previous value and shows its reason as a Warning
    /// toast (design D6).
    fn apply_panel_setting(&mut self, key: SettingKey, delta: i32) {
        let candidate = {
            let panel = self.config.lock().unwrap().panel;
            mbv_ui_model::settings::changed_panel_config(key, panel, delta)
        };
        let Some(candidate) = candidate else {
            return;
        };
        // Row 4.4 of `pin-mbv-in-pinwin` replaces this with the live
        // `pinwin_apply_layout` call. Until a panel handle exists on `App`
        // there is no panel to validate against, so every step saves.
        let applied: Result<(), String> = Ok(());
        match applied {
            Ok(()) => {
                self.config.lock().unwrap().panel = candidate;
                self.settings_save_at = Some(Instant::now() + Duration::from_millis(500));
            }
            Err(reason) => self.flash(reason, ToastSeverity::Warning),
        }
    }

    fn open_settings_destination(&mut self, key: SettingKey) -> bool {
        match key {
            SettingKey::Services => {
                self.open_services_settings();
            }
            SettingKey::Keys => {
                self.open_keys_settings();
            }
            SettingKey::Panel => {
                self.open_panel_settings();
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
        let protocol = match self.images.protocol_override() {
            None => Some("halfblocks".into()),
            Some("halfblocks") => Some("sixel".into()),
            Some("sixel") => Some("kitty".into()),
            Some("kitty") => Some("iterm2".into()),
            Some("iterm2") => Some("auto".into()),
            _ => None,
        };
        let enabled = protocol.is_some();
        self.images.configure_protocol(protocol, enabled);
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
            c.subtitle_lang.clone_from(&new);
            new
        };
        self.player.subtitle_prefs.lock().unwrap().subtitle_lang = new_lang;
        self.push_subtitle_prefs();
    }

    fn cycle_audio_language(&mut self) {
        let new_lang = {
            let mut c = self.config.lock().unwrap();
            let new = cycle_lang(&c.my_languages, &c.audio_lang);
            c.audio_lang.clone_from(&new);
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
                c.save_playlist_on_consume = !c.save_playlist_on_consume;
            }
            SettingKey::SavePlaylistOnConsumeAudio => {
                c.save_playlist_on_consume_audio = !c.save_playlist_on_consume_audio;
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
