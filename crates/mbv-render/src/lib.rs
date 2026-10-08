//! Stateless painting for the terminal UI: content preparation, placement, and paint.
//!
//! `screens/` prepare content, `arrangements/` place it, `components/` paint it, and
//! `layout.rs` composes the layouts. Painters take typed models plus a `Rect` and never
//! name `App`; cursor, scroll, and selection live in `mbv-components`, and the painted
//! models live in `mbv-ui-model`.

pub mod arrangements;
pub mod components;
pub mod layout;
pub mod screens;

// Render-seam re-exports for Interactive Components (design D9): the free
// functions extracted from `impl App` methods are `pub(crate)` inside
// `render::components::help`, but the `components` module is private. Re-export
// them here so `mbv_ui_model::components::help` can import them without widening
// the whole `render::components` module.
#[doc(inline)]
pub use components::artwork_placeholder::render_artwork_placeholder;
#[doc(inline)]
pub use components::audiobookshelf_book::book_rows;

#[doc(inline)]
pub use components::chrome_player::{
    HeaderTitle, PlaybackControls, PlaybackRenderContext, PlaybackStripAreas, playback_state_icon,
    render_header_title, render_player_panel,
};
#[doc(inline)]
pub use components::chrome_status_bar::{
    StatusBarModel, StatusBarRegions, VisualModeIndicator, render_status_bar,
};
#[doc(inline)]
pub use components::chrome_tabs::{TabBarModel, render_tab_bar};
#[doc(inline)]
pub use components::confirm_modal::{ConfirmRenderGeometry, render_confirm_modal_content};
#[doc(inline)]
pub use components::context_menu::render_context_menu_content;
#[doc(inline)]
pub use components::daemon_lost_modal::render_daemon_lost_modal_content;
#[doc(inline)]
pub use components::feeds_manage::{FeedsManageRenderModel, render_feeds_manage_content};
#[doc(inline)]
pub use components::help::{
    HelpDestination, HelpRenderGeometry, help_destination, render_help_panel,
};

#[doc(inline)]
pub use components::queue_playback::render_playback_header;

#[doc(inline)]
pub use arrangements::wide_hero::{
    PANE_PAD_X, PANE_PAD_Y, WrappedHeroLine, paint_wide_hero_text, place_media_list_below,
    wide_hero_fits, wide_hero_hero_pane,
};
#[doc(inline)]
pub use components::hero::render_search_box;
#[doc(inline)]
pub use components::library_routes::{LibraryRoutesRenderModel, render_library_routes_content};
#[doc(inline)]
pub use components::list_rows::LibraryListRenderCtx;
#[doc(inline)]
pub use screens::album_plan::group_album_plan;
#[doc(inline)]
pub use screens::feeds_model::{FeedDisplayRow, current_time_secs, feed_display_rows};
// `LetterFilter` is already `pub(crate)` re-exported below (ui_model::sort_filter).
#[doc(inline)]
pub use components::media_list::render_wide_media_list_component;
#[doc(inline)]
pub use components::multiselect::{MultiSelectRenderModel, render_multiselect_content};
#[doc(inline)]
pub use components::music_wide::MusicWideRenderCtx;
#[doc(inline)]
pub use components::playlists::{
    PlaylistsRenderGeometry, PlaylistsViewState, render_playlists_content,
    render_save_playlist_content,
};
#[doc(inline)]
pub use components::search_sidebar::render_search_sidebar;
#[doc(inline)]
pub use components::sessions::{render_sessions_overlay_content, render_sessions_scrollbar};
#[doc(inline)]
pub use components::settings_component::{
    SettingsRenderGeometry, SettingsRenderModel, render_settings_content,
};
#[doc(inline)]
pub use components::three_line_flat_list::render_three_line_flat_list;
#[doc(inline)]
pub use components::tree_browser::{render_tree_browser, tree_row_is_full_width};
#[doc(inline)]
pub use components::tv_wide::TvWideRenderCtx;
#[doc(inline)]
pub use components::widgets::{PillBar, PillBarWindow, render_pill_bar, render_placeholder};
// Render-seam re-exports (design D9, task 3.1): the panel shell/scrollbar/row
// free functions extracted from `impl App` in `chrome.rs`. Used by the
// Interactive Components in `mbv_ui_model::components` (task 3.2+).
#[doc(inline)]
pub use components::chrome::{render_panel_shell_at, render_sidebar_scrollbar};

// Re-exports so paths that resolved at `render::X` (or, from render's other
// submodules, `super::X`) before the render.rs split (issue #365 step 2,
// lane C) keep resolving without editing any call site. The `pub(crate)`
// trio is referenced from outside `render` entirely (src/app.rs,
// src/app/actions.rs); the rest are referenced via `super::X` from render's
// sibling submodules (album, card, detail, home, list, music, pills, queue).
#[doc(inline)]
pub use components::indicators;
use components::widgets::render_right_scrollbar;
#[doc(inline)]
pub use mbv_ui_model::sort_filter::{
    LIBRARY_PILL_THRESHOLD, LetterFilter, LetterFilterKind, initial_group_artist_sort_key,
};
#[doc(inline)]
pub use mbv_ui_model::sort_filter::{
    effective_sort_str, letter_bucket, parse_album_folder_name, strip_article,
};
#[doc(inline)]
pub use screens::album_plan::sorted_group_album_order;
// `theme`'s roles are re-exported here (rather than reached directly) so
// `palette.rs` — a sibling of `render`, not a descendant — can bridge to them;
// see `palette.rs`'s own re-export.
// The closed surface table (`unify-surface-colour-neutral` D1/D2/D7) is bridged
// to `palette.rs` the same way. Its visibility is `crate::app`, so it is not
// part of the wider `pub(crate)` role list above.

use mbv_theme as palette;
use mbv_ui_model::ui_util::natural_sort_key;
