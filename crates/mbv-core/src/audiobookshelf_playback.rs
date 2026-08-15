use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use super::{AudiobookshelfClient, AudiobookshelfError, AudiobookshelfFailureClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudiobookshelfSourceMethod {
    Direct,
    Hls,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfAudioSource {
    pub method: AudiobookshelfSourceMethod,
    pub url: String,
    pub mime_type: String,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfPlaybackSession {
    pub id: String,
    pub library_item_id: String,
    pub episode_id: String,
    pub duration_seconds: f64,
    pub current_time_seconds: f64,
    pub source: AudiobookshelfAudioSource,
}

/// A book playback session opens over the whole library item (no episode
/// segment) and carries one `AudiobookshelfAudioSource` per audio file, in
/// playback order. The merged timeline is built from `sources`; the summed
/// source durations and the authoritative `duration_seconds` both describe
/// the same continuous book.
#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfBookPlaybackSession {
    pub id: String,
    pub library_item_id: String,
    pub duration_seconds: f64,
    pub current_time_seconds: f64,
    pub sources: Vec<AudiobookshelfAudioSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookshelfPlaybackProgress {
    pub current_time: f64,
    pub time_listened: f64,
    pub duration: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceInfo<'a> {
    device_id: &'a str,
    client_name: &'static str,
    client_version: &'static str,
    manufacturer: &'static str,
    model: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayRequest<'a> {
    device_info: DeviceInfo<'a>,
    force_direct_play: bool,
    force_transcode: bool,
    supported_mime_types: [&'static str; 4],
    media_player: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackSessionWire {
    id: String,
    library_item_id: String,
    episode_id: String,
    media_type: String,
    duration: f64,
    play_method: u8,
    current_time: f64,
    audio_tracks: Vec<AudioTrackWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioTrackWire {
    duration: f64,
    content_url: String,
    mime_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BookPlaybackSessionWire {
    id: String,
    library_item_id: String,
    media_type: String,
    duration: f64,
    play_method: u8,
    current_time: f64,
    audio_tracks: Vec<AudioTrackWire>,
}

impl AudiobookshelfClient {
    pub const HLS_READY_BOUND: Duration = Duration::from_secs(20);
    pub const HLS_POLL_INTERVAL: Duration = Duration::from_millis(250);

    pub fn create_playback_session_bounded(
        &self,
        api_key: &str,
        device_id: &str,
        library_item_id: &str,
        episode_id: &str,
        force_transcode: bool,
        hard_bound: Duration,
    ) -> Result<AudiobookshelfPlaybackSession, AudiobookshelfError> {
        if api_key.trim().is_empty()
            || device_id.trim().is_empty()
            || library_item_id.trim().is_empty()
            || episode_id.trim().is_empty()
        {
            return Err(AudiobookshelfError::protocol());
        }
        let client = self.clone();
        let api_key = api_key.to_owned();
        let device_id = device_id.to_owned();
        let library_item_id = library_item_id.to_owned();
        let episode_id = episode_id.to_owned();
        let cleanup_client = client.clone();
        let cleanup_key = api_key.clone();
        crate::bounded::run_with_hard_bound_or_cleanup(
            move || {
                client.create_playback_session(
                    &api_key,
                    &device_id,
                    &library_item_id,
                    &episode_id,
                    force_transcode,
                )
            },
            move |session| {
                let _ = cleanup_client.close_playback_session_bounded(
                    &cleanup_key,
                    &session.id,
                    AudiobookshelfPlaybackProgress {
                        current_time: 0.0,
                        time_listened: 0.0,
                        duration: session.duration_seconds,
                    },
                    AudiobookshelfClient::REQUEST_HARD_BOUND,
                );
            },
            hard_bound,
        )
    }

    pub fn create_book_playback_session_bounded(
        &self,
        api_key: &str,
        device_id: &str,
        library_item_id: &str,
        force_transcode: bool,
        hard_bound: Duration,
    ) -> Result<AudiobookshelfBookPlaybackSession, AudiobookshelfError> {
        if api_key.trim().is_empty()
            || device_id.trim().is_empty()
            || library_item_id.trim().is_empty()
        {
            return Err(AudiobookshelfError::protocol());
        }
        let client = self.clone();
        let api_key = api_key.to_owned();
        let device_id = device_id.to_owned();
        let library_item_id = library_item_id.to_owned();
        let cleanup_client = client.clone();
        let cleanup_key = api_key.clone();
        crate::bounded::run_with_hard_bound_or_cleanup(
            move || {
                client.create_book_playback_session(
                    &api_key,
                    &device_id,
                    &library_item_id,
                    force_transcode,
                )
            },
            move |session| {
                let _ = cleanup_client.close_playback_session_bounded(
                    &cleanup_key,
                    &session.id,
                    AudiobookshelfPlaybackProgress {
                        current_time: 0.0,
                        time_listened: 0.0,
                        duration: session.duration_seconds,
                    },
                    AudiobookshelfClient::REQUEST_HARD_BOUND,
                );
            },
            hard_bound,
        )
    }

    pub fn sync_playback_session_bounded(
        &self,
        api_key: &str,
        session_id: &str,
        progress: AudiobookshelfPlaybackProgress,
        hard_bound: Duration,
    ) -> Result<(), AudiobookshelfError> {
        self.session_action_bounded(api_key, session_id, "sync", progress, hard_bound)
    }

    pub fn close_playback_session_bounded(
        &self,
        api_key: &str,
        session_id: &str,
        progress: AudiobookshelfPlaybackProgress,
        hard_bound: Duration,
    ) -> Result<(), AudiobookshelfError> {
        self.session_action_bounded(api_key, session_id, "close", progress, hard_bound)
    }

    pub fn wait_for_hls_ready_bounded(
        &self,
        url: &str,
        hard_bound: Duration,
    ) -> Result<(), AudiobookshelfError> {
        let url = url.to_owned();
        let client = self.clone();
        crate::bounded::run_with_hard_bound(
            move || client.wait_for_hls_ready(&url, hard_bound),
            hard_bound,
        )
    }

    fn create_playback_session(
        &self,
        api_key: &str,
        device_id: &str,
        library_item_id: &str,
        episode_id: &str,
        force_transcode: bool,
    ) -> Result<AudiobookshelfPlaybackSession, AudiobookshelfError> {
        let body = PlayRequest {
            device_info: DeviceInfo {
                device_id,
                client_name: "mbv",
                client_version: env!("CARGO_PKG_VERSION"),
                manufacturer: "mbv",
                model: "mbv",
            },
            force_direct_play: !force_transcode,
            force_transcode,
            supported_mime_types: ["audio/mpeg", "audio/mp4", "audio/flac", "audio/ogg"],
            media_player: "mpv",
        };
        let response = self
            .post_json(
                api_key,
                &format!("/api/items/{library_item_id}/play/{episode_id}"),
                &body,
            )?
            .into_json::<serde_json::Value>()
            .map_err(|_| AudiobookshelfError::malformed())?;
        let opened_id = response
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let opened_duration = response
            .get("duration")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let decoded = serde_json::from_value::<PlaybackSessionWire>(response)
            .map_err(|_| AudiobookshelfError::malformed())
            .and_then(|response| {
                self.decode_playback_session(response, library_item_id, episode_id)
            });
        if decoded.is_err() {
            if let Some(session_id) = opened_id {
                let _ = self.post_json(
                    api_key,
                    &format!("/api/session/{session_id}/close"),
                    &AudiobookshelfPlaybackProgress {
                        current_time: 0.0,
                        time_listened: 0.0,
                        duration: opened_duration,
                    },
                );
            }
        }
        decoded
    }

    fn create_book_playback_session(
        &self,
        api_key: &str,
        device_id: &str,
        library_item_id: &str,
        force_transcode: bool,
    ) -> Result<AudiobookshelfBookPlaybackSession, AudiobookshelfError> {
        let body = PlayRequest {
            device_info: DeviceInfo {
                device_id,
                client_name: "mbv",
                client_version: env!("CARGO_PKG_VERSION"),
                manufacturer: "mbv",
                model: "mbv",
            },
            force_direct_play: !force_transcode,
            force_transcode,
            supported_mime_types: ["audio/mpeg", "audio/mp4", "audio/flac", "audio/ogg"],
            media_player: "mpv",
        };
        let response = self
            .post_json(
                api_key,
                &format!("/api/items/{library_item_id}/play"),
                &body,
            )?
            .into_json::<serde_json::Value>()
            .map_err(|_| AudiobookshelfError::malformed())?;
        let opened_id = response
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let opened_duration = response
            .get("duration")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let decoded = serde_json::from_value::<BookPlaybackSessionWire>(response)
            .map_err(|_| AudiobookshelfError::malformed())
            .and_then(|response| self.decode_book_playback_session(response, library_item_id));
        if decoded.is_err() {
            if let Some(session_id) = opened_id {
                let _ = self.post_json(
                    api_key,
                    &format!("/api/session/{session_id}/close"),
                    &AudiobookshelfPlaybackProgress {
                        current_time: 0.0,
                        time_listened: 0.0,
                        duration: opened_duration,
                    },
                );
            }
        }
        decoded
    }

    fn decode_book_playback_session(
        &self,
        response: BookPlaybackSessionWire,
        requested_library_item_id: &str,
    ) -> Result<AudiobookshelfBookPlaybackSession, AudiobookshelfError> {
        if response.id.trim().is_empty()
            || response.media_type != "book"
            || response.library_item_id != requested_library_item_id
            || response.audio_tracks.is_empty()
            || !response.duration.is_finite()
            || response.duration < 0.0
            || !response.current_time.is_finite()
            || response.current_time < 0.0
        {
            return Err(AudiobookshelfError::protocol());
        }
        let method = match response.play_method {
            0 => AudiobookshelfSourceMethod::Direct,
            2 => AudiobookshelfSourceMethod::Hls,
            _ => return Err(AudiobookshelfError::protocol()),
        };
        let mut sources = Vec::with_capacity(response.audio_tracks.len());
        for track in response.audio_tracks {
            match (method, track.mime_type.as_str()) {
                (AudiobookshelfSourceMethod::Direct, mime) if mime.starts_with("audio/") => {}
                (AudiobookshelfSourceMethod::Hls, "application/vnd.apple.mpegurl") => {}
                _ => return Err(AudiobookshelfError::protocol()),
            }
            if !track.duration.is_finite() || track.duration < 0.0 {
                return Err(AudiobookshelfError::protocol());
            }
            let url = self.join_source_path(&track.content_url)?;
            sources.push(AudiobookshelfAudioSource {
                method,
                url,
                mime_type: track.mime_type,
                duration_seconds: track.duration,
            });
        }
        Ok(AudiobookshelfBookPlaybackSession {
            id: response.id,
            library_item_id: response.library_item_id,
            duration_seconds: response.duration,
            current_time_seconds: response.current_time,
            sources,
        })
    }

    fn decode_playback_session(
        &self,
        response: PlaybackSessionWire,
        requested_library_item_id: &str,
        requested_episode_id: &str,
    ) -> Result<AudiobookshelfPlaybackSession, AudiobookshelfError> {
        if response.id.trim().is_empty()
            || response.media_type != "podcast"
            || response.library_item_id != requested_library_item_id
            || response.episode_id != requested_episode_id
            || response.audio_tracks.len() != 1
            || !response.duration.is_finite()
            || response.duration < 0.0
            || !response.current_time.is_finite()
            || response.current_time < 0.0
        {
            return Err(AudiobookshelfError::protocol());
        }
        let track = response.audio_tracks.into_iter().next().unwrap();
        let method = match response.play_method {
            0 if track.mime_type.starts_with("audio/") => AudiobookshelfSourceMethod::Direct,
            2 if track.mime_type == "application/vnd.apple.mpegurl" => {
                AudiobookshelfSourceMethod::Hls
            }
            _ => return Err(AudiobookshelfError::protocol()),
        };
        if !track.duration.is_finite() || track.duration < 0.0 {
            return Err(AudiobookshelfError::protocol());
        }
        let url = self.join_source_path(&track.content_url)?;
        Ok(AudiobookshelfPlaybackSession {
            id: response.id,
            library_item_id: response.library_item_id,
            episode_id: response.episode_id,
            duration_seconds: response.duration,
            current_time_seconds: response.current_time,
            source: AudiobookshelfAudioSource {
                method,
                url,
                mime_type: track.mime_type,
                duration_seconds: track.duration,
            },
        })
    }

    fn join_source_path(&self, path: &str) -> Result<String, AudiobookshelfError> {
        if !path.starts_with('/')
            || path.starts_with("//")
            || path.contains('\\')
            || path.split('/').any(|part| part == "." || part == "..")
        {
            return Err(AudiobookshelfError::protocol());
        }
        Ok(format!("{}{}", self.server_url, path))
    }

    fn session_action_bounded(
        &self,
        api_key: &str,
        session_id: &str,
        action: &'static str,
        progress: AudiobookshelfPlaybackProgress,
        hard_bound: Duration,
    ) -> Result<(), AudiobookshelfError> {
        if api_key.trim().is_empty() || session_id.trim().is_empty() {
            return Err(AudiobookshelfError::protocol());
        }
        let client = self.clone();
        let api_key = api_key.to_owned();
        let session_id = session_id.to_owned();
        crate::bounded::run_with_hard_bound(
            move || {
                client
                    .post_json(
                        &api_key,
                        &format!("/api/session/{session_id}/{action}"),
                        &progress,
                    )
                    .map(|_| ())
            },
            hard_bound,
        )
    }

    fn post_json<T: Serialize>(
        &self,
        api_key: &str,
        path: &str,
        body: &T,
    ) -> Result<ureq::Response, AudiobookshelfError> {
        self.agent
            .post(&format!("{}{}", self.server_url, path))
            .set("Authorization", &format!("Bearer {api_key}"))
            .set("Content-Type", "application/json")
            .send_json(body)
            .map_err(super::audiobookshelf_catalog::map_error)
    }

    fn wait_for_hls_ready(
        &self,
        url: &str,
        hard_bound: Duration,
    ) -> Result<(), AudiobookshelfError> {
        if !url.starts_with(&format!("{}/", self.server_url)) {
            return Err(AudiobookshelfError::protocol());
        }
        let started = Instant::now();
        while started.elapsed() < hard_bound {
            match self.agent.get(url).call() {
                Ok(response) if response.status() == 200 => {
                    let body = response
                        .into_string()
                        .map_err(|_| AudiobookshelfError::malformed())?;
                    if body.starts_with("#EXTM3U") {
                        return Ok(());
                    }
                }
                Ok(_) | Err(ureq::Error::Status(_, _)) | Err(ureq::Error::Transport(_)) => {}
            }
            std::thread::sleep(
                Self::HLS_POLL_INTERVAL.min(hard_bound.saturating_sub(started.elapsed())),
            );
        }
        Err(AudiobookshelfError::new(
            AudiobookshelfFailureClass::Unavailable,
        ))
    }
}
