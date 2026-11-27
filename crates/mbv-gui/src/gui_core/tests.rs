//! The GUI Client's own tests (`2-split-tui-gui-clients` design D4c): the
//! families that drive `GuiCore` over a core `App`, beside the code they test.
//! The prelude gives a child module what the core's test prelude gave before
//! the move, through the core's public surface and the GUI's own support
//! module.

use super::*;
use crate::gui_core::test_support::*;
use mbv_client_core::state::events::AudiobookshelfEvent;
use mbv_client_core::state::playback::DestinationLatestSource;
use mbv_client_core::*;
use mbv_ctrl::player::PlayerEvent;
use mbv_emby_model::EmbyItem;
use mbv_emby_model::test_support::make_item;
use mbv_ui_model::browse::{BrowseLevel, SeriesDetail};
use mbv_ui_model::confirm::ConfirmAction;
use mbv_ui_model::feed::{FeedHomeVideoSelection, FeedHomeVideoState};
use mbv_ui_model::library_tab::LibraryTab;
use mbv_ui_model::settings::{PanelFocus, SettingKey};
use mbv_ui_model::tab_selection::TabSelection;
use std::time::{Duration, Instant};

mod gui_library;
mod gui_library_audiobooks;
mod gui_library_emby;
mod gui_library_ensure;
mod gui_library_favourites;
mod gui_library_feeds;
mod gui_library_home;
mod gui_library_podcasts;
mod gui_library_refresh;
mod gui_library_rows;
mod gui_library_search;
mod gui_library_shows;
mod gui_library_tv;
mod gui_playlists;
mod gui_settings;
mod named_queue_controls;
mod row_menu;
