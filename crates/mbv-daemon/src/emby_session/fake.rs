//! A recording `SessionApi` double for the daemon tests (emby-session-owner-
//! output design D4): no network, no live Emby server. It records every call
//! the worker makes and returns scripted Sessions, so a test can assert both
//! what reached the device and what the owner observed.

use std::sync::{Arc, Mutex};

use mbv_emby::SessionInfo;

use super::worker::SessionApi;

/// One call the worker made on the fake, in order. Tests match on the exact
/// arguments so a load or a transport command cannot pass unnoticed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionCall {
    GetSessions,
    Play {
        session_id: String,
        item_id: String,
        start_ticks: i64,
    },
    Transport {
        session_id: String,
        command: String,
    },
    Seek {
        session_id: String,
        ticks: i64,
    },
    SetVolume {
        session_id: String,
        volume: i64,
    },
    SetMute {
        session_id: String,
        muted: bool,
    },
    SetAudioIndex {
        session_id: String,
        index: i64,
    },
    SetSubtitleIndex {
        session_id: String,
        index: i64,
    },
}

/// The recording `SessionApi` double (design D4). Handles are `Arc`s so a test
/// keeps reading them after the fake itself moved into the worker thread;
/// `Clone` shares those handles, so a test can build one fake per selected
/// output and still assert on a single call log.
#[derive(Debug, Default, Clone)]
pub(crate) struct FakeSessionApi {
    calls: Arc<Mutex<Vec<SessionCall>>>,
    sessions: Arc<Mutex<Vec<SessionInfo>>>,
    failure: Arc<Mutex<Option<String>>>,
}

impl FakeSessionApi {
    /// A fake whose `get_sessions` returns `sessions`.
    pub(crate) fn with_sessions(sessions: Vec<SessionInfo>) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(sessions)),
            ..Self::default()
        }
    }

    /// The call log, readable after the fake moved into the worker thread.
    pub(crate) fn calls(&self) -> Arc<Mutex<Vec<SessionCall>>> {
        Arc::clone(&self.calls)
    }

    /// Replaces the Sessions the next poll returns, so a test can make a
    /// device appear, disappear, or change its session id.
    pub(crate) fn set_sessions(&self, sessions: Vec<SessionInfo>) {
        *self.sessions.lock().unwrap() = sessions;
    }

    /// Makes every call fail with `error`.
    pub(crate) fn fail_with(&self, error: &str) {
        *self.failure.lock().unwrap() = Some(error.to_string());
    }

    fn record(&self, call: SessionCall) -> Result<(), String> {
        self.calls.lock().unwrap().push(call);
        match self.failure.lock().unwrap().as_deref() {
            Some(error) => Err(error.to_string()),
            None => Ok(()),
        }
    }

    fn recorded_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        match self.failure.lock().unwrap().as_deref() {
            Some(error) => Err(error.to_string()),
            None => Ok(self.sessions.lock().unwrap().clone()),
        }
    }
}

impl SessionApi for FakeSessionApi {
    fn get_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        self.calls.lock().unwrap().push(SessionCall::GetSessions);
        self.recorded_sessions()
    }

    fn play(&self, session_id: &str, item_id: &str, start_ticks: i64) -> Result<(), String> {
        self.record(SessionCall::Play {
            session_id: session_id.to_string(),
            item_id: item_id.to_string(),
            start_ticks,
        })
    }

    fn transport(&self, session_id: &str, command: &str) -> Result<(), String> {
        self.record(SessionCall::Transport {
            session_id: session_id.to_string(),
            command: command.to_string(),
        })
    }

    fn seek(&self, session_id: &str, ticks: i64) -> Result<(), String> {
        self.record(SessionCall::Seek {
            session_id: session_id.to_string(),
            ticks,
        })
    }

    fn set_volume(&self, session_id: &str, volume: i64) -> Result<(), String> {
        self.record(SessionCall::SetVolume {
            session_id: session_id.to_string(),
            volume,
        })
    }

    fn set_mute(&self, session_id: &str, muted: bool) -> Result<(), String> {
        self.record(SessionCall::SetMute {
            session_id: session_id.to_string(),
            muted,
        })
    }

    fn set_audio_index(&self, session_id: &str, index: i64) -> Result<(), String> {
        self.record(SessionCall::SetAudioIndex {
            session_id: session_id.to_string(),
            index,
        })
    }

    fn set_subtitle_index(&self, session_id: &str, index: i64) -> Result<(), String> {
        self.record(SessionCall::SetSubtitleIndex {
            session_id: session_id.to_string(),
            index,
        })
    }
}

/// A scripted Session with every field at a quiet default; tests override the
/// fields they assert on.
pub(crate) fn session(device_id: &str, session_id: &str) -> SessionInfo {
    SessionInfo {
        id: session_id.to_string(),
        device_id: device_id.to_string(),
        device_name: "Living room TV".to_string(),
        client: "Emby".to_string(),
        user_name: "user".to_string(),
        host: "192.0.2.20".to_string(),
        supported_commands: Vec::new(),
        playable_media_types: Vec::new(),
        now_playing: None,
        now_playing_item_id: None,
        now_playing_item_type: None,
        now_playing_series_id: None,
        now_playing_series_name: None,
        now_playing_logo_etag: None,
        position_s: 0,
        runtime_s: 0,
        position_ticks: 0,
        runtime_ticks: 0,
        is_paused: false,
        volume: 100,
        sub_index: -1,
        audio_index: 0,
        muted: false,
        media_info: mbv_emby::SessionMediaInfo::default(),
    }
}
