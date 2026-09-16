pub(super) mod artwork_placeholder;

#[cfg(test)]
#[path = "artwork_placeholder_tests.rs"]
mod artwork_placeholder_tests;
pub(super) mod audiobookshelf_book;

pub(super) mod backdrop;
pub(in crate::app) mod card;
pub(in crate::app) mod chrome;
pub(super) mod chrome_player;
pub(in crate::app) mod chrome_player_context;
pub(in crate::app) mod chrome_status;
pub(in crate::app) mod chrome_tabs;
pub(super) mod confirm_modal;
pub(super) mod context_menu;
pub(super) mod daemon_lost_modal;
pub(super) mod detail;
pub(super) mod feeds_manage;
pub(super) mod help;
pub(super) mod hero;
pub(in crate::app) mod hero_model;
pub(super) mod home_feed;
pub(super) mod home_video;
pub mod indicators;
pub(in crate::app) mod inline_search;
pub(in crate::app) mod library_hero_overlay;
pub(super) mod library_routes;
pub(super) mod list;
pub(super) mod list_context;
pub(super) mod list_letter_groups;
pub(super) mod list_rows;
pub(super) mod marquee;
pub(super) mod media_list;
pub(super) mod modal_frame;
pub(super) mod multiselect;
pub(super) mod music_wide;
pub(super) mod playlists;
pub(in crate::app) mod queue;
pub(in crate::app) mod queue_playback;
pub(super) mod search_sidebar;
pub(super) mod sessions;
pub(super) mod settings;
pub(super) mod settings_component;
pub(super) mod tv_wide;
pub(super) mod visualizer;
pub(in crate::app) mod widgets;

#[cfg(test)]
#[path = "home_video_tests.rs"]
mod home_video_tests;
