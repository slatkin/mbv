//! Plain presentation models shared by the UI crates.
//!
//! One module per surface (browse, playback, settings, search) holds display-ready
//! state with no painting and no input handling. `mbv-render` paints these models;
//! `mbv-components` keeps cursor, scroll, and selection locally, never here.

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
mod error;
#[doc(inline)]
pub use error::UiModelError;
pub mod feed;
pub mod feed_age;
pub mod feed_tab;
pub mod feeds_manage;
pub mod home_latest;
pub mod library;
pub mod library_tab;
pub mod media_list;
pub mod music_artist_detail;
pub mod music_grouping;
pub mod overlay;
pub mod panel_targets;
pub mod playback;
pub mod playback_target;
pub mod search_sidebar;
pub mod settings;
pub mod sort_filter;
pub mod tab_selection;
pub mod targets;
pub mod ui_util;
