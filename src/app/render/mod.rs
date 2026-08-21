mod arrangements;
mod components;
mod screens;
mod theme;

// Re-exports so paths that resolved at `render::X` (or, from render's other
// submodules, `super::X`) before the render/mod.rs split (issue #365 step 2,
// lane C) keep resolving without editing any call site. The `pub(crate)`
// trio is referenced from outside `render` entirely (src/app/mod.rs,
// src/app/actions.rs); the rest are referenced via `super::X` from render's
// sibling submodules (album, card, detail, home, list, music, pills, queue)
// and/or `use super::*` in render/tests.rs.
use components::chrome::LIST_PLAY_ICON;
pub use components::indicators;
use components::list_rows::{selection_marker, MarkerEdge};
use components::widgets::{
    content_width, render_count_label, render_pill_bar, render_placeholder, render_right_scrollbar,
    render_right_scrollbar_with_viewport, render_selected_block_background,
    render_selected_block_borders, PillBar, SelectedBlockBorderStyle, MUSIC_ALBUM_IMAGE_TYPES,
    RENDER_FILTER,
};
pub(super) use screens::album_plan::sorted_group_album_order;
pub(super) use screens::sort_filter::{
    effective_sort_str, letter_bucket, parse_album_folder_name, strip_article,
};
pub(crate) use screens::sort_filter::{
    initial_group_artist_sort_key, LetterFilter, LIBRARY_PILL_THRESHOLD,
};
// `theme`'s roles are re-exported here (rather than reached directly) so
// `palette.rs` — a sibling of `render`, not a descendant — can bridge to them;
// see `palette.rs`'s own re-export.
pub(crate) use theme::{
    resolve_surface_focus, ACCENT, ACCENT_ACTIVE, ACCENT_AUDIOBOOKSHELF, BORDER_UNFOCUSED,
    INDICATOR_AUDIO_FG, INDICATOR_RESOLUTION_FG, PILL_BG, PILL_FG, PILL_OVERFLOW_FG, PILL_ROW_BG,
    PILL_SELECTED_BG, PILL_SELECTED_FG, PLAYBACK_META_FG, PLAYBACK_VALUE_FG, PROGRESS_TRACK,
    QUEUE_ROW_FG, SCROLLBAR, STATUS_AVAILABLE, STATUS_ERROR, SURFACE_ACCENT_SOFT, SURFACE_BACKDROP,
    SURFACE_CHROME, SURFACE_FOCUSED, SURFACE_ITEM_FOCUSED, SURFACE_PLAYBACK, SURFACE_RESTING,
    SURFACE_SIDEBAR, SURFACE_STATUS_PILL, TEXT_ACCENT_MUTED, TEXT_DETAIL_META, TEXT_EMPHASIS,
    TEXT_FOCUS_ACCENT, TEXT_METADATA, TEXT_MUTED, TEXT_ON_ACCENT, TEXT_PRIMARY, TEXT_SECONDARY,
    TEXT_STRONG, TOAST_BG_ERROR, TOAST_BG_NEUTRAL, TOAST_BG_SUCCESS, TOAST_BG_WARNING, TOAST_FG,
};

use super::ui_util::natural_sort_key;
use super::{palette, App};

// Test-only: these names are otherwise unused in the production build (their
// only production callers moved into root.rs/queue.rs under screens/, which
// import them directly), but render/tests.rs and friends still reach them via
// `use super::*`.
#[cfg(test)]
use super::PanelFocus;
#[cfg(test)]
use crate::app::layout::LayoutMain;
#[cfg(test)]
use components::widgets::right_panel_content_area;
#[cfg(test)]
use mbv_core::api::TICKS_PER_SECOND;
#[cfg(test)]
use ratatui::layout::Rect;
#[cfg(test)]
use unicode_width::UnicodeWidthStr;

#[cfg(test)]
#[path = "tests_album_focus.rs"]
mod album_focus_tests;
#[cfg(test)]
#[path = "tests_album_listing.rs"]
mod album_listing_tests;
#[cfg(test)]
#[path = "tests_audiobookshelf_books.rs"]
mod audiobookshelf_book_tests;
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
#[path = "tests_remote_reanchor.rs"]
mod remote_reanchor_tests;
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
#[path = "tests.rs"]
mod tests;
#[cfg(test)]
mod tests_audiobookshelf_podcasts;
#[cfg(test)]
mod tests_feeds;
#[cfg(test)]
mod tests_home_inline;
