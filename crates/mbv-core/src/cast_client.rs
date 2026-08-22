// A synchronous Google Cast sender, matching mbv-core's blocking style
// (ureq, no tokio). Connect/launch/load/transport/status are verified
// against real Shield Android TV receivers; see design.md Risks for the
// rust_cast 0.21 gaps this client works around (no wire support for
// subtitle tracks or a queue-jump message; heartbeat requires an explicit
// keep-alive pump).

use rust_cast::channels::media::{
    Media, MediaQueue, PlayerState as CastPlayerState, QueueItem, QueueType, StatusEntry,
    StreamType,
};
use rust_cast::channels::receiver::CastDeviceApp;
use rust_cast::CastDevice;

const RECEIVER_PLATFORM_ID: &str = "receiver-0";

/// A dispatched-and-forgotten cast session: mbv connects, launches the
/// default media receiver, loads items, and controls transport. The
/// receiver owns its queue once loaded (see design.md "receiver owns its
/// queue").
pub struct CastClient {
    device: CastDevice<'static>,
    transport_id: String,
    session_id: String,
    media_session_id: Option<i32>,
    dispatched_queue: Option<DispatchedQueue>,
}

/// The last queue this client loaded, kept only so `next`/`previous` can
/// replay it with a shifted `start_index` -- not a projection of mbv's
/// queue, and never reissued except on an explicit transport action.
struct DispatchedQueue {
    queue: MediaQueue,
    current_index: u16,
}

/// One item to dispatch to the receiver's queue.
#[derive(Debug)]
pub struct CastMediaItem {
    pub url: String,
    pub content_type: String,
}

/// The receiver's reported playback state, decoupled from rust_cast's own
/// types so callers outside this module don't take on that dependency.
#[derive(Debug, Clone, PartialEq)]
pub struct CastStatus {
    pub position_seconds: Option<f32>,
    pub duration_seconds: Option<f32>,
    pub playback_rate: f32,
    pub state: CastPlaybackState,
    /// The URL of the currently loaded item, when the receiver reports one.
    pub playing_content_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastPlaybackState {
    Idle,
    Playing,
    Buffering,
    Paused,
}

impl CastClient {
    /// Connects to a receiver at `host:port` and launches the default media
    /// receiver app. Returns an error rather than panicking on any failure
    /// (connection, app launch, or app-transport handshake).
    pub fn connect(host: &str, port: u16) -> Result<Self, String> {
        let device = CastDevice::connect_without_host_verification(host.to_string(), port)
            .map_err(|e| format!("cast connect failed: {e}"))?;
        device
            .connection
            .connect(RECEIVER_PLATFORM_ID)
            .map_err(|e| format!("cast receiver-platform connect failed: {e}"))?;
        let app = device
            .receiver
            .launch_app(&CastDeviceApp::DefaultMediaReceiver)
            .map_err(|e| format!("cast launch_app failed: {e}"))?;
        device
            .connection
            .connect(app.transport_id.as_str())
            .map_err(|e| format!("cast app-transport connect failed: {e}"))?;
        Ok(Self {
            device,
            transport_id: app.transport_id,
            session_id: app.session_id,
            media_session_id: None,
            dispatched_queue: None,
        })
    }

    /// Stops the launched receiver app. Never called on mbv's own exit while
    /// attached (design.md "exit orphans the session") -- this is for
    /// explicit teardown paths only.
    pub fn teardown(&self) -> Result<(), String> {
        self.device
            .receiver
            .stop_app(self.session_id.as_str())
            .map_err(|e| format!("cast teardown failed: {e}"))
            .map(|_| ())
    }

    /// Answers any heartbeat PING the receiver has sent since the last call
    /// with a PONG, keeping the connection alive. A real device drops an
    /// unanswered sender within roughly a minute; callers on a status-poll
    /// cadence (design.md targets 5-10s) should call this every tick.
    pub fn keep_alive(&self) -> Result<(), String> {
        self.device
            .heartbeat
            .pong()
            .map_err(|e| format!("cast keep_alive failed: {e}"))
    }

    pub fn load(
        &mut self,
        item: &CastMediaItem,
        start_position_seconds: f64,
    ) -> Result<(), String> {
        let media = build_media(&item.url, &item.content_type);
        let status = self
            .device
            .media
            .load_with_opts(
                self.transport_id.as_str(),
                self.session_id.as_str(),
                &media,
                rust_cast::channels::media::LoadOptions {
                    current_time: start_position_seconds,
                    autoplay: true,
                },
            )
            .map_err(|e| format!("cast load failed: {e}"))?;
        self.adopt_status(&status);
        self.dispatched_queue = None;
        Ok(())
    }

    pub fn load_queue(&mut self, items: &[CastMediaItem], start_index: u16) -> Result<(), String> {
        let queue = build_queue(items, start_index);
        let status = self
            .device
            .media
            .load_queue(self.transport_id.as_str(), self.session_id.as_str(), &queue)
            .map_err(|e| format!("cast load_queue failed: {e}"))?;
        self.adopt_status(&status);
        self.dispatched_queue = Some(DispatchedQueue {
            queue,
            current_index: start_index,
        });
        Ok(())
    }

    pub fn play(&mut self) -> Result<(), String> {
        let id = self.require_media_session_id()?;
        let entry = self
            .device
            .media
            .play(self.transport_id.as_str(), id)
            .map_err(|e| format!("cast play failed: {e}"))?;
        self.adopt_status_entry(&entry);
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), String> {
        let id = self.require_media_session_id()?;
        let entry = self
            .device
            .media
            .pause(self.transport_id.as_str(), id)
            .map_err(|e| format!("cast pause failed: {e}"))?;
        self.adopt_status_entry(&entry);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        let id = self.require_media_session_id()?;
        let entry = self
            .device
            .media
            .stop(self.transport_id.as_str(), id)
            .map_err(|e| format!("cast stop failed: {e}"))?;
        self.adopt_status_entry(&entry);
        self.dispatched_queue = None;
        Ok(())
    }

    pub fn seek(&mut self, position_seconds: f32) -> Result<(), String> {
        let id = self.require_media_session_id()?;
        let entry = self
            .device
            .media
            .seek(self.transport_id.as_str(), id, Some(position_seconds), None)
            .map_err(|e| format!("cast seek failed: {e}"))?;
        self.adopt_status_entry(&entry);
        Ok(())
    }

    /// Advances to the next entry of the queue this client last loaded, by
    /// reloading it with a shifted `start_index`. rust_cast 0.21 has no
    /// QUEUE_UPDATE/jump message; see design.md Risks.
    pub fn skip_next(&mut self) -> Result<(), String> {
        self.jump(1)
    }

    pub fn skip_previous(&mut self) -> Result<(), String> {
        self.jump(-1)
    }

    fn jump(&mut self, delta: i32) -> Result<(), String> {
        let Some(dispatched) = &self.dispatched_queue else {
            return Err("cast jump failed: no dispatched queue to advance".to_string());
        };
        let Some(next_index) = next_queue_index(
            dispatched.current_index,
            dispatched.queue.items.len(),
            delta,
        ) else {
            return Err("cast jump failed: no adjacent queue item".to_string());
        };
        let queue = dispatched.queue.clone();
        let status = self
            .device
            .media
            .load_queue(self.transport_id.as_str(), self.session_id.as_str(), &queue)
            .map_err(|e| format!("cast jump failed: {e}"))?;
        self.adopt_status(&status);
        if let Some(dispatched) = &mut self.dispatched_queue {
            dispatched.current_index = next_index;
        }
        Ok(())
    }

    pub fn set_volume(&self, level: f32) -> Result<(), String> {
        self.device
            .receiver
            .set_volume(level)
            .map_err(|e| format!("cast set_volume failed: {e}"))
            .map(|_| ())
    }

    pub fn set_muted(&self, muted: bool) -> Result<(), String> {
        self.device
            .receiver
            .set_volume(muted)
            .map_err(|e| format!("cast set_muted failed: {e}"))
            .map(|_| ())
    }

    pub fn status(&mut self) -> Result<CastStatus, String> {
        let status = self
            .device
            .media
            .get_status(self.transport_id.as_str(), self.media_session_id)
            .map_err(|e| format!("cast get_status failed: {e}"))?;
        let entry = status
            .entries
            .first()
            .ok_or_else(|| "cast get_status returned no entries".to_string())?;
        self.adopt_status_entry(entry);
        Ok(cast_status_from_entry(entry))
    }

    fn adopt_status(&mut self, status: &rust_cast::channels::media::Status) {
        if let Some(entry) = status.entries.first() {
            self.adopt_status_entry(entry);
        }
    }

    fn adopt_status_entry(&mut self, entry: &StatusEntry) {
        self.media_session_id = Some(entry.media_session_id);
    }

    fn require_media_session_id(&self) -> Result<i32, String> {
        self.media_session_id
            .ok_or_else(|| "cast command failed: nothing loaded yet".to_string())
    }
}

fn build_media(url: &str, content_type: &str) -> Media {
    Media {
        content_id: url.to_string(),
        stream_type: StreamType::Buffered,
        content_type: content_type.to_string(),
        metadata: None,
        duration: None,
    }
}

fn build_queue(items: &[CastMediaItem], start_index: u16) -> MediaQueue {
    MediaQueue {
        items: items
            .iter()
            .map(|item| QueueItem {
                media: build_media(&item.url, &item.content_type),
            })
            .collect(),
        start_index,
        queue_type: QueueType::Playlist,
    }
}

/// Pure index arithmetic for `next`/`previous`, kept separate from the
/// network call so it is unit-testable without a device.
fn next_queue_index(current: u16, len: usize, delta: i32) -> Option<u16> {
    let len = u16::try_from(len).ok()?;
    let target = i32::from(current) + delta;
    if target < 0 || target >= i32::from(len) {
        return None;
    }
    u16::try_from(target).ok()
}

fn cast_status_from_entry(entry: &StatusEntry) -> CastStatus {
    CastStatus {
        position_seconds: entry.current_time,
        duration_seconds: entry.media.as_ref().and_then(|m| m.duration),
        playback_rate: entry.playback_rate,
        state: cast_playback_state(entry.player_state),
        playing_content_id: entry.media.as_ref().map(|m| m.content_id.clone()),
    }
}

fn cast_playback_state(state: CastPlayerState) -> CastPlaybackState {
    match state {
        CastPlayerState::Idle => CastPlaybackState::Idle,
        CastPlayerState::Playing => CastPlaybackState::Playing,
        CastPlayerState::Buffering => CastPlaybackState::Buffering,
        CastPlayerState::Paused => CastPlaybackState::Paused,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_failure_returns_error_not_panic() {
        // Port 0 on loopback refuses immediately; no real device required.
        let result = CastClient::connect("127.0.0.1", 1);
        assert!(result.is_err());
    }

    #[test]
    fn build_media_sets_url_and_content_type() {
        let media = build_media("http://host/stream.mp4", "video/mp4");
        assert_eq!(media.content_id, "http://host/stream.mp4");
        assert_eq!(media.content_type, "video/mp4");
        assert_eq!(media.stream_type, StreamType::Buffered);
    }

    #[test]
    fn build_queue_carries_start_index_and_items_in_order() {
        let items = vec![
            CastMediaItem {
                url: "http://a".to_string(),
                content_type: "video/mp4".to_string(),
            },
            CastMediaItem {
                url: "http://b".to_string(),
                content_type: "video/mp4".to_string(),
            },
        ];
        let queue = build_queue(&items, 1);
        assert_eq!(queue.start_index, 1);
        assert_eq!(queue.items.len(), 2);
        assert_eq!(queue.items[0].media.content_id, "http://a");
        assert_eq!(queue.items[1].media.content_id, "http://b");
        assert_eq!(queue.queue_type, QueueType::Playlist);
    }

    #[test]
    fn next_queue_index_advances_within_bounds() {
        assert_eq!(next_queue_index(0, 3, 1), Some(1));
        assert_eq!(next_queue_index(2, 3, 1), None);
    }

    #[test]
    fn next_queue_index_retreats_within_bounds() {
        assert_eq!(next_queue_index(1, 3, -1), Some(0));
        assert_eq!(next_queue_index(0, 3, -1), None);
    }

    // Real payload shape captured against a Shield Android TV during the
    // task 1.3 spike (see design.md Risks).
    fn recorded_playing_entry() -> StatusEntry {
        StatusEntry {
            media_session_id: 4,
            media: Some(Media {
                content_id: "http://server/Videos/492984/stream?static=true&api_key=redacted"
                    .to_string(),
                stream_type: StreamType::Buffered,
                content_type: "video/mp4".to_string(),
                metadata: None,
                duration: Some(2705.045),
            }),
            playback_rate: 1.0,
            player_state: CastPlayerState::Playing,
            current_item_id: Some(6),
            loading_item_id: None,
            preloaded_item_id: None,
            idle_reason: None,
            extended_status: None,
            current_time: Some(64.384_93),
            supported_media_commands: 274_447,
        }
    }

    #[test]
    fn cast_status_from_entry_maps_recorded_playing_payload() {
        let status = cast_status_from_entry(&recorded_playing_entry());
        assert_eq!(status.position_seconds, Some(64.384_93));
        assert_eq!(status.duration_seconds, Some(2705.045));
        assert_eq!(status.playback_rate, 1.0);
        assert_eq!(status.state, CastPlaybackState::Playing);
        assert_eq!(
            status.playing_content_id.as_deref(),
            Some("http://server/Videos/492984/stream?static=true&api_key=redacted")
        );
    }

    #[test]
    fn cast_status_from_entry_maps_idle_payload() {
        let mut entry = recorded_playing_entry();
        entry.player_state = CastPlayerState::Idle;
        entry.media = None;
        entry.current_time = None;
        let status = cast_status_from_entry(&entry);
        assert_eq!(status.state, CastPlaybackState::Idle);
        assert_eq!(status.playing_content_id, None);
        assert_eq!(status.duration_seconds, None);
    }
}
