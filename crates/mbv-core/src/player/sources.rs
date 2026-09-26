use super::{
    mpv_title_opt, mpv_url_for_queue_item, resume_start_pos, saturating_i64_from_f64,
    AudiobookshelfBookPlaybackLifecycle, AudiobookshelfPlaybackLifecycle, PreparedLifecycle,
};
use crate::audiobookshelf::{
    AudiobookshelfAudioSource, AudiobookshelfClient, AudiobookshelfError,
    AudiobookshelfFailureClass, AudiobookshelfSourceMethod,
};
use crate::config::AudiobookshelfSetup;
use crate::playback_queue::{AudiobookshelfBookQueueItem, AudiobookshelfQueueItem};
use crate::playback_queue::{AudiobookshelfItem, QueueItem};
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
/// redacted from Debug and absent from every serializable boundary.
#[derive(Clone)]
pub struct AudiobookshelfPlayerContext {
    generation: SetupGeneration,
    setup: AudiobookshelfSetup,
    credential: String,
    device_id: String,
    progress_updates: Option<std::sync::mpsc::Sender<AudiobookshelfProgressUpdate>>,
    book_progress_updates: Option<std::sync::mpsc::Sender<AudiobookshelfBookProgressUpdate>>,
}

impl std::fmt::Debug for AudiobookshelfPlayerContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudiobookshelfPlayerContext")
            .field("generation", &self.generation)
            .field("setup", &self.setup)
            .field("device_id", &self.device_id)
            .field("credential", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl AudiobookshelfPlayerContext {
    #[must_use]
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

    #[must_use]
    pub fn with_progress_updates(
        mut self,
        sender: std::sync::mpsc::Sender<AudiobookshelfProgressUpdate>,
    ) -> Self {
        self.progress_updates = Some(sender);
        self
    }

    #[must_use]
    pub fn with_book_progress_updates(
        mut self,
        sender: std::sync::mpsc::Sender<AudiobookshelfBookProgressUpdate>,
    ) -> Self {
        self.book_progress_updates = Some(sender);
        self
    }

    #[must_use]
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
    pub(super) book_extra_sources: Vec<AudiobookshelfAudioSource>,
    /// Whether this source is projected as a merged multi-file timeline
    /// (books) rather than a single `loadfile`.
    pub(super) merged_timeline: bool,
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
    pub(super) fn mpv_load_options(&self, item: &QueueItem) -> String {
        let mut options = vec![mpv_title_opt(&item.display_name())];
        if self.start_seconds > 0.0 && !self.merged_timeline {
            options.push(format!("start={}", self.start_seconds));
        }
        options.extend(self.mpv_options.iter().cloned());
        options.join(",")
    }

    /// Header-only options for appended merged-timeline sources (no title,
    /// no resume start).
    pub(super) fn extra_source_options(&self) -> String {
        self.mpv_options.join(",")
    }

    pub(super) fn close(&mut self, current_time: f64) {
        if let Some(lifecycle) = self.lifecycle.as_mut() {
            #[expect(
                clippy::cast_precision_loss,
                reason = "mpv current time seconds → ticks through f64; no lossless integer-path conversion exists (approved, issue #804)"
            )]
            lifecycle.close(saturating_i64_from_f64(
                current_time.max(0.0) * crate::api::TICKS_PER_SECOND as f64,
            ));
        }
        self.lifecycle = None;
    }

    pub(super) fn take_lifecycle(&mut self) -> Option<PreparedLifecycle> {
        self.lifecycle.take()
    }

    pub(super) fn has_sensitive_lifecycle(&self) -> bool {
        self.lifecycle.is_some()
    }
}

pub(super) fn prepare_source(
    item: &QueueItem,
    server_url: &str,
    token: &str,
    context: Option<&AudiobookshelfPlayerContext>,
) -> Result<PreparedSource, AudiobookshelfError> {
    match item {
        QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(episode)) => {
            prepare_episode_source(episode, context)
        }
        QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book)) => {
            prepare_book_source(book, context)
        }
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
                Some(episode.episode_id.clone()),
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
                None,
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
mod tests {
    use super::*;

    #[test]
    fn player_context_debug_redacts_credential() {
        let Some(context) = AudiobookshelfPlayerContext::new(
            SetupGeneration::default(),
            AudiobookshelfSetup::new("http://abs:13378"),
            "abs-secret-credential".to_string(),
            "device-1".to_string(),
        ) else {
            panic!("valid context");
        };
        let rendered = format!("{context:?}");
        assert!(rendered.contains("AudiobookshelfPlayerContext"));
        assert!(!rendered.contains("abs-secret-credential"));
    }
}
