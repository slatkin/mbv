pub(in crate::app) mod arrangements;
pub(in crate::app) mod components;
mod screens;

// Render-seam re-exports for Interactive Components (design D9): the free
// functions extracted from `impl App` methods are `pub(in crate::app)` inside
// `render::components::help`, but the `components` module is private. Re-export
// them here so `crate::app::components::help` can import them without widening
// the whole `render::components` module.
pub(in crate::app) use components::artwork_placeholder::render_artwork_placeholder;
pub(in crate::app) use components::audiobookshelf_book::book_rows;

pub(in crate::app) use components::chrome_player::{
    render_player_panel, PlaybackControls, PlaybackRenderContext, PlaybackStripAreas,
};
pub(in crate::app) use components::chrome_status_bar::{
    render_status_bar, StatusBarModel, StatusBarRegions, VisualModeIndicator,
};
pub(in crate::app) use components::chrome_tabs::{render_tab_bar, TabBarModel};
pub(in crate::app) use components::confirm_modal::render_confirm_modal_content;
pub(in crate::app) use components::context_menu::render_context_menu_content;
pub(in crate::app) use components::daemon_lost_modal::render_daemon_lost_modal_content;
pub(in crate::app) use components::feeds_manage::{
    render_feeds_manage_content, FeedsManageRenderModel,
};
pub(in crate::app) use components::help::{
    help_destination, render_help_panel, HelpDestination, HelpRenderGeometry,
};

pub(in crate::app) use components::queue_playback::render_playback_header;

pub(in crate::app) use arrangements::wide_hero::{
    paint_wide_hero_text, place_media_list_below, wide_hero_fits, wide_hero_hero_pane,
    WrappedHeroLine, PANE_PAD_X, PANE_PAD_Y,
};
pub(in crate::app) use components::hero::render_search_box;
pub(in crate::app) use components::library_routes::{
    render_library_routes_content, save_route_config, LibraryRoutesRenderModel,
};
pub(in crate::app) use components::list_rows::LibraryListRenderCtx;
pub(in crate::app) use screens::album_plan::group_album_plan;
pub(in crate::app) use screens::feeds_model::{
    current_time_secs, feed_display_rows, FeedDisplayRow,
};
// `LetterFilter` is already `pub(crate)` re-exported below (ui_model::sort_filter).
pub(in crate::app) use components::media_list::render_wide_media_list_component;
pub(in crate::app) use components::multiselect::{
    render_multiselect_content, MultiSelectRenderModel,
};
pub(in crate::app) use components::music_wide::MusicWideRenderCtx;
pub(in crate::app) use components::playlists::{
    render_playlists_content, render_save_playlist_content, PlaylistsRenderGeometry,
    PlaylistsViewState,
};
pub(in crate::app) use components::search_sidebar::render_search_sidebar;
pub(in crate::app) use components::sessions::{
    render_sessions_overlay_content, render_sessions_scrollbar,
};
pub(in crate::app) use components::settings_component::{
    render_settings_content, SettingsRenderGeometry, SettingsRenderModel,
};
pub(in crate::app) use components::three_line_flat_list::render_three_line_flat_list;
pub(in crate::app) use components::tree_browser::{render_tree_browser, tree_row_is_full_width};
pub(in crate::app) use components::tv_wide::TvWideRenderCtx;
pub(in crate::app) use components::widgets::{
    render_pill_bar, render_placeholder, PillBar, PillBarWindow,
};
// Render-seam re-exports (design D9, task 3.1): the panel shell/scrollbar/row
// free functions extracted from `impl App` in `chrome.rs`. Used by the
// Interactive Components in `crate::app::components` (task 3.2+).
pub(in crate::app) use components::chrome::{render_panel_shell_at, render_sidebar_scrollbar};

// Re-exports so paths that resolved at `render::X` (or, from render's other
// submodules, `super::X`) before the render.rs split (issue #365 step 2,
// lane C) keep resolving without editing any call site. The `pub(crate)`
// trio is referenced from outside `render` entirely (src/app.rs,
// src/app/actions.rs); the rest are referenced via `super::X` from render's
// sibling submodules (album, card, detail, home, list, music, pills, queue).
pub(super) use crate::app::ui_model::sort_filter::{
    effective_sort_str, letter_bucket, parse_album_folder_name, strip_article,
};
pub(crate) use crate::app::ui_model::sort_filter::{
    initial_group_artist_sort_key, LetterFilter, LetterFilterKind, LIBRARY_PILL_THRESHOLD,
};
pub use components::indicators;
use components::widgets::render_right_scrollbar;
pub(super) use screens::album_plan::sorted_group_album_order;
// `theme`'s roles are re-exported here (rather than reached directly) so
// `palette.rs` — a sibling of `render`, not a descendant — can bridge to them;
// see `palette.rs`'s own re-export.
// The closed surface table (`unify-surface-colour-neutral` D1/D2/D7) is bridged
// to `palette.rs` the same way. Its visibility is `crate::app`, so it is not
// part of the wider `pub(crate)` role list above.

use crate::app::ui_model::ui_util::natural_sort_key;
use mbv_theme as palette;
