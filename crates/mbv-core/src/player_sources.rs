use crate::audiobookshelf::{
    AudiobookshelfAudioSource, AudiobookshelfClient, AudiobookshelfError,
    AudiobookshelfFailureClass, AudiobookshelfSourceMethod,
};
use crate::config::AudiobookshelfSetup;
use crate::playback_queue::{AudiobookshelfBookQueueItem, AudiobookshelfQueueItem};
use crate::service_runtime::SetupGeneration;

#[derive(Clone, Debug, PartialEq)]
pub struct AudiobookshelfProgressUpdate {
    pub generation: SetupGeneration,
    pub library_item_id: String,
    pub episode_id: String,
    pub current_time_seconds: f64,
    pub duration_seconds: f64,
    pub is_finished: bool,
}

/// Book-shaped progress, keyed by `library_item_id` only (no episode
/// identity). Mirrors `AudiobookshelfProgressUpdate` but never carries an
/// `episode_id`, so a book update can't be matched against an episode slot.
#[derive(Clone, Debug, PartialEq)]
pub struct AudiobookshelfBookProgressUpdate {
    pub generation: SetupGeneration,
    pub library_item_id: String,
    pub current_time_seconds: f64,
    pub duration_seconds: f64,
    pub is_finished: bool,
}

/// In-process Audiobookshelf access owned by a Player. The credential is
/// intentionally absent from Debug and every serializable boundary.
#[derive(Clone)]
pub struct AudiobookshelfPlayerContext {
    generation: SetupGeneration,
    setup: AudiobookshelfSetup,
    credential: String,
    device_id: String,
    progress_updates: Option<std::sync::mpsc::Sender<AudiobookshelfProgressUpdate>>,
    book_progress_updates: Option<std::sync::mpsc::Sender<AudiobookshelfBookProgressUpdate>>,
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
            progress_updates: None,
            book_progress_updates: None,
        })
    }

    pub fn with_progress_updates(
        mut self,
        sender: std::sync::mpsc::Sender<AudiobookshelfProgressUpdate>,
    ) -> Self {
        self.progress_updates = Some(sender);
        self
    }

    pub fn with_book_progress_updates(
        mut self,
        sender: std::sync::mpsc::Sender<AudiobookshelfBookProgressUpdate>,
    ) -> Self {
        self.book_progress_updates = Some(sender);
        self
    }

    pub const fn generation(&self) -> SetupGeneration {
        self.generation
    }
}

pub(crate) struct PreparedSource {
    pub(crate) url: String,
    /// Options applied to every source `loadfile` (the per-file bearer header
    /// for direct Audiobookshelf sources); empty for plain Emby/Feed sources.
    pub(crate) mpv_options: Vec<String>,
    pub(crate) start_seconds: f64,
    /// Remaining sources appended after `url` to project a book's audio files
    /// as one continuous merged timeline.
    book_extra_sources: Vec<AudiobookshelfAudioSource>,
    /// Whether this source is projected as a merged multi-file timeline
    /// (books) rather than a single `loadfile`.
    merged_timeline: bool,
    lifecycle: Option<PreparedLifecycle>,
}

impl PreparedSource {
    fn plain(item: &QueueItem, server_url: &str, token: &str) -> Self {
        Self {
            url: mpv_url_for_queue_item(item, server_url, token),
            mpv_options: Vec::new(),
            start_seconds: resume_start_pos(item),
            book_extra_sources: Vec::new(),
            merged_timeline: false,
            lifecycle: None,
        }
    }

    /// Options for the first source load (title, resume start, per-file
    /// header). For a merged timeline the resume start is omitted here and
    /// applied as an absolute seek after loading, so the position is
    /// unambiguous across the whole book.
    fn mpv_load_options(&self, item: &QueueItem) -> String {
        let mut options = vec![mpv_title_opt(&item.display_name())];
        if self.start_seconds > 0.0 && !self.merged_timeline {
            options.push(format!("start={}", self.start_seconds));
        }
        options.extend(self.mpv_options.iter().cloned());
        options.join(",")
    }

    /// Header-only options for appended merged-timeline sources (no title,
    /// no resume start).
    fn extra_source_options(&self) -> String {
        self.mpv_options.join(",")
    }

    fn close(&mut self, current_time: f64) {
        if let Some(lifecycle) = self.lifecycle.as_mut() {
            lifecycle.close((current_time.max(0.0) * crate::api::TICKS_PER_SECOND as f64) as i64);
        }
        self.lifecycle = None;
    }

    fn take_lifecycle(&mut self) -> Option<PreparedLifecycle> {
        self.lifecycle.take()
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
    match item {
        QueueItem::Audiobookshelf(episode) => prepare_episode_source(episode, context),
        QueueItem::AudiobookshelfBook(book) => prepare_book_source(book, context),
        _ => Ok(PreparedSource::plain(item, server_url, token)),
    }
}

fn prepare_episode_source(
    episode: &AudiobookshelfQueueItem,
    context: Option<&AudiobookshelfPlayerContext>,
) -> Result<PreparedSource, AudiobookshelfError> {
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
        book_extra_sources: Vec::new(),
        merged_timeline: false,
        lifecycle: Some(PreparedLifecycle::Episode(
            AudiobookshelfPlaybackLifecycle::new(
                context.generation,
                client.clone(),
                context.credential.clone(),
                session.id,
                episode.library_item_id.clone(),
                episode.episode_id.clone(),
                session.current_time_seconds,
                session.duration_seconds,
                context.progress_updates.clone(),
            ),
        )),
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

fn prepare_book_source(
    book: &AudiobookshelfBookQueueItem,
    context: Option<&AudiobookshelfPlayerContext>,
) -> Result<PreparedSource, AudiobookshelfError> {
    let context = context
        .ok_or_else(|| AudiobookshelfError::from_class(AudiobookshelfFailureClass::Unavailable))?;
    let client = AudiobookshelfClient::new(&context.setup.server_url)?;
    let session = client.create_book_playback_session_bounded(
        &context.credential,
        &context.device_id,
        &book.library_item_id,
        false,
        AudiobookshelfClient::REQUEST_HARD_BOUND,
    )?;
    let mut sources = session.sources;
    let first = sources
        .drain(..1)
        .next()
        .ok_or_else(|| AudiobookshelfError::from_class(AudiobookshelfFailureClass::Protocol))?;
    let first_method = first.method;
    let mut prepared = PreparedSource {
        url: first.url,
        mpv_options: Vec::new(),
        start_seconds: session.current_time_seconds,
        book_extra_sources: sources,
        merged_timeline: true,
        lifecycle: Some(PreparedLifecycle::Book(
            AudiobookshelfBookPlaybackLifecycle::new(
                context.generation,
                client.clone(),
                context.credential.clone(),
                session.id,
                book.library_item_id.clone(),
                session.current_time_seconds,
                session.duration_seconds,
                context.book_progress_updates.clone(),
            ),
        )),
    };
    match first_method {
        AudiobookshelfSourceMethod::Direct => {
            let header = format!("Authorization: Bearer {}", context.credential);
            prepared
                .mpv_options
                .push(format!("http-header-fields=%{}%{header}", header.len()));
        }
        AudiobookshelfSourceMethod::Hls => {
            for source in std::iter::once(&prepared.url)
                .chain(prepared.book_extra_sources.iter().map(|s| &s.url))
            {
                if let Err(error) =
                    client.wait_for_hls_ready_bounded(source, AudiobookshelfClient::HLS_READY_BOUND)
                {
                    prepared.close(0.0);
                    return Err(error);
                }
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

    pub(super) fn serve_close(current_time: f64) -> (String, std::sync::mpsc::Receiver<String>) {
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
                fixture("session-sync.json"),
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
    fn progress_sender_builder_stays_owner_local() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let context = AudiobookshelfPlayerContext::new(
            SetupGeneration::new(7),
            AudiobookshelfSetup::new("http://server"),
            "secret".into(),
            "device".into(),
        )
        .unwrap()
        .with_progress_updates(sender);

        assert!(context.progress_updates.is_some());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn successful_bounded_sync_emits_progress_update() {
        let (base, requests) = serve_close(1.0);
        let (sender, updates) = std::sync::mpsc::channel();
        let context = AudiobookshelfPlayerContext::new(
            SetupGeneration::new(8),
            AudiobookshelfSetup::new(base),
            "secret".into(),
            "device".into(),
        )
        .unwrap()
        .with_progress_updates(sender);
        let mut prepared = prepare_source(&episode(), "", "", Some(&context)).unwrap();
        let duration = match prepared.lifecycle.as_ref().unwrap() {
            PreparedLifecycle::Episode(lifecycle) => lifecycle.duration,
            PreparedLifecycle::Book(_) => unreachable!("episode fixture"),
        };

        prepared.close(duration);

        let _create = requests.recv().unwrap();
        let _sync = requests.recv().unwrap();
        let _close = requests.recv().unwrap();
        let update = updates.recv().unwrap();
        assert_eq!(update.generation, SetupGeneration::new(8));
        assert_eq!(update.library_item_id, "show");
        assert_eq!(update.episode_id, "episode");
        assert!((update.current_time_seconds - duration).abs() < 0.000001);
        assert_eq!(update.duration_seconds, duration);
        assert!(update.is_finished);
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
            let sync = requests.recv().unwrap();
            let close = requests.recv().unwrap();
            let body = sync.split("\r\n\r\n").nth(1).unwrap();
            assert!(
                body.contains(&format!("\"currentTime\":{expected}")),
                "{body}"
            );
            assert!(close.starts_with("POST /api/session/%3CSESSION_ID%3E/close HTTP/1.1"));
        }
    }
}
