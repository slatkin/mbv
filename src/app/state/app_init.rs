use crate::app::state::types::feed::IdleFeed;
use crate::app::state::types::playback::QueueScope;
use crate::app::state::types::player_tab::PlayerTab;
use mbv_core::player::{PlayerEvent, PlayerProxy};
use mbv_core::service_runtime::{AudiobookshelfRuntime, EmbyRuntime};
use mbv_ws::WsEvent;
use std::sync::{mpsc, Arc, Mutex};

pub(in crate::app) struct AppInit {
    pub(in crate::app) config: Arc<Mutex<crate::config::Config>>,
    pub(in crate::app) emby_runtime: EmbyRuntime,
    pub(in crate::app) audiobookshelf_runtime: AudiobookshelfRuntime,
    pub(in crate::app) player: PlayerProxy,
    pub(in crate::app) player_rx: mpsc::Receiver<PlayerEvent>,
    pub(in crate::app) ws_rx: mpsc::Receiver<WsEvent>,
    pub(in crate::app) ws_send_tx: Option<mbv_ws::WsSender>,
    pub(in crate::app) audiobookshelf_socket_rx:
        mpsc::Receiver<mbv_core::audiobookshelf::socket::SocketEvent>,
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
    pub(in crate::app) library_routes: std::collections::BTreeMap<String, String>,
    pub(in crate::app) music_levels: Vec<String>,
    pub(in crate::app) use_nerd_fonts: bool,
    pub(in crate::app) indicator_style: crate::app::render::indicators::IndicatorStyle,
    pub(in crate::app) image_cache_size: usize,
    pub(in crate::app) visualizer_glyph: String,
    pub(in crate::app) card_image_tx: mpsc::Sender<(String, Option<image::DynamicImage>)>,
    pub(in crate::app) card_image_rx: mpsc::Receiver<(String, Option<image::DynamicImage>)>,
    pub(in crate::app) channels: super::runtime_channels::RuntimeChannels,
    pub(in crate::app) idle_feed: Option<IdleFeed>,
}
