use crate::audiobookshelf::{
    AudiobookshelfClient, AudiobookshelfError, AudiobookshelfFailureClass,
    AudiobookshelfPlaybackProgress, AudiobookshelfSourceMethod,
};
use crate::config::AudiobookshelfSetup;
use crate::service_runtime::SetupGeneration;

/// In-process Audiobookshelf access owned by a Player. The credential is
/// intentionally absent from Debug and every serializable boundary.
#[derive(Clone)]
pub struct AudiobookshelfPlayerContext {
    generation: SetupGeneration,
    setup: AudiobookshelfSetup,
    credential: String,
    device_id: String,
}

impl AudiobookshelfPlayerContext {
    pub fn new(
        generation: SetupGeneration,
        setup: AudiobookshelfSetup,
        credential: String,
        device_id: String,
    ) -> Option<Self> {
        (!setup.server_url.is_empty()
            && !credential.trim().is_empty()
            && !device_id.trim().is_empty())
        .then_some(Self {
            generation,
            setup,
            credential,
            device_id,
        })
    }

    pub const fn generation(&self) -> SetupGeneration {
        self.generation
    }
}

struct AudiobookshelfLifecycle {
    client: AudiobookshelfClient,
    credential: String,
    session_id: String,
    duration: f64,
    last_valid_pos: f64,
    closed: bool,
}

impl AudiobookshelfLifecycle {
    fn close(&mut self, current_time: f64) {
        if self.closed {
            return;
        }
        self.last_valid_pos = current_time.max(0.0);
        let progress = AudiobookshelfPlaybackProgress {
            current_time: self.last_valid_pos,
            time_listened: 0.0,
            duration: self.duration,
        };
        let _ = self.client.close_playback_session_bounded(
            &self.credential,
            &self.session_id,
            progress,
            AudiobookshelfClient::REQUEST_HARD_BOUND,
        );
        self.closed = true;
    }
}

impl Drop for AudiobookshelfLifecycle {
    fn drop(&mut self) {
        self.close(self.last_valid_pos);
    }
}

pub(crate) struct PreparedSource {
    pub(crate) url: String,
    pub(crate) mpv_options: Vec<String>,
    pub(crate) start_seconds: f64,
    lifecycle: Option<AudiobookshelfLifecycle>,
}

impl PreparedSource {
    fn plain(item: &QueueItem, server_url: &str, token: &str) -> Self {
        Self {
            url: mpv_url_for_queue_item(item, server_url, token),
            mpv_options: Vec::new(),
            start_seconds: resume_start_pos(item),
            lifecycle: None,
        }
    }

    fn mpv_load_options(&self, item: &QueueItem) -> String {
        let mut options = vec![mpv_title_opt(&item.display_name())];
        if self.start_seconds > 0.0 {
            options.push(format!("start={}", self.start_seconds));
        }
        options.extend(self.mpv_options.iter().cloned());
        options.join(",")
    }

    fn close(&mut self, current_time: f64) {
        if let Some(lifecycle) = self.lifecycle.as_mut() {
            lifecycle.close(current_time);
        }
        self.lifecycle = None;
    }

    fn has_sensitive_lifecycle(&self) -> bool {
        self.lifecycle.is_some()
    }
}

fn prepare_source(
    item: &QueueItem,
    server_url: &str,
    token: &str,
    context: Option<&AudiobookshelfPlayerContext>,
) -> Result<PreparedSource, AudiobookshelfError> {
    let QueueItem::Audiobookshelf(episode) = item else {
        return Ok(PreparedSource::plain(item, server_url, token));
    };
    let context = context
        .ok_or_else(|| AudiobookshelfError::from_class(AudiobookshelfFailureClass::Unavailable))?;
    let client = AudiobookshelfClient::new(&context.setup.server_url)?;
    let session = client.create_playback_session_bounded(
        &context.credential,
        &context.device_id,
        &episode.library_item_id,
        &episode.episode_id,
        false,
        AudiobookshelfClient::REQUEST_HARD_BOUND,
    )?;
    let mut prepared = PreparedSource {
        url: session.source.url,
        mpv_options: Vec::new(),
        start_seconds: session.current_time_seconds,
        lifecycle: Some(AudiobookshelfLifecycle {
            client: client.clone(),
            credential: context.credential.clone(),
            session_id: session.id,
            duration: session.duration_seconds,
            last_valid_pos: session.current_time_seconds,
            closed: false,
        }),
    };
    match session.source.method {
        AudiobookshelfSourceMethod::Direct => {
            let header = format!("Authorization: Bearer {}", context.credential);
            prepared
                .mpv_options
                .push(format!("http-header-fields=%{}%{header}", header.len()));
        }
        AudiobookshelfSourceMethod::Hls => {
            if let Err(error) = client
                .wait_for_hls_ready_bounded(&prepared.url, AudiobookshelfClient::HLS_READY_BOUND)
            {
                prepared.close(0.0);
                return Err(error);
            }
        }
    }
    Ok(prepared)
}

#[cfg(test)]
mod source_tests {
    use super::*;
    use crate::playback_queue::{AudiobookshelfQueueItem, FeedEntry};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn serve(body: String) -> (String, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let read = stream.read(&mut chunk).unwrap();
                bytes.extend_from_slice(&chunk[..read]);
                let text = String::from_utf8_lossy(&bytes);
                let Some(end) = text.find("\r\n\r\n") else {
                    continue;
                };
                let length = text[..end]
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            let _ = tx.send(request);
        });
        (format!("http://{address}"), rx)
    }

    fn serve_close(current_time: f64) -> (String, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for body in [
                fixture("play-direct.json")
                    .replace("<LIBRARY_ITEM_ID>", "show")
                    .replace("<EPISODE_ID>", "episode")
                    .replace("<DIRECT_PATH>", "/direct.mp3")
                    .replace(
                        "\"currentTime\": 1",
                        &format!("\"currentTime\": {current_time}"),
                    ),
                fixture("session-close.json"),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0; 4096];
                loop {
                    let read = stream.read(&mut chunk).unwrap();
                    bytes.extend_from_slice(&chunk[..read]);
                    let text = String::from_utf8_lossy(&bytes);
                    let Some(end) = text.find("\r\n\r\n") else {
                        continue;
                    };
                    let length = text[..end]
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|value| value.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
                let request = String::from_utf8(bytes).unwrap();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                tx.send(request).unwrap();
            }
        });
        (format!("http://{address}"), rx)
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/audiobookshelf/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn episode() -> QueueItem {
        QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
            library_item_id: "show".into(),
            episode_id: "episode".into(),
            title: "Title".into(),
            show_title: None,
            author: None,
            duration_ticks: Some(100),
            position_ticks: 99,
            played: true,
            pub_date_secs: None,
            is_finished: true,
            cover_path: None,
        })
    }

    #[test]
    fn direct_preparation_uses_authoritative_resume_and_per_file_bearer_only() {
        for current_time in [3.5, 0.0] {
            let fixture = std::fs::read_to_string(format!(
                "{}/tests/fixtures/audiobookshelf/play-direct.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap()
            .replace("<LIBRARY_ITEM_ID>", "show")
            .replace("<EPISODE_ID>", "episode")
            .replace("<DIRECT_PATH>", "/direct.mp3")
            .replace(
                "\"currentTime\": 1",
                &format!("\"currentTime\": {current_time}"),
            );
            let (base, _) = serve(fixture);
            let context = AudiobookshelfPlayerContext::new(
                SetupGeneration::new(7),
                AudiobookshelfSetup::new(base),
                "secret".into(),
                "device".into(),
            )
            .unwrap();
            let mut prepared = prepare_source(&episode(), "", "", Some(&context)).unwrap();
            assert_eq!(prepared.start_seconds, current_time);
            assert_eq!(prepared.mpv_options.len(), 1);
            assert!(prepared.mpv_options[0].contains("Authorization: Bearer secret"));
            prepared.lifecycle = None;
        }

        let feed = QueueItem::Feed(FeedEntry {
            guid: "feed".into(),
            title: "Feed".into(),
            enclosure_url: Some("https://feed.test/file".into()),
            link: None,
            mime_type: Some("audio/mpeg".into()),
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: Some("feed".into()),
            position_ticks: 0,
            played: false,
        });
        let prepared = prepare_source(&feed, "", "", None).unwrap();
        assert!(prepared.mpv_options.is_empty());
        assert!(!prepared.mpv_load_options(&feed).contains("Authorization"));
    }

    #[test]
    fn lifecycle_close_loopback_captures_selected_position_and_resume_on_drop() {
        for (fixture_time, close_position) in [(1.0, Some(3054.336)), (42.0, None)] {
            let (base, requests) = serve_close(fixture_time);
            let context = AudiobookshelfPlayerContext::new(
                SetupGeneration::new(8),
                AudiobookshelfSetup::new(base),
                "secret".into(),
                "device".into(),
            )
            .unwrap();
            let mut prepared = prepare_source(&episode(), "", "", Some(&context)).unwrap();
            let expected = if let Some(position) = close_position {
                prepared.close(position);
                position
            } else {
                drop(prepared);
                fixture_time
            };
            let _create = requests.recv().unwrap();
            let close = requests.recv().unwrap();
            let body = close.split("\r\n\r\n").nth(1).unwrap();
            assert!(
                body.contains(&format!("\"currentTime\":{expected}")),
                "{body}"
            );
        }
    }
}
