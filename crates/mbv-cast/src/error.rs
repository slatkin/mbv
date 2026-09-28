use std::{error::Error, fmt};

#[derive(Debug)]
pub struct CastError {
    kind: CastErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

#[derive(Debug, Clone, Copy)]
enum CastErrorKind {
    Transport(TransportKind),
    Discovery,
    Dispatch(DispatchKind),
    Worker(WorkerKind),
}

#[derive(Debug, Clone, Copy)]
enum TransportKind {
    Connect,
    ReceiverPlatformConnect,
    LaunchApp,
    AppTransportConnect,
    Teardown,
    KeepAlive,
    Load,
    LoadQueue,
    Play,
    Pause,
    Stop,
    Seek,
    Jump,
    JumpNoQueue,
    JumpNoAdjacent,
    SetVolume,
    SetMuted,
    Status,
    StatusNoEntries,
    NoSessionId,
    Command,
}

#[derive(Debug, Clone, Copy)]
enum DispatchKind {
    FeedNoUrl,
    AudiobookshelfHls,
    AudiobookshelfBook,
    EmbyUnavailable,
    EmbyPlaybackInfo,
    AudiobookshelfUnavailable,
    AudiobookshelfSession,
}

#[derive(Debug, Clone, Copy)]
enum WorkerKind {
    NotReady,
    ReceiverNotFound,
    NotConnected,
    Gone,
}

impl CastError {
    fn new(kind: CastErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    fn with_source<E>(kind: CastErrorKind, context: &str, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind,
            message: format!("{context}: {source}"),
            source: Some(Box::new(source)),
        }
    }

    pub(crate) fn transport<E>(operation: &'static str, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        let (operation_kind, context) = match operation {
            "connect" => (TransportKind::Connect, "cast connect failed"),
            "receiver-platform connect" => (
                TransportKind::ReceiverPlatformConnect,
                "cast receiver-platform connect failed",
            ),
            "launch_app" => (TransportKind::LaunchApp, "cast launch_app failed"),
            "app-transport connect" => (
                TransportKind::AppTransportConnect,
                "cast app-transport connect failed",
            ),
            "teardown" => (TransportKind::Teardown, "cast teardown failed"),
            "keep_alive" => (TransportKind::KeepAlive, "cast keep_alive failed"),
            "load" => (TransportKind::Load, "cast load failed"),
            "load_queue" => (TransportKind::LoadQueue, "cast load_queue failed"),
            "play" => (TransportKind::Play, "cast play failed"),
            "pause" => (TransportKind::Pause, "cast pause failed"),
            "stop" => (TransportKind::Stop, "cast stop failed"),
            "seek" => (TransportKind::Seek, "cast seek failed"),
            "jump" => (TransportKind::Jump, "cast jump failed"),
            "set_volume" => (TransportKind::SetVolume, "cast set_volume failed"),
            "set_muted" => (TransportKind::SetMuted, "cast set_muted failed"),
            "get_status" => (TransportKind::Status, "cast get_status failed"),
            _ => (TransportKind::Command, "cast command failed"),
        };
        Self::with_source(CastErrorKind::Transport(operation_kind), context, source)
    }

    pub(crate) fn jump_no_queue() -> Self {
        Self::new(
            CastErrorKind::Transport(TransportKind::JumpNoQueue),
            "cast jump failed: no dispatched queue to advance",
        )
    }

    pub(crate) fn jump_no_adjacent() -> Self {
        Self::new(
            CastErrorKind::Transport(TransportKind::JumpNoAdjacent),
            "cast jump failed: no adjacent queue item",
        )
    }

    #[must_use]
    pub fn status_no_entries() -> Self {
        Self::new(
            CastErrorKind::Transport(TransportKind::StatusNoEntries),
            "cast get_status returned no entries",
        )
    }

    pub(crate) fn no_session_id() -> Self {
        Self::new(
            CastErrorKind::Transport(TransportKind::NoSessionId),
            "cast command failed: nothing loaded yet",
        )
    }

    #[must_use]
    pub fn feed_no_url(title: &str) -> Self {
        Self::new(
            CastErrorKind::Dispatch(DispatchKind::FeedNoUrl),
            format!("\"{title}\" has no media URL to cast"),
        )
    }

    pub(crate) fn audiobookshelf_hls() -> Self {
        Self::new(
            CastErrorKind::Dispatch(DispatchKind::AudiobookshelfHls),
            "Audiobookshelf HLS renditions are not castable",
        )
    }

    #[must_use]
    pub fn audiobookshelf_book(title: &str) -> Self {
        Self::new(
            CastErrorKind::Dispatch(DispatchKind::AudiobookshelfBook),
            format!("\"{title}\" is a multi-file audiobook and can't be cast"),
        )
    }

    #[must_use]
    pub fn worker_not_ready() -> Self {
        Self::new(
            CastErrorKind::Worker(WorkerKind::NotReady),
            "cast worker thread exited before reporting readiness",
        )
    }

    #[must_use]
    pub fn receiver_not_found() -> Self {
        Self::new(
            CastErrorKind::Worker(WorkerKind::ReceiverNotFound),
            "receiver not found",
        )
    }

    #[must_use]
    pub fn not_connected() -> Self {
        Self::new(
            CastErrorKind::Worker(WorkerKind::NotConnected),
            "not connected to the receiver yet",
        )
    }

    #[must_use]
    pub fn worker_gone() -> Self {
        Self::new(
            CastErrorKind::Worker(WorkerKind::Gone),
            "cast worker is gone",
        )
    }

    #[must_use]
    pub fn emby_unavailable(title: &str) -> Self {
        Self::new(
            CastErrorKind::Dispatch(DispatchKind::EmbyUnavailable),
            format!("\"{title}\" needs Emby, which isn't connected"),
        )
    }

    #[must_use]
    pub fn emby_playback_info<E>(source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind: CastErrorKind::Dispatch(DispatchKind::EmbyPlaybackInfo),
            message: source.to_string(),
            source: Some(Box::new(source)),
        }
    }

    #[must_use]
    pub fn audiobookshelf_unavailable(title: &str) -> Self {
        Self::new(
            CastErrorKind::Dispatch(DispatchKind::AudiobookshelfUnavailable),
            format!("\"{title}\" needs Audiobookshelf, which isn't connected"),
        )
    }

    #[must_use]
    pub fn audiobookshelf_session<E>(title: &str, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind: CastErrorKind::Dispatch(DispatchKind::AudiobookshelfSession),
            message: format!("\"{title}\" Audiobookshelf session failed: {source}"),
            source: Some(Box::new(source)),
        }
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        self.kind.kind_name()
    }
}

impl CastErrorKind {
    fn kind_name(self) -> &'static str {
        match self {
            Self::Transport(kind) => kind.kind_name(),
            Self::Discovery => "cast.discovery",
            Self::Dispatch(kind) => kind.kind_name(),
            Self::Worker(kind) => kind.kind_name(),
        }
    }
}

impl TransportKind {
    fn kind_name(self) -> &'static str {
        match self {
            Self::Connect => "cast.connect",
            Self::ReceiverPlatformConnect => "cast.receiver_platform_connect",
            Self::LaunchApp => "cast.launch_app",
            Self::AppTransportConnect => "cast.app_transport_connect",
            Self::Teardown => "cast.teardown",
            Self::KeepAlive => "cast.keep_alive",
            Self::Load => "cast.load",
            Self::LoadQueue => "cast.load_queue",
            Self::Play => "cast.play",
            Self::Pause => "cast.pause",
            Self::Stop => "cast.stop",
            Self::Seek => "cast.seek",
            Self::Jump => "cast.jump",
            Self::JumpNoQueue => "cast.jump_no_queue",
            Self::JumpNoAdjacent => "cast.jump_no_adjacent",
            Self::SetVolume => "cast.set_volume",
            Self::SetMuted => "cast.set_muted",
            Self::Status => "cast.status",
            Self::StatusNoEntries => "cast.status_no_entries",
            Self::NoSessionId => "cast.no_session_id",
            Self::Command => "cast.command",
        }
    }
}

impl DispatchKind {
    fn kind_name(self) -> &'static str {
        match self {
            Self::FeedNoUrl => "cast.feed_no_url",
            Self::AudiobookshelfHls => "cast.audiobookshelf_hls",
            Self::AudiobookshelfBook => "cast.audiobookshelf_book",
            Self::EmbyUnavailable => "cast.emby_unavailable",
            Self::EmbyPlaybackInfo => "cast.emby_playback_info",
            Self::AudiobookshelfUnavailable => "cast.audiobookshelf_unavailable",
            Self::AudiobookshelfSession => "cast.audiobookshelf_session",
        }
    }
}

impl WorkerKind {
    fn kind_name(self) -> &'static str {
        match self {
            Self::NotReady => "cast.worker_not_ready",
            Self::ReceiverNotFound => "cast.receiver_not_found",
            Self::NotConnected => "cast.not_connected",
            Self::Gone => "cast.worker_gone",
        }
    }
}

impl fmt::Display for CastError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<mdns_sd::Error> for CastError {
    fn from(source: mdns_sd::Error) -> Self {
        Self {
            kind: CastErrorKind::Discovery,
            message: source.to_string(),
            source: Some(Box::new(source)),
        }
    }
}

impl Error for CastError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
