use super::super::super::App;
use super::notify::ToastSeverity;
use crate::pin::PinnedSettingRequest;
use mbv_ui_model::context_menu::MultiSelectKind;
use mbv_ui_model::overlay::OverlayRequest;
use mbv_ui_model::settings::PinnedWidth;
use mbv_ui_model::settings::SettingKey;
use mbv_ui_model::settings::SettingsDestination;
use mbv_ui_model::ui_util::{cycle_lang, next_subtitle_mode};
use std::time::{Duration, Instant};

/// How a Settings row responds when the user activates it. Each row is
/// classified exactly once, and the dispatch sites route on the kind, so
/// a new row states its behavior once instead of once per dispatch site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingRowKind {
    /// Opens a destination: a sub-panel, a multiselect overlay, the
    /// library-routes editor or the feeds manager.
    Destination,
    /// Activates through its own handler: a confirm step, a cycle, or a
    /// toggle with a refusal rule or a live side effect.
    OwnActivation,
    /// Flips one stored config flag in place.
    ConfigToggle,
    /// Takes no activation in this dispatch: the panel geometry rows step
    /// through `apply_panel_setting`, and `ResetUiState` runs through its
    /// own confirm path in the shell.
    Passive,
}

/// Classify one Settings row. The match is exhaustive over `SettingKey`,
/// so a new row cannot be added without stating its kind once.
fn setting_row_kind(key: SettingKey) -> SettingRowKind {
    use SettingKey as K;
    match key {
        K::Services
        | K::Keys
        | K::Panel
        | K::HiddenLibraries
        | K::MyLanguages
        | K::FeedViewLibraries
        | K::LibraryRoutes
        | K::ManageFeeds => SettingRowKind::Destination,
        K::LogOut
        | K::ImageProtocol
        | K::ShowSysTrayIcon
        | K::SystemNotifications
        | K::MouseSupport
        | K::SubtitleMode
        | K::SubtitleLanguage
        | K::AudioLanguage => SettingRowKind::OwnActivation,
        K::StayAlive
        | K::AutoReconnect
        | K::SavePlaylistOnQuit
        | K::AlwaysPlayNext
        | K::ConsumeVideos
        | K::ConsumeAudio
        | K::SavePlaylistOnConsume
        | K::SavePlaylistOnConsumeAudio
        | K::AlwaysSkipIntro
        | K::ShowAudioWindow
        | K::UseMpvConfig
        | K::NoScripts
        | K::Autoload => SettingRowKind::ConfigToggle,
        K::PanelSide
        | K::PanelWidth
        | K::PanelWidthExpanded
        | K::PanelGutterTop
        | K::PanelGutterBottom
        | K::PanelGutterLeft
        | K::PanelGutterRight
        | K::PanelAccent
        | K::PanelAccentColor
        | K::PanelAccentWidth
        | K::ResetUiState => SettingRowKind::Passive,
    }
}

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

    /// Apply and persist one Panel value change: an activate steps the row up
    /// by one step (`Side`
    /// cycles and ignores the delta), a step applies the component's signed
    /// delta (±1, or ±10 with Shift), and a width row counts each step as 10
    /// logical pixels while every other row counts 1.
    ///
    /// While pinned, the row sends one `pinwin` request and only an accepted
    /// request is saved: a width row asks for the width it edited, a `Side`
    /// row moves the panel, and a gutter row
    /// changes that gutter at the panel's current width. A refusal keeps the
    /// previous value and flashes pinwin's reason as a Warning toast.
    ///
    /// While pinned, the width recorded is the one the edited row belongs to:
    /// `Width` is `width`, `Expanded width` is `width_expanded`, any other row
    /// keeps the current width. An unpinned launch never changes the active
    /// width.
    ///
    /// The accent rows are not part of the layout and the accent is fixed at
    /// panel start, so they send no
    /// panel request; while pinned a `Neutral` toast announces that the change
    /// applies on the next `mbv --pin` launch.
    pub(crate) fn apply_panel_setting(&mut self, key: SettingKey, delta: i32) {
        self.apply_panel_setting_pinned(key, delta, crate::pin::is_pinned());
    }

    /// The Panel-row path with the pinned decision supplied, so a test can
    /// drive the pinned branch without claiming the process-wide identity.
    pub(in crate::app) fn apply_panel_setting_pinned(
        &mut self,
        key: SettingKey,
        delta: i32,
        pinned: bool,
    ) {
        // One request in flight: an F2 step while pinwin is still working is
        // ignored, with no queue (design D4/D7).
        if pinned && self.pinned_request_pending.is_some() {
            return;
        }
        let candidate = {
            let panel = self.config.lock().unwrap().panel.clone();
            self.accent_custom = self
                .accent_custom
                .or_else(|| mbv_ui_model::settings::accent_color_outside_cycle(panel.accent_color));
            mbv_ui_model::settings::changed_panel_config(key, panel, delta, self.accent_custom)
        };
        let Some(candidate) = candidate else {
            return;
        };
        let request = pinned_setting_request(key, &candidate);
        // Outside a panel, and for a row that takes no request (an accent
        // row), the candidate saves as it is.
        if let (Some(request), true) = (request, pinned) {
            let edited_width = request.edited_width();
            let pending = crate::app::state::pinned_request::PinnedRequestPending::Setting {
                key,
                edited_width,
                candidate: candidate.clone(),
            };
            if let Err(reason) = self.begin_pinned_request(pending, &candidate, request) {
                self.apply_panel_setting_outcome(true, key, edited_width, candidate, Err(reason));
            }
            return;
        }
        let edited_width = request.and_then(PinnedSettingRequest::edited_width);
        self.apply_panel_setting_outcome(pinned, key, edited_width, candidate, Ok(()));
    }

    /// Apply a Panel row's candidate once the panel's request outcome is known:
    /// only an accepted request saves,
    /// and a refusal flashes pinwin's reason and keeps both the row's value and
    /// the panel. A width row records the width it switched the panel to (the
    /// request already derived it), and an accent row while pinned announces
    /// the next launch.
    pub(in crate::app) fn apply_panel_setting_outcome(
        &mut self,
        pinned: bool,
        key: SettingKey,
        edited_width: Option<PinnedWidth>,
        candidate: mbv_config::PanelConfig,
        applied: Result<(), String>,
    ) {
        if let Err(reason) = applied {
            self.flash(reason, ToastSeverity::Warning);
            return;
        }
        self.config.lock().unwrap().panel = candidate;
        if pinned && let Some(edited) = edited_width {
            self.note_pinned_width_applied(edited);
        }
        self.settings_save_at = Some(Instant::now() + Duration::from_millis(500));
        if pinned && mbv_ui_model::settings::is_panel_accent_row(key) {
            self.flash(
                "Accent changes apply on the next mbv --pin launch".into(),
                ToastSeverity::Neutral,
            );
        }
    }

    /// Record the width a running panel accepted (change
    /// `panel-expand-toggle`): the active width only. The new terminal size
    /// arrives as a normal `Resize` observer event and leaves the image cache
    /// and `force_clear` alone (invariant 17). Both the width toggle and a
    /// Panel settings step record through here.
    pub(in crate::app) fn note_pinned_width_applied(&mut self, width: PinnedWidth) {
        self.pinned_width = width;
    }

    fn open_settings_destination(&mut self, key: SettingKey) -> bool {
        if setting_row_kind(key) != SettingRowKind::Destination {
            return false;
        }
        // The kind gate admits Destination rows only, so each branch below
        // is that row's own destination opener.
        if key == SettingKey::Services {
            self.open_services_settings();
        }
        if key == SettingKey::Keys {
            self.open_keys_settings();
        }
        if key == SettingKey::Panel {
            self.open_panel_settings();
        }
        if key == SettingKey::HiddenLibraries {
            self.pending_overlay = Some(OverlayRequest::OpenMultiselect(
                MultiSelectKind::HiddenLibraries,
            ));
        }
        if key == SettingKey::MyLanguages {
            self.pending_overlay = Some(OverlayRequest::OpenMultiselect(
                MultiSelectKind::MyLanguages,
            ));
        }
        if key == SettingKey::FeedViewLibraries {
            self.pending_overlay = Some(OverlayRequest::OpenMultiselect(
                MultiSelectKind::FeedViewLibraries,
            ));
        }
        if key == SettingKey::LibraryRoutes {
            self.pending_overlay = Some(OverlayRequest::OpenLibraryRoutes);
        }
        if key == SettingKey::ManageFeeds {
            self.pending_overlay = Some(OverlayRequest::OpenFeedsManage);
        }
        true
    }

    fn apply_settings_activation(&mut self, key: SettingKey) {
        match setting_row_kind(key) {
            // open_settings_destination already routed the row, and the
            // panel geometry rows step through `apply_panel_setting`
            // (`ResetUiState` runs through its own confirm path in the
            // shell), so neither reaches a config toggle.
            SettingRowKind::Destination | SettingRowKind::Passive => {}
            SettingRowKind::OwnActivation => self.apply_own_activation(key),
            SettingRowKind::ConfigToggle => self.toggle_config_setting(key),
        }
    }

    /// The `OwnActivation` handlers, one per row. The caller's kind match
    /// admits `OwnActivation` rows only, so each branch below is that row's
    /// own activation.
    fn apply_own_activation(&mut self, key: SettingKey) {
        if key == SettingKey::LogOut {
            self.confirm_logout = true;
        }
        if key == SettingKey::ImageProtocol {
            self.cycle_image_protocol();
        }
        if key == SettingKey::SystemNotifications {
            let new_val = {
                let mut c = self.config.lock().unwrap();
                c.system_notifications = !c.system_notifications;
                c.system_notifications
            };
            self.system_notifications = new_val;
        }
        if key == SettingKey::MouseSupport {
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
        if key == SettingKey::SubtitleMode {
            self.apply_subtitle_mode();
        }
        if key == SettingKey::SubtitleLanguage {
            self.cycle_subtitle_language();
        }
        if key == SettingKey::AudioLanguage {
            self.cycle_audio_language();
        }
        // The Tray row shows `tray_enabled()` — the single source of the
        // tray-enable rule — not the stored preference, so its toggle is
        // refused when flipping the preference cannot change that value:
        // while stay-alive forces the Tray on. `toggle_config_setting` holds
        // the config lock, so the refusal is decided here where flashing
        // is possible; a refused toggle is reverted so the stored
        // preference stays untouched.
        if key == SettingKey::ShowSysTrayIcon {
            let refused = {
                let mut c = self.config.lock().unwrap();
                let enabled = c.tray_enabled();
                c.show_systray_icon = !c.show_systray_icon;
                let refused = c.tray_enabled() == enabled;
                if refused {
                    c.show_systray_icon = !c.show_systray_icon;
                }
                refused
            };
            if refused {
                self.flash(
                    "Tray stays on while Stay alive is on".into(),
                    ToastSeverity::Neutral,
                );
            }
        }
    }

    fn cycle_image_protocol(&mut self) {
        let protocol = match self.images.protocol_override() {
            None => Some("halfblocks".into()),
            Some("halfblocks") => Some("sixel".into()),
            Some("sixel") => Some("kitty".into()),
            Some("kitty") => Some("iterm2".into()),
            Some("iterm2") => Some("auto".into()),
            // The protocol name is a string, so the key space is open: an
            // unknown protocol has no next protocol.
            // ast-grep-ignore: mbv-dispatch-no-wildcard-arm
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
        self.player
            .update_subtitle_prefs(|prefs| prefs.mode = new_mode);
        self.push_subtitle_prefs();
    }

    fn cycle_subtitle_language(&mut self) {
        let new_lang = {
            let mut c = self.config.lock().unwrap();
            let new = cycle_lang(&c.my_languages, &c.subtitle_lang);
            c.subtitle_lang.clone_from(&new);
            new
        };
        self.player
            .update_subtitle_prefs(|prefs| prefs.subtitle_lang = new_lang);
        self.push_subtitle_prefs();
    }

    fn cycle_audio_language(&mut self) {
        let new_lang = {
            let mut c = self.config.lock().unwrap();
            let new = cycle_lang(&c.my_languages, &c.audio_lang);
            c.audio_lang.clone_from(&new);
            new
        };
        self.player
            .update_subtitle_prefs(|prefs| prefs.audio_lang = new_lang);
        self.push_subtitle_prefs();
    }

    /// Flip the stored config flag of a `ConfigToggle` row. Rows of any
    /// other kind keep no flag here, so the kind gate returns before the
    /// config lock is taken.
    fn toggle_config_setting(&mut self, key: SettingKey) {
        if setting_row_kind(key) != SettingRowKind::ConfigToggle {
            return;
        }
        let mut c = self.config.lock().unwrap();
        if key == SettingKey::StayAlive {
            c.stay_alive = !c.stay_alive;
        }
        if key == SettingKey::AutoReconnect {
            c.auto_reconnect = !c.auto_reconnect;
        }
        if key == SettingKey::SavePlaylistOnQuit {
            c.save_playlist_on_quit = !c.save_playlist_on_quit;
        }
        if key == SettingKey::AlwaysPlayNext {
            c.always_play_next = !c.always_play_next;
        }
        if key == SettingKey::ConsumeVideos {
            c.consume_videos = !c.consume_videos;
        }
        if key == SettingKey::ConsumeAudio {
            c.consume_audio = !c.consume_audio;
        }
        if key == SettingKey::SavePlaylistOnConsume {
            c.save_playlist_on_consume = !c.save_playlist_on_consume;
        }
        if key == SettingKey::SavePlaylistOnConsumeAudio {
            c.save_playlist_on_consume_audio = !c.save_playlist_on_consume_audio;
        }
        if key == SettingKey::AlwaysSkipIntro {
            c.always_skip_intro = !c.always_skip_intro;
        }
        if key == SettingKey::ShowAudioWindow {
            c.show_audio_window = !c.show_audio_window;
        }
        if key == SettingKey::UseMpvConfig {
            c.use_mpv_config = !c.use_mpv_config;
        }
        if key == SettingKey::NoScripts {
            c.no_scripts = !c.no_scripts;
        }
        if key == SettingKey::Autoload {
            c.autoload = !c.autoload;
        }
    }
}

/// The request a Panel row sends while pinned, or `None` for a row
/// that takes none: an accent row is fixed at panel start and any
/// other Settings row is not a Panel layout value.
fn pinned_setting_request(
    key: SettingKey,
    candidate: &mbv_config::PanelConfig,
) -> Option<PinnedSettingRequest> {
    match key {
        SettingKey::PanelWidth | SettingKey::PanelWidthExpanded => {
            mbv_ui_model::settings::panel_row_edited_width(key).map(PinnedSettingRequest::Width)
        }
        SettingKey::PanelSide => Some(PinnedSettingRequest::Side(candidate.side)),
        SettingKey::PanelGutterTop => Some(PinnedSettingRequest::Gutter(
            crate::pin::PanelGutter::Top,
            candidate.gutter_top,
        )),
        SettingKey::PanelGutterBottom => Some(PinnedSettingRequest::Gutter(
            crate::pin::PanelGutter::Bottom,
            candidate.gutter_bottom,
        )),
        SettingKey::PanelGutterLeft => Some(PinnedSettingRequest::Gutter(
            crate::pin::PanelGutter::Left,
            candidate.gutter_left,
        )),
        SettingKey::PanelGutterRight => Some(PinnedSettingRequest::Gutter(
            crate::pin::PanelGutter::Right,
            candidate.gutter_right,
        )),
        SettingKey::PanelAccent | SettingKey::PanelAccentColor | SettingKey::PanelAccentWidth => {
            None
        }
        SettingKey::Services
        | SettingKey::Keys
        | SettingKey::Panel
        | SettingKey::StayAlive
        | SettingKey::AutoReconnect
        | SettingKey::SavePlaylistOnQuit
        | SettingKey::AlwaysPlayNext
        | SettingKey::ConsumeVideos
        | SettingKey::ConsumeAudio
        | SettingKey::SavePlaylistOnConsume
        | SettingKey::SavePlaylistOnConsumeAudio
        | SettingKey::AlwaysSkipIntro
        | SettingKey::ImageProtocol
        | SettingKey::HiddenLibraries
        | SettingKey::ShowAudioWindow
        | SettingKey::UseMpvConfig
        | SettingKey::NoScripts
        | SettingKey::Autoload
        | SettingKey::ShowSysTrayIcon
        | SettingKey::SystemNotifications
        | SettingKey::MouseSupport
        | SettingKey::MyLanguages
        | SettingKey::SubtitleMode
        | SettingKey::FeedViewLibraries
        | SettingKey::LibraryRoutes
        | SettingKey::SubtitleLanguage
        | SettingKey::AudioLanguage
        | SettingKey::ManageFeeds
        | SettingKey::ResetUiState
        | SettingKey::LogOut => None,
    }
}

#[cfg(test)]
mod tests;
