pub(in crate::app) mod arrangements;
pub(in crate::app) mod components;
mod screens;
mod theme;

// Render-seam re-exports for Interactive Components (design D9): the free
// functions extracted from `impl App` methods are `pub(in crate::app)` inside
// `render::components::help`, but the `components` module is private. Re-export
// them here so `crate::app::components::help` can import them without widening
// the whole `render::components` module.
pub(in crate::app) use components::artwork_placeholder::render_artwork_placeholder;
pub(in crate::app) use components::audiobookshelf_book::book_rows;

#[allow(unused_imports)]
pub(in crate::app) use components::chrome_player::{
    render_player_panel, render_title_row, PlaybackRenderContext, PlaybackStripAreas,
};
pub(in crate::app) use components::chrome_status::{
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
pub(in crate::app) use components::queue::{render_queue_body, QueuePresentation};
pub(in crate::app) use components::queue_playback::render_playback_header;

pub(in crate::app) use arrangements::wide_hero::{
    paint_wide_hero_text, place_media_list_below, wide_hero_browser_pane, wide_hero_fits,
    wide_hero_hero_pane, WrappedHeroLine, PANE_PAD_X, PANE_PAD_Y,
};
pub(in crate::app) use components::hero::render_search_box;
pub(in crate::app) use components::library_routes::{
    render_library_routes_content, save_route_config, LibraryRoutesRenderModel,
};
pub(in crate::app) use components::list_rows::LibraryListRenderCtx;
pub(in crate::app) use screens::feeds_model::{
    current_time_secs, feed_age_group, feed_display_rows, FeedAgeGroup, FeedDisplayRow,
};
// `LetterFilter` is already `pub(crate)` re-exported below (screens::sort_filter).
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
pub(in crate::app) use components::sessions::render_sessions_overlay_content;
pub(in crate::app) use components::settings_component::{
    render_settings_content, SettingsRenderGeometry, SettingsRenderModel,
};
pub(in crate::app) use components::tv_wide::TvWideRenderCtx;
pub(in crate::app) use components::widgets::{
    render_pill_bar, render_placeholder, PillBar, PillBarWindow,
};
// Render-seam re-exports (design D9, task 3.1): the panel shell/scrollbar/row
// free functions extracted from `impl App` in `chrome.rs`. Used by the
// Interactive Components in `crate::app::components` (task 3.2+).
pub(in crate::app) use components::chrome::{render_panel_shell_at, render_sidebar_scrollbar};

// Re-exports so paths that resolved at `render::X` (or, from render's other
// submodules, `super::X`) before the render/mod.rs split (issue #365 step 2,
// lane C) keep resolving without editing any call site. The `pub(crate)`
// trio is referenced from outside `render` entirely (src/app/mod.rs,
// src/app/actions.rs); the rest are referenced via `super::X` from render's
// sibling submodules (album, card, detail, home, list, music, pills, queue)
// and/or `use super::*` in render/tests.rs.
pub use components::indicators;
use components::widgets::render_right_scrollbar;
pub(super) use screens::album_plan::sorted_group_album_order;
pub(super) use screens::sort_filter::{
    effective_sort_str, letter_bucket, parse_album_folder_name, strip_article,
};
pub(crate) use screens::sort_filter::{
    initial_group_artist_sort_key, LetterFilter, LetterFilterKind, LIBRARY_PILL_THRESHOLD,
};
// `theme`'s roles are re-exported here (rather than reached directly) so
// `palette.rs` — a sibling of `render`, not a descendant — can bridge to them;
// see `palette.rs`'s own re-export.
pub(crate) use theme::{
    bar_role_fg, ACCENT, ACCENT_ACTIVE, ACCENT_AUDIOBOOKSHELF, DURATION, HERO_CREDITS_NAME,
    HERO_CREDITS_STRIPE, HERO_META_ROLES, HERO_OVERVIEW_SEPARATOR, HINT_PILL_FILLS,
    INDICATOR_AUDIO_FG, INDICATOR_RESOLUTION_FG, MUSIC_HEADER, PILL_OVERFLOW_FG, PILL_SELECTED_FG,
    PLAYBACK_CONTEXT_FG, PLAYBACK_META_FG, PLAYBACK_TITLE_FG, PLAYBACK_VALUE_FG, PROGRESS_PERCENT,
    PROGRESS_TRACK, SCROLLBAR, SELECTED_ROW_BG, SELECTED_ROW_FG, SELECTED_ROW_PROGRESS_FG,
    SPLIT_ROW_CONTEXT_FG, SPLIT_ROW_TITLE_FG, STATUS_AVAILABLE, STATUS_ERROR, TEXT_ACCENT_MUTED,
    TEXT_EMPHASIS, TEXT_FOCUS_ACCENT, TEXT_HERO_TITLE, TEXT_METADATA, TEXT_MUTED, TEXT_ON_ACCENT,
    TEXT_PRIMARY, TEXT_SECONDARY, TEXT_STRONG, WORKSPACE_HEADER_FG,
};
// Task 4.2: the retired role names and the value-aliased resolver survive only
// as test-fed re-exports — each is pinned by a frozen pre-existing test file
// the neutrality rule forbids editing, so a minimal named re-export stays for
// exactly those names (see the change report for the per-name reasons).
#[cfg(test)]
pub(crate) use theme::{
    resolve_surface_focus, PILL_ROW_BG, PILL_SELECTED_BG, SURFACE_ARTWORK_PLACEHOLDER,
    SURFACE_BACKDROP, SURFACE_CHROME, SURFACE_FOCUSED, SURFACE_PLAYBACK, SURFACE_RESTING,
};
// The closed surface table (`unify-surface-colour-neutral` D1/D2/D7) is bridged
// to `palette.rs` the same way. Its visibility is `crate::app`, so it is not
// part of the wider `pub(crate)` role list above.
pub(in crate::app) use theme::{surface_colors, Surface};

use super::ui_util::natural_sort_key;
use super::{palette, App};

// Test-only: these names are otherwise unused in the production build (their
// only production callers moved into root.rs/queue.rs under screens/, which
// import them directly), but render/tests.rs and friends still reach them via
// `use super::*`.
#[cfg(test)]
use components::widgets::right_panel_content_area;
#[cfg(test)]
use mbv_core::api::TICKS_PER_SECOND;
#[cfg(test)]
use ratatui::layout::Rect;
#[cfg(test)]
use unicode_width::UnicodeWidthStr;

#[cfg(test)]
#[path = "tests_confirm_modal.rs"]
mod confirm_modal_tests;
#[cfg(test)]
#[path = "tests_context_menu.rs"]
mod context_menu_tests;
#[cfg(test)]
#[path = "tests_daemon_lost_modal.rs"]
mod daemon_lost_modal_tests;
#[cfg(test)]
#[path = "tests_feeds_manage_popup.rs"]
mod feeds_manage_popup_tests;
#[cfg(test)]
#[path = "tests_help.rs"]
mod help_tests;
#[cfg(test)]
#[path = "tests_home_characterization.rs"]
mod home_characterization_tests;
#[cfg(test)]
#[path = "tests_library_characterization.rs"]
mod library_characterization_tests;
#[cfg(test)]
#[path = "tests_library_routes_popup.rs"]
mod library_routes_popup_tests;
#[cfg(test)]
#[path = "tests_multiselect.rs"]
mod multiselect_tests;
#[cfg(test)]
#[path = "tests_music_characterization.rs"]
mod music_characterization_tests;
#[cfg(test)]
#[path = "tests_music_groups.rs"]
mod music_group_tests;
#[cfg(test)]
#[path = "tests_non_music.rs"]
mod non_music_tests;
#[cfg(test)]
#[path = "tests_panel.rs"]
mod panel_tests;
#[cfg(test)]
#[path = "tests_playlists.rs"]
mod playlists_tests;
#[cfg(test)]
#[path = "tests_queue.rs"]
mod queue_tests;
#[cfg(test)]
#[cfg(test)]
#[path = "tests_scroll_pills.rs"]
mod scroll_pills_tests;
#[cfg(test)]
#[path = "tests_search_sidebar.rs"]
mod search_sidebar_tests;
#[cfg(test)]
#[path = "tests_sessions.rs"]
mod sessions_tests;
#[cfg(test)]
#[path = "tests_settings.rs"]
mod settings_tests;
#[cfg(test)]
#[path = "test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
pub(crate) use test_helpers::{
    make_movie_app, make_music_group_app, make_music_group_app_with_second_album, make_queue_app,
};
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[cfg(test)]
mod tests_conformance_matrix;
#[cfg(test)]
mod tests_feeds;
#[cfg(test)]
mod tests_podcast_panel;
#[cfg(test)]
mod tests_surface_conformance;
#[cfg(test)]
mod tests_surface_conformance_component_views;
#[cfg(test)]
mod tests_wide_hero_pane_characterization;
#[cfg(test)]
mod tests_wide_hero_split_override;
