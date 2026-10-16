// A synchronous Google Cast sender, matching mbv-core's blocking style
// (ureq, no tokio). Connect/launch/load/transport/status are verified
// against real Shield Android TV receivers; see design.md Risks for the
// rust_cast 0.21 gaps this client works around (no wire support for
// subtitle tracks; heartbeat requires an explicit
// keep-alive pump).

use crate::CastError;
use crate::error::TransportKind;
use rust_cast::CastDevice;
use rust_cast::channels::media::{
    IdleReason, Media, PlayerState as CastPlayerState, StatusEntry, StreamType,
};
use rust_cast::channels::receiver::{Application, CastDeviceApp};
use rust_cast::errors::Error as RustCastError;
// Trait only, for `Error::source()` in the LOADING parse-error match.
use std::error::Error as _;

const RECEIVER_PLATFORM_ID: &str = "receiver-0";

/// A dispatched-and-forgotten cast session: mbv connects, launches the
/// default media receiver, loads items, and controls transport.
pub struct CastClient {
    device: CastDevice<'static>,
    transport_id: String,
    session_id: String,
    media_session_id: Option<i32>,
    /// The URL of the last item `load` accepted, kept so a `LOADING`-report
    /// status (design D7) can still name the loaded content id.
    loaded_content_id: Option<String>,
}

impl std::fmt::Debug for CastClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `device` (rust_cast) has no Debug.
        f.debug_struct("CastClient")
            .field("transport_id", &self.transport_id)
            .field("session_id", &self.session_id)
            .field("media_session_id", &self.media_session_id)
            .finish_non_exhaustive()
    }
}

/// One item to dispatch to the receiver.
#[derive(Debug)]
pub struct CastMediaItem {
    pub url: String,
    pub content_type: String,
}

/// What `connect` is for: a user's first selection of this receiver, or an
/// automatic reconnect after a connection loss (design D11). The mode only
/// changes what happens when another application is running: a first
/// selection still launches, a reconnect never takes the TV over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectMode {
    FirstSelect,
    Reconnect,
}

/// Why a receiver reports an idle media session (`StatusEntry.idle_reason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastIdleReason {
    Finished,
    Cancelled,
    Interrupted,
    Error,
}

/// The receiver's reported playback state, decoupled from `rust_cast`'s own
/// types so callers outside this module don't take on that dependency.
#[derive(Debug, Clone, PartialEq)]
pub struct CastStatus {
    pub position_seconds: Option<f32>,
    pub duration_seconds: Option<f32>,
    pub playback_rate: f32,
    pub state: CastPlaybackState,
    /// The URL of the currently loaded item, when the receiver reports one.
    pub playing_content_id: Option<String>,
    /// Why the receiver went idle, when it reports one (design D7).
    pub idle_reason: Option<CastIdleReason>,
}

impl CastStatus {
    /// A receiver with no media session: idle, no position, no duration, no
    /// content id.
    #[must_use]
    pub fn idle() -> Self {
        Self {
            position_seconds: None,
            duration_seconds: None,
            playback_rate: 1.0,
            state: CastPlaybackState::Idle,
            playing_content_id: None,
            idle_reason: None,
        }
    }

    /// A load that the receiver accepted and is still fetching media for:
    /// `rust_cast` 0.21 cannot parse the receiver's `LOADING` player state
    /// (design D7), so a load in progress is reported as this status rather
    /// than as an error.
    #[must_use]
    pub fn buffering(content_id: Option<String>) -> Self {
        Self {
            position_seconds: None,
            duration_seconds: None,
            playback_rate: 1.0,
            state: CastPlaybackState::Buffering,
            playing_content_id: content_id,
            idle_reason: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastPlaybackState {
    Idle,
    Playing,
    Buffering,
    Paused,
}

impl CastClient {
    /// Connects to a receiver at `host:port` and either joins the default
    /// media receiver app that is already running there or launches it
    /// (design D11): the decision comes from the receiver's application
    /// list, read before any launch. A reconnect that would have to launch
    /// while another application runs is refused. Returns an error rather
    /// than panicking on any failure (connection, app launch, or
    /// app-transport handshake).
    pub fn connect(host: &str, port: u16, mode: ConnectMode) -> Result<Self, CastError> {
        let device = CastDevice::connect_without_host_verification(host.to_string(), port)
            .map_err(|e| CastError::transport(TransportKind::Connect, e))?;
        device
            .connection
            .connect(RECEIVER_PLATFORM_ID)
            .map_err(|e| CastError::transport(TransportKind::ReceiverPlatformConnect, e))?;
        let status = device
            .receiver
            .get_status()
            .map_err(|e| CastError::transport(TransportKind::ReceiverStatus, e))?;
        match decide_connect(&status.applications, mode) {
            ConnectPlan::Join {
                transport_id,
                session_id,
            } => Self::join_running(device, transport_id, session_id),
            ConnectPlan::Launch => Self::launch(device),
            ConnectPlan::Refuse => Err(CastError::reconnect_refused()),
        }
    }

    /// The no-app-running path: launch the default media receiver and
    /// connect to its new app transport.
    fn launch(device: CastDevice<'static>) -> Result<Self, CastError> {
        let app = device
            .receiver
            .launch_app(&CastDeviceApp::DefaultMediaReceiver)
            .map_err(|e| CastError::transport(TransportKind::LaunchApp, e))?;
        device
            .connection
            .connect(app.transport_id.as_str())
            .map_err(|e| CastError::transport(TransportKind::AppTransportConnect, e))?;
        Ok(Self {
            device,
            transport_id: app.transport_id,
            session_id: app.session_id,
            media_session_id: None,
            loaded_content_id: None,
        })
    }

    /// The default-media-receiver-already-running path (design D11): connect
    /// to the running app's transport and adopt its media session id.
    fn join_running(
        device: CastDevice<'static>,
        transport_id: String,
        session_id: String,
    ) -> Result<Self, CastError> {
        device
            .connection
            .connect(transport_id.as_str())
            .map_err(|e| CastError::transport(TransportKind::AppTransportConnect, e))?;
        let media_session_id = match device.media.get_status(transport_id.as_str(), None) {
            Ok(status) => status.entries.first().map(|entry| entry.media_session_id),
            Err(e) => {
                // A session that is still LOADING is joinable; the media
                // session id is adopted on the first status poll instead
                // (design D7).
                let error = CastError::transport(TransportKind::Status, e);
                if is_loading_parse_error(&error) {
                    None
                } else {
                    return Err(error);
                }
            }
        };
        Ok(Self {
            device,
            transport_id,
            session_id,
            media_session_id,
            loaded_content_id: None,
        })
    }

    /// Stops the launched receiver app. Never called on mbv's own exit while
    /// attached (design.md "exit orphans the session") -- this is for
    /// explicit teardown paths only.
    pub fn teardown(&self) -> Result<(), CastError> {
        self.device
            .receiver
            .stop_app(self.session_id.as_str())
            .map_err(|e| CastError::transport(TransportKind::Teardown, e))
    }

    /// Answers any heartbeat PING the receiver has sent since the last call
    /// with a PONG, keeping the connection alive. A real device drops an
    /// unanswered sender within roughly a minute; callers on a status-poll
    /// cadence (design.md targets 5-10s) should call this every tick.
    pub fn keep_alive(&self) -> Result<(), CastError> {
        self.device
            .heartbeat
            .pong()
            .map_err(|e| CastError::transport(TransportKind::KeepAlive, e))
    }

    pub fn load(
        &mut self,
        item: &CastMediaItem,
        start_position_seconds: f64,
    ) -> Result<(), CastError> {
        let media = build_media(&item.url, &item.content_type);
        match self
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
            .map_err(|e| CastError::transport(TransportKind::Load, e))
        {
            Ok(status) => {
                self.adopt_status(&status);
                self.record_accepted_load(&item.url);
                Ok(())
            }
            Err(error) => {
                // A receiver that accepted the load but is still fetching
                // media answers with a `LOADING` state rust_cast cannot
                // parse (design D7): the load was accepted, not lost.
                if is_loading_parse_error(&error) {
                    self.record_accepted_load(&item.url);
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }

    fn record_accepted_load(&mut self, url: &str) {
        self.loaded_content_id = Some(url.to_string());
    }

    pub fn play(&mut self) -> Result<(), CastError> {
        let id = self.require_media_session_id()?;
        let entry = self
            .device
            .media
            .play(self.transport_id.as_str(), id)
            .map_err(|e| CastError::transport(TransportKind::Play, e))?;
        self.adopt_status_entry(&entry);
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), CastError> {
        let id = self.require_media_session_id()?;
        let entry = self
            .device
            .media
            .pause(self.transport_id.as_str(), id)
            .map_err(|e| CastError::transport(TransportKind::Pause, e))?;
        self.adopt_status_entry(&entry);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), CastError> {
        let id = self.require_media_session_id()?;
        let entry = self
            .device
            .media
            .stop(self.transport_id.as_str(), id)
            .map_err(|e| CastError::transport(TransportKind::Stop, e))?;
        self.adopt_status_entry(&entry);
        Ok(())
    }

    pub fn seek(&mut self, position_seconds: f32) -> Result<(), CastError> {
        let id = self.require_media_session_id()?;
        let entry = self
            .device
            .media
            .seek(self.transport_id.as_str(), id, Some(position_seconds), None)
            .map_err(|e| CastError::transport(TransportKind::Seek, e))?;
        self.adopt_status_entry(&entry);
        Ok(())
    }

    pub fn set_volume(&self, level: f32) -> Result<(), CastError> {
        self.device
            .receiver
            .set_volume(level)
            .map_err(|e| CastError::transport(TransportKind::SetVolume, e))
            .map(|_| ())
    }

    pub fn set_muted(&self, muted: bool) -> Result<(), CastError> {
        self.device
            .receiver
            .set_volume(muted)
            .map_err(|e| CastError::transport(TransportKind::SetMuted, e))
            .map(|_| ())
    }

    pub fn status(&mut self) -> Result<CastStatus, CastError> {
        match self
            .device
            .media
            .get_status(self.transport_id.as_str(), self.media_session_id)
            .map_err(|e| CastError::transport(TransportKind::Status, e))
        {
            Ok(status) => {
                self.adopt_status(&status);
                Ok(status_from_entries(&status.entries))
            }
            Err(error) => {
                // The receiver is reporting a load in progress it cannot be
                // parsed from (design D7): a load in progress is not a
                // disconnect, so it surfaces as a Buffering status for the
                // loaded content id.
                if is_loading_parse_error(&error) {
                    Ok(CastStatus::buffering(self.loaded_content_id.clone()))
                } else {
                    Err(error)
                }
            }
        }
    }

    fn adopt_status(&mut self, status: &rust_cast::channels::media::Status) {
        if let Some(entry) = status.entries.first() {
            self.adopt_status_entry(entry);
        }
    }

    fn adopt_status_entry(&mut self, entry: &StatusEntry) {
        self.media_session_id = Some(entry.media_session_id);
    }

    fn require_media_session_id(&self) -> Result<i32, CastError> {
        self.media_session_id.ok_or_else(CastError::no_session_id)
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

fn cast_status_from_entry(entry: &StatusEntry) -> CastStatus {
    CastStatus {
        position_seconds: entry.current_time,
        duration_seconds: entry.media.as_ref().and_then(|m| m.duration),
        playback_rate: entry.playback_rate,
        state: cast_playback_state(entry.player_state),
        playing_content_id: entry.media.as_ref().map(|m| m.content_id.clone()),
        idle_reason: entry.idle_reason.map(cast_idle_reason),
    }
}

fn cast_idle_reason(reason: IdleReason) -> CastIdleReason {
    match reason {
        IdleReason::Finished => CastIdleReason::Finished,
        IdleReason::Cancelled => CastIdleReason::Cancelled,
        IdleReason::Interrupted => CastIdleReason::Interrupted,
        IdleReason::Error => CastIdleReason::Error,
    }
}

/// The exact `rust_cast` 0.21 error text for the receiver's `LOADING`
/// player state, which its `PlayerState::from_str` rejects (design D7). The
/// pinned `rust_cast` version is part of this contract; a test owns it.
const LOADING_PARSE_ERROR_TEXT: &str = "Unknown player state LOADING";

/// Recognizes the one `rust_cast` parse error a receiver produces during
/// every load (`Error::Internal` naming `LOADING`): `media.get_status` and
/// `load` fail with it while the receiver fetches media (design D7). Every
/// other error -- including any other `Error::Internal` -- stays an error.
fn is_loading_parse_error(error: &CastError) -> bool {
    matches!(
        error.source().and_then(|source| source.downcast_ref::<RustCastError>()),
        Some(RustCastError::Internal(text)) if text == LOADING_PARSE_ERROR_TEXT
    )
}

/// What `connect` should do given the receiver's running applications and
/// the connect mode (design D11). Pure so the three outcomes are
/// unit-testable without a device.
enum ConnectPlan {
    /// Join the running default media receiver.
    Join {
        transport_id: String,
        session_id: String,
    },
    /// Launch the default media receiver.
    Launch,
    /// Another application is running and a reconnect must not launch over
    /// it: the output stays disconnected instead of taking the TV over.
    Refuse,
}

fn decide_connect(applications: &[Application], mode: ConnectMode) -> ConnectPlan {
    let expected = CastDeviceApp::DefaultMediaReceiver.to_string();
    match applications
        .iter()
        .find(|app| app.app_id == expected)
        .map(|app| ConnectPlan::Join {
            transport_id: app.transport_id.clone(),
            session_id: app.session_id.clone(),
        }) {
        Some(plan) => plan,
        None if applications.is_empty() => ConnectPlan::Launch,
        None => match mode {
            ConnectMode::FirstSelect => ConnectPlan::Launch,
            ConnectMode::Reconnect => ConnectPlan::Refuse,
        },
    }
}

/// The status of a receiver that reports no media session (cast-session-
/// control "Receiver is idle"): idle, no position, no duration, no content
/// id. An empty report is a status, not an error -- reporting it as `Err`
/// made the app mark the target disconnected and drop the transport on the
/// first poll after attach, so nothing could ever be dispatched to an idle
/// receiver.
fn status_from_entries(entries: &[StatusEntry]) -> CastStatus {
    match entries.first() {
        Some(entry) => cast_status_from_entry(entry),
        None => CastStatus::idle(),
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
    use rstest::rstest;

    #[test]
    fn build_media_sets_url_and_content_type() {
        let media = build_media("http://host/stream.mp4", "video/mp4");
        assert_eq!(media.content_id, "http://host/stream.mp4");
        assert_eq!(media.content_type, "video/mp4");
        assert_eq!(media.stream_type, StreamType::Buffered);
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
        assert!((status.playback_rate - 1.0).abs() < f32::EPSILON);
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

    /// The receiver's idle reason must cross the crate boundary: the Owner
    /// advance rule (design D7) reads it.
    #[test]
    fn cast_status_from_entry_maps_idle_finished_reason() {
        let mut entry = recorded_playing_entry();
        entry.player_state = CastPlayerState::Idle;
        entry.media = None;
        entry.idle_reason = Some(IdleReason::Finished);
        let status = cast_status_from_entry(&entry);
        assert_eq!(status.idle_reason, Some(CastIdleReason::Finished));
    }

    /// A load in progress is not a disconnect (design D7): the one
    /// `rust_cast` parse error naming `LOADING` is recognized and becomes a
    /// Buffering status for the loaded content id, while any other
    /// `Error::Internal` stays an error. The pinned `rust_cast` version is
    /// part of this contract.
    #[test]
    fn loading_parse_error_maps_to_buffering_other_internal_errors_stay_errors() {
        let loading = CastError::transport(
            TransportKind::Status,
            RustCastError::Internal("Unknown player state LOADING".to_string()),
        );
        assert!(is_loading_parse_error(&loading));
        let status = CastStatus::buffering(Some("http://host/stream".to_string()));
        assert_eq!(status.state, CastPlaybackState::Buffering);
        assert_eq!(
            status.playing_content_id.as_deref(),
            Some("http://host/stream")
        );

        let other = CastError::transport(
            TransportKind::Status,
            RustCastError::Internal("Unknown player state PAUSED".to_string()),
        );
        assert!(!is_loading_parse_error(&other));
    }

    /// A real receiver status payload's `applications` list is reduced to
    /// its app ids here; only the ids the decision reads are filled in.
    fn running_app(app_id: &str) -> Application {
        Application {
            app_id: app_id.to_string(),
            session_id: "session-1".to_string(),
            transport_id: "transport-1".to_string(),
            namespaces: Vec::new(),
            display_name: app_id.to_string(),
            status_text: String::new(),
        }
    }

    #[derive(Debug, PartialEq)]
    enum ExpectedConnect {
        Join,
        Launch,
        Refuse,
    }

    /// The connect decision (design D11), one case per distinct
    /// outcome-and-input pair: join the running default media receiver,
    /// launch when no app runs, refuse a reconnect when another app runs,
    /// and still launch on a first selection when another app runs.
    #[rstest]
    #[case::joins_running_default_media_receiver(
        ConnectMode::FirstSelect,
        &["CC1AD845"],
        ExpectedConnect::Join
    )]
    #[case::launches_when_no_app_runs(ConnectMode::FirstSelect, &[], ExpectedConnect::Launch)]
    #[case::refuses_reconnect_when_another_app_runs(
        ConnectMode::Reconnect,
        &["youtube"],
        ExpectedConnect::Refuse
    )]
    #[case::first_selection_launches_when_another_app_runs(
        ConnectMode::FirstSelect,
        &["youtube"],
        ExpectedConnect::Launch
    )]
    fn connect_decision_matches_design_d11(
        #[case] mode: ConnectMode,
        #[case] app_ids: &[&str],
        #[case] expected: ExpectedConnect,
    ) {
        let applications: Vec<Application> = app_ids.iter().copied().map(running_app).collect();
        let plan = decide_connect(&applications, mode);
        let actual = match &plan {
            ConnectPlan::Join { transport_id, .. } => {
                assert_eq!(transport_id, "transport-1");
                ExpectedConnect::Join
            }
            ConnectPlan::Launch => ExpectedConnect::Launch,
            ConnectPlan::Refuse => ExpectedConnect::Refuse,
        };
        assert_eq!(actual, expected);
    }

    /// Joining is mode-independent: a reconnect to a receiver that still
    /// runs the default media receiver joins it (design D11; the reconnect
    /// recovery of task 7.4 depends on it).
    #[test]
    fn reconnect_joins_running_default_media_receiver() {
        let applications = vec![running_app("CC1AD845")];
        assert!(matches!(
            decide_connect(&applications, ConnectMode::Reconnect),
            ConnectPlan::Join { .. }
        ));
    }

    /// An idle receiver answers `get_status` with no entries at all (no
    /// media session, recorded against the Shield Android TV spike setup).
    /// That empty report must come back as an idle status, not as an error:
    /// the app treats every status error as connection loss, so the error
    /// form killed the attachment on the first poll after attach and made
    /// the receiver undispatchable (cast-session-control "Receiver is
    /// idle").
    #[test]
    fn empty_status_report_is_an_idle_status_not_an_error() {
        let status = status_from_entries(&[]);
        assert_eq!(status.state, CastPlaybackState::Idle);
        assert_eq!(status.playing_content_id, None);
        assert_eq!(status.position_seconds, None);

        let status = status_from_entries(&[recorded_playing_entry()]);
        assert_eq!(status.state, CastPlaybackState::Playing);
    }
}
