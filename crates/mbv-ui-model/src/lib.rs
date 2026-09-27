#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiConfig {
    pub image_protocol: Option<String>,
    pub image_cache_size: usize,
    pub use_nerd_fonts: bool,
    pub indicator_style: String,
    pub visualizer_glyph: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            image_protocol: None,
            image_cache_size: 50,
            use_nerd_fonts: false,
            indicator_style: "keyvalue".into(),
            visualizer_glyph: "●".into(),
        }
    }
}

pub mod audiobookshelf_browse;
pub mod browse;
pub mod confirm;
pub mod context_menu;
pub mod daemon_lost;
pub mod feed;
pub mod feed_age;
pub mod feed_tab;
pub mod feeds_manage;
pub mod home_latest;
pub mod library;
pub mod library_tab;
pub mod media_list;
pub mod msg;
pub mod music_artist_detail;
pub mod music_grouping;
pub mod overlay;
pub mod panel_targets;
pub mod playback;
pub mod playback_target;
pub mod queue_card;
pub mod search_sidebar;
pub mod settings;
pub mod sidebar;
pub mod sort_filter;
pub mod tab_selection;
pub mod ui_util;
pub mod volume;
