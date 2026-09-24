use crate::app::state::types::events::{LibEvent, SessionEvent};
use crate::app::state::types::feed::IdleFeed;
use crate::app::state::types::playback::QueueScope;
use crate::app::state::types::player_tab::PlayerTab;
use mbv_core::api::EmbyItem;
use mbv_core::player::{PlayerEvent, PlayerProxy};
use mbv_core::service_runtime::{AudiobookshelfRuntime, EmbyRuntime};
use mbv_core::ws::WsEvent;
use std::sync::{mpsc, Arc, Mutex};

pub(in crate::app) struct AppInit {
    pub(in crate::app) config: Arc<Mutex<crate::config::Config>>,
    pub(in crate::app) emby_runtime: EmbyRuntime,
    pub(in crate::app) audiobookshelf_runtime: AudiobookshelfRuntime,
    pub(in crate::app) emby_startup_rx: Option<crate::app::service_startup::StartupReceiver>,
    pub(in crate::app) emby_startup_request: Option<(
        crate::config::Config,
        mbv_core::service_runtime::SetupGeneration,
    )>,
    pub(in crate::app) audiobookshelf_startup_rx:
        Option<crate::app::service_startup::AudiobookshelfStartupReceiver>,
    pub(in crate::app) audiobookshelf_startup_request: Option<(
        crate::config::Config,
        mbv_core::service_runtime::SetupGeneration,
    )>,
    pub(in crate::app) audiobookshelf_test_rx:
        Option<crate::app::service_startup::AudiobookshelfStartupReceiver>,
    pub(in crate::app) audiobookshelf_setup_rx:
        Option<mpsc::Receiver<crate::app::service_startup::AudiobookshelfSetupCompletion>>,
    pub(in crate::app) emby_setup_form: Option<crate::app::services_settings::EmbySetupForm>,
    pub(in crate::app) emby_setup_rx:
        Option<mpsc::Receiver<crate::app::service_startup::SetupCompletion>>,
    pub(in crate::app) player: PlayerProxy,
    pub(in crate::app) player_rx: mpsc::Receiver<PlayerEvent>,
    pub(in crate::app) ws_rx: mpsc::Receiver<WsEvent>,
    pub(in crate::app) ws_send_tx: Option<mbv_core::ws::WsSender>,
    pub(in crate::app) audiobookshelf_socket_rx:
        mpsc::Receiver<mbv_core::audiobookshelf_socket::SocketEvent>,
    pub(in crate::app) audiobookshelf_socket_tx: Option<mpsc::Sender<()>>,
    pub(in crate::app) audiobookshelf_socket_generation:
        Option<mbv_core::service_runtime::SetupGeneration>,
    pub(in crate::app) player_tab: PlayerTab,
    pub(in crate::app) remote_player_tab: Option<PlayerTab>,
    pub(in crate::app) initial_queue_scope: QueueScope,
    pub(in crate::app) system_notifications: bool,
    pub(in crate::app) image_protocol: Option<String>,
    pub(in crate::app) image_protocol_enabled: bool,
    pub(in crate::app) hidden_libraries: Vec<String>,
    pub(in crate::app) library_routes: std::collections::HashMap<String, String>,
    pub(in crate::app) music_levels: Vec<String>,
    pub(in crate::app) use_nerd_fonts: bool,
    pub(in crate::app) indicator_style: crate::app::render::indicators::IndicatorStyle,
    pub(in crate::app) image_cache_size: usize,
    pub(in crate::app) visualizer_glyph: String,
    pub(in crate::app) lib_tx: mpsc::Sender<LibEvent>,
    pub(in crate::app) lib_rx: mpsc::Receiver<LibEvent>,
    pub(in crate::app) sessions_tx: mpsc::Sender<SessionEvent>,
    pub(in crate::app) sessions_rx: mpsc::Receiver<SessionEvent>,
    pub(in crate::app) card_image_tx: mpsc::Sender<(String, Option<image::DynamicImage>)>,
    pub(in crate::app) card_image_rx: mpsc::Receiver<(String, Option<image::DynamicImage>)>,
    pub(in crate::app) notif_action_tx: mpsc::Sender<String>,
    pub(in crate::app) notif_action_rx: mpsc::Receiver<String>,
    pub(in crate::app) search_tx: mpsc::Sender<(String, Result<Vec<EmbyItem>, String>)>,
    pub(in crate::app) search_rx: mpsc::Receiver<(String, Result<Vec<EmbyItem>, String>)>,
    pub(in crate::app) idle_feed: Option<IdleFeed>,
}
