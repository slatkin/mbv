pub(super) mod artwork_placeholder;

pub(super) mod audiobookshelf_book;

pub(super) mod backdrop;
pub(in crate::app) mod card;
pub(in crate::app) mod chrome;
pub(crate) mod chrome_player;
pub(in crate::app) mod chrome_status_bar;
pub(in crate::app) mod chrome_tabs;
pub(super) mod confirm_modal;
pub(super) mod context_menu;
pub(super) mod daemon_lost_modal;
pub(super) mod feeds_manage;
pub(super) mod help;
pub(super) mod hero;
pub(in crate::app) mod hero_model;
// Feed/home-video group browsing is painted by the embedded EmbyLibraryContent
// owner; the shell owns loading and projects FeedHomeVideoState into its context.
pub(super) mod home_video;
pub mod indicators;
pub(in crate::app) mod library_hero_overlay;
pub(super) mod library_routes;
pub(super) mod list_rows;
pub(in crate::app) mod marquee;
pub(crate) mod media_list;
pub(super) mod modal_frame;
pub(super) mod multiselect;
pub(in crate::app) mod music_wide;
pub(super) mod playlists;
pub(in crate::app) mod queue;
pub(in crate::app) mod queue_playback;
pub(super) mod search_sidebar;
pub(super) mod sessions;
pub(crate) mod settings_component;
pub(crate) mod three_line_flat_list;
pub(crate) mod tree_browser;
pub(crate) mod tv_wide;
pub(in crate::app) mod widgets;
