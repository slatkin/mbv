use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;

impl App {
    pub(in crate::app) fn cycle_audio(&mut self) {
        crate::app::LocalPlaybackTarget::cycle_audio(self);
    }

    /// Seeds a freshly attached daemon target's subtitle/audio-language
    /// state with this client's own preferences and pushes them over the
    /// wire. A newly connected `RemotePlayer` always starts at
    /// `SubtitlePrefs::default()` — `mbvd` no longer reads these from its
    /// own host config — so without this, direct-daemon and `stay_alive`
    /// sessions would silently ignore the controlling client's language
    /// preferences until the user manually cycled subtitle mode once.
    /// Call this right after any `self.player = PlayerProxy::from_remote(...)`
    /// assignment.
    pub(in crate::app) fn sync_subtitle_prefs_to_player(&mut self) {
        let prefs = {
            let config = self.config.lock().unwrap();
            if config.subtitle_mode.is_empty()
                && config.subtitle_lang.is_empty()
                && config.audio_lang.is_empty()
            {
                mbv_ctrl::player::SubtitlePrefs::default()
            } else {
                mbv_ctrl::player::SubtitlePrefs {
                    mode: config.subtitle_mode.clone(),
                    subtitle_lang: config.subtitle_lang.clone(),
                    audio_lang: config.audio_lang.clone(),
                }
            }
        };
        self.player.set_subtitle_prefs(prefs);
        self.push_subtitle_prefs();
    }

    /// Fetch Emby's preferences after a service-independent startup reaches
    /// Ready. Local preferences remain authoritative once explicitly set.
    pub(in crate::app) fn sync_subtitle_prefs_from_emby(&mut self) {
        let use_emby_prefs = {
            let config = self.config.lock().unwrap();
            config.subtitle_mode.is_empty()
                && config.subtitle_lang.is_empty()
                && config.audio_lang.is_empty()
        };
        if !use_emby_prefs {
            return;
        }
        let Some(client) = self.emby_client() else {
            return;
        };
        let prefs = match client.lock().unwrap().get_user_subtitle_prefs() {
            Ok(prefs) => prefs,
            Err(error) => {
                tracing::warn!(name: "startup.emby.subtitle_preferences_load_failed", target: "startup", error = %error, "could not load Emby subtitle preferences");
                return;
            }
        };
        self.player.set_subtitle_prefs(prefs);
        self.push_subtitle_prefs();
    }

    /// Clone the current subtitle prefs and notify the player thread.
    pub(in crate::app) fn push_subtitle_prefs(&self) {
        let prefs = self.player.subtitle_prefs_snapshot();
        let _ = self
            .player
            .send_command(mbv_ctrl::player::PlayerCommand::SetSubtitlePrefs {
                mode: prefs.mode,
                subtitle_lang: prefs.subtitle_lang,
                audio_lang: prefs.audio_lang,
            });
    }

    pub(in crate::app) fn cycle_subtitle_mode(&mut self) {
        let (new_mode, cfg) = {
            let mut config = self.config.lock().unwrap();
            config.subtitle_mode =
                mbv_ui_model::ui_util::next_subtitle_mode(&config.subtitle_mode).to_string();
            (config.subtitle_mode.clone(), config.clone())
        };
        self.player
            .update_subtitle_prefs(|prefs| prefs.mode.clone_from(&new_mode));
        self.push_subtitle_prefs();
        if let Err(e) =
            crate::config::save_config_section(&cfg, mbv_config::ConfigSection::Playback)
        {
            tracing::warn!(name: "config.save.failed", target: "config", error = %e, "config save failed");
        }
        self.flash(format!("Subtitle mode: {new_mode}"), ToastSeverity::Neutral);
    }

    /// Returns the next entry in a subtitle-cycle sequence, wrapping around.
    /// `entries` is the ordered list of subtitle option ids -- the "off"
    /// sentinel first (`0`), followed by each available track/index -- and
    /// `current` is the presently active selection. `cycle_sub` walks the
    /// exact same wraparound logic for every track (see #86: local `z` used
    /// to be a plain on/off toggle instead of cycling).
    pub(in crate::app) fn next_subtitle_entry(entries: &[i64], current: i64) -> i64 {
        if entries.is_empty() {
            return current;
        }
        let cur_pos = entries.iter().position(|&e| e == current).unwrap_or(0);
        entries[(cur_pos + 1) % entries.len()]
    }

    pub(in crate::app) fn cycle_sub(&mut self) {
        crate::app::LocalPlaybackTarget::cycle_sub(self);
    }
}
