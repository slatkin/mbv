use super::types_events::{LibEvent, SessionEvent};
use super::types_feed::IdleFeed;
use super::types_playback::QueueScope;
use super::types_player_tab::PlayerTab;
use mbv_core::api::EmbyItem;
use mbv_core::player::{PlayerEvent, PlayerProxy};
use mbv_core::service_runtime::{AudiobookshelfRuntime, EmbyRuntime};
use mbv_core::ws::WsEvent;
use std::sync::{mpsc, Arc, Mutex};

pub(super) struct AppInit {
    pub(super) config: Arc<Mutex<crate::config::Config>>,
    pub(super) emby_runtime: EmbyRuntime,
    pub(super) audiobookshelf_runtime: AudiobookshelfRuntime,
    pub(super) emby_startup_rx: Option<super::service_startup::StartupReceiver>,
    pub(super) emby_startup_request: Option<(
        crate::config::Config,
        mbv_core::service_runtime::SetupGeneration,
    )>,
    pub(super) audiobookshelf_startup_rx:
        Option<super::service_startup::AudiobookshelfStartupReceiver>,
    pub(super) audiobookshelf_startup_request: Option<(
        crate::config::Config,
        mbv_core::service_runtime::SetupGeneration,
    )>,
    pub(super) audiobookshelf_test_rx:
        Option<super::service_startup::AudiobookshelfStartupReceiver>,
    pub(super) audiobookshelf_setup_rx:
        Option<mpsc::Receiver<super::service_startup::AudiobookshelfSetupCompletion>>,
    pub(super) emby_setup_form: Option<super::services_settings::EmbySetupForm>,
    pub(super) emby_setup_rx: Option<mpsc::Receiver<super::service_startup::SetupCompletion>>,
    pub(super) player: PlayerProxy,
    pub(super) player_rx: mpsc::Receiver<PlayerEvent>,
    pub(super) ws_rx: mpsc::Receiver<WsEvent>,
    pub(super) ws_send_tx: Option<mbv_core::ws::WsSender>,
    pub(super) audiobookshelf_socket_rx:
        mpsc::Receiver<mbv_core::audiobookshelf_socket::SocketEvent>,
    pub(super) audiobookshelf_socket_tx: Option<mbv_core::audiobookshelf_socket::SocketSender>,
    pub(super) audiobookshelf_socket_generation: Option<mbv_core::service_runtime::SetupGeneration>,
    pub(super) player_tab: PlayerTab,
    pub(super) remote_player_tab: Option<PlayerTab>,
    pub(super) initial_queue_scope: QueueScope,
    pub(super) system_notifications: bool,
    pub(super) image_protocol: Option<String>,
    pub(super) image_protocol_enabled: bool,
    pub(super) hidden_libraries: Vec<String>,
    pub(super) library_routes: std::collections::HashMap<String, String>,
    pub(super) hidden_latest: Vec<String>,
    pub(super) music_levels: Vec<String>,
    pub(super) use_nerd_fonts: bool,
    pub(super) indicator_style: super::render::indicators::IndicatorStyle,
    pub(super) image_cache_size: usize,
    pub(super) lib_tx: mpsc::Sender<LibEvent>,
    pub(super) lib_rx: mpsc::Receiver<LibEvent>,
    pub(super) sessions_tx: mpsc::Sender<SessionEvent>,
    pub(super) sessions_rx: mpsc::Receiver<SessionEvent>,
    pub(super) card_image_tx: mpsc::Sender<(String, Option<image::DynamicImage>)>,
    pub(super) card_image_rx: mpsc::Receiver<(String, Option<image::DynamicImage>)>,
    pub(super) notif_action_tx: mpsc::Sender<String>,
    pub(super) notif_action_rx: mpsc::Receiver<String>,
    pub(super) search_tx: mpsc::Sender<(String, Result<Vec<EmbyItem>, String>)>,
    pub(super) search_rx: mpsc::Receiver<(String, Result<Vec<EmbyItem>, String>)>,
    pub(super) idle_feed: Option<IdleFeed>,
}
