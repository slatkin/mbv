use super::notify_actions::ToastSeverity;
use super::App;

impl App {
    pub(super) fn toggle_mute(&mut self) {
        self.playback_target().toggle_soft_mute(self);
    }

    /// Session-aware mute toggle for `Action::ToggleMute` (the `m` key) when
    /// attached to a remote session. Mirrors `cycle_audio()`/`cycle_sub()`:
    /// computes an explicit target state (not a blind server-side toggle),
    /// writes it into `connected_session_state` optimistically, and fires the
    /// outbound command asynchronously via `do_session_command`. Does not
    /// touch local player mute state or the persisted `mute_on` preference --
    /// those are exclusively the local (no-session) branch's concern.
    pub(super) fn session_toggle_mute(&mut self) {
        let Some(conn_id) = self.connected_session_id.clone() else {
            return;
        };
        let current = self
            .connected_session_state
            .as_ref()
            .map(|s| s.muted)
            .unwrap_or(false);
        let next = !current;
        if let Some(ref mut state) = self.connected_session_state {
            state.muted = next;
        }
        self.do_session_command(move |c| c.session_set_mute(&conn_id, next));
    }

    pub(super) fn cycle_audio(&mut self) {
        self.playback_target().cycle_audio(self);
    }

    /// Seeds a freshly attached daemon target's subtitle/audio-language
    /// state with this client's own preferences and pushes them over the
    /// wire. A newly connected `RemotePlayer` always starts at
    /// `SubtitlePrefs::default()` — `mbvd` no longer reads these from its
    /// own host config — so without this, direct-daemon and stay_alive
    /// sessions would silently ignore the controlling client's language
    /// preferences until the user manually cycled subtitle mode once.
    /// Call this right after any `self.player = PlayerProxy::remote(...)`
    /// assignment.
    pub(super) fn sync_subtitle_prefs_to_player(&mut self) {
        let prefs = {
            let client = self.client.lock().unwrap();
            if client.config.subtitle_mode.is_empty()
                && client.config.subtitle_lang.is_empty()
                && client.config.audio_lang.is_empty()
            {
                client.get_user_subtitle_prefs().unwrap_or_default()
            } else {
                mbv_core::player::SubtitlePrefs {
                    mode: client.config.subtitle_mode.clone(),
                    subtitle_lang: client.config.subtitle_lang.clone(),
                    audio_lang: client.config.audio_lang.clone(),
                }
            }
        };
        *self.player.subtitle_prefs.lock().unwrap() = prefs;
        self.push_subtitle_prefs();
    }

    /// Clone the current subtitle prefs from the shared Arc and notify the player thread.
    pub(super) fn push_subtitle_prefs(&self) {
        let prefs = self.player.subtitle_prefs.lock().unwrap().clone();
        self.player
            .send_command(mbv_core::player::PlayerCommand::SetSubtitlePrefs {
                mode: prefs.mode,
                subtitle_lang: prefs.subtitle_lang,
                audio_lang: prefs.audio_lang,
            });
    }

    pub(super) fn cycle_subtitle_mode(&mut self) {
        let (new_mode, cfg) = {
            let mut c = self.client.lock().unwrap();
            c.config.subtitle_mode =
                super::ui_util::next_subtitle_mode(&c.config.subtitle_mode).to_string();
            (c.config.subtitle_mode.clone(), c.config.clone())
        };
        self.player.subtitle_prefs.lock().unwrap().mode = new_mode.clone();
        self.push_subtitle_prefs();
        if let Err(e) = crate::config::save_config_settings(&cfg) {
            log::warn!(target: "config", "config save failed: {e}");
        }
        self.flash(format!("Subtitle mode: {new_mode}"), ToastSeverity::Neutral);
    }

    /// Returns the next entry in a subtitle-cycle sequence, wrapping around.
    /// `entries` is the ordered list of subtitle option ids -- the "off"
    /// sentinel first (`0` for local playback, `-1` for remote sessions),
    /// followed by each available track/index -- and `current` is the
    /// presently active selection. Shared by the remote-session and local
    /// branches of `cycle_sub` so both walk the exact same wraparound logic
    /// (see #86: local `z` used to be a plain on/off toggle instead of
    /// cycling through every track like the remote path).
    pub(super) fn next_subtitle_entry(entries: &[i64], current: i64) -> i64 {
        if entries.is_empty() {
            return current;
        }
        let cur_pos = entries.iter().position(|&e| e == current).unwrap_or(0);
        entries[(cur_pos + 1) % entries.len()]
    }

    /// Toggles between "off" and the last-selected subtitle index for a
    /// remote session. The only remaining caller is `cycle_sub`'s
    /// remote-session branch, as a fallback for when the session reports
    /// zero subtitle tracks (nothing to cycle through). Local playback no
    /// longer routes through here -- see #86, which replaced its on/off
    /// toggle with full track-cycling in `cycle_sub`.
    pub(super) fn toggle_sub(&mut self) {
        let Some(conn_id) = self.connected_session_id.clone() else {
            return;
        };
        let remote_indexes = self.remote_subtitle_indexes();
        let idx = self
            .connected_session_state
            .as_ref()
            .map(|s| s.sub_index)
            .unwrap_or(-1);
        let next = if idx == -1 {
            remote_indexes.first().copied().unwrap_or(1)
        } else {
            -1
        };
        if let Some(ref mut state) = self.connected_session_state {
            state.sub_index = next;
        }
        self.do_session_command(move |c| c.session_set_subtitle_index(&conn_id, next));
    }

    pub(super) fn cycle_sub(&mut self) {
        self.playback_target().cycle_sub(self);
    }
}
