use super::components::ShellRequest;
use super::Model;
use super::{ConfirmAction, ConfirmModal};
use crate::app::images::NAV_IMAGE_FETCH_IDLE_DELAY;
use std::time::Instant;

impl Model {
    /// Route the generic Emby browser's selected-item typed effects (task
    /// 5.3d, Emby browser effect decoupling) to their `App` handlers with the
    /// component-resolved owned target. `EmbyLibraryContent` resolves its own
    /// selected `EmbyItem` from its component-local cursor/content; the
    /// effect acts on that supplied item directly — never by copying the
    /// component cursor into a `BrowseLevel.cursor` and re-reading it. The
    /// active library index is derived from the shell's own tab state (the
    /// browser is mounted only for the active generic/Movies/home-video
    /// `EmbyLibrary` tab, same derivation as the `EmbyLibraryRow*`/`EmbyLibraryPillClick` mouse arms).
    /// A missing library index is a defensive no-op.
    pub(in crate::app) fn handle_emby_library_request(&mut self, request: ShellRequest) {
        let Some(lib_idx) = self.app.tab.emby_library_index() else {
            return;
        };
        match request {
            // A `Series` item routes through the shared Series-activation gate
            // first (task 3.4a): at non-Wide TV width — the only layout where
            // `TvContent` is mounted for a TV library — that opens the Library
            // Hero overlay instead of a flat drill-in. `false` means
            // it was not a Series (or had no id), so fall back to the normal
            // select-item path, including the folder scroll-persist.
            ShellRequest::EmbyLibraryActivate { item } => {
                if item.item_type == "Series"
                    && self.app.activate_selected_series_item(lib_idx, &item)
                {
                    // handled by Series activation (Library Hero overlay at
                    // non-Wide width, persistent workspace at wide)
                } else {
                    self.app.select_item(lib_idx, item);
                }
            }
            ShellRequest::EmbyLibraryPlay { item } => {
                self.app.play_or_activate_lib_item(lib_idx, item)
            }
            ShellRequest::EmbyLibraryEnqueue { item } => self.app.enqueue_lib_item(lib_idx, item),
            ShellRequest::EmbyLibraryToggleWatched { item } => {
                self.app.toggle_watched_item(lib_idx, item)
            }
            // Ctrl+S shuffles the supplied item with the preserved
            // `shuffle_play` tail: a folder item shuffles the folder itself;
            // a non-folder item shuffles the current browse level's parent
            // (falling back to the library id). The folder target comes from
            // the component-resolved item, never a `BrowseLevel.cursor`
            // re-read.
            ShellRequest::EmbyLibraryShuffle { item } => {
                self.app.shuffle_play_selected(lib_idx, item)
            }
            // Bare `r` refreshes the active Emby library (task 5.3d,
            // Emby browser refresh): the shell derives the active library
            // index from its own tab state and runs `App::refresh_lib` on it,
            // the same call the legacy `handle_lib_key` `Char('r')` arm made.
            ShellRequest::EmbyLibraryRefresh => self.app.refresh_lib(lib_idx),
            // Ctrl+`r` raises the Rescan Library confirmation (task 5.3d,
            // Emby browser rescan): same title/message/hint and
            // `ConfirmAction::RescanLibrary(lib_idx)` as the legacy
            // `handle_lib_key` CONTROL arm, derived from the shell's own tab
            // state (the library name comes from the active library).
            ShellRequest::EmbyLibraryRescan => {
                let name = self.app.libs[lib_idx].library.name.clone();
                self.app.ask_confirm(ConfirmModal {
                    title: " Rescan Library ".into(),
                    message: format!("Rescan '{name}'?"),
                    hint: "[y] Confirm    [Esc] Cancel".into(),
                    on_confirm: ConfirmAction::RescanLibrary(lib_idx),
                });
            }
            // Esc/Backspace go back through the browse history (task 5.3d,
            // Emby browser back): the shell derives the active Emby library
            // index from its own tab state and runs `App::go_back` on it, the
            // same call the legacy `handle_lib_key` `Esc | Backspace` arm
            // made — preserving synthetic-group/root guards, parent-cursor
            // restoration, season-level skip, persistence, and stale-index
            // behavior.
            ShellRequest::EmbyLibraryBack => {
                self.app.go_back(lib_idx);
            }

            // `[`/`]` group cycling is resolved locally by the mounted
            // Emby library component from its projected group-picker state.

            // Every local browser cursor key (arrows/hjkl, Page keys,
            // Home/End) resolves to an item index inside the component and
            // arrives here already resolved. Keep the resting-position write
            // and its navigation effects in this shell arm.
            ShellRequest::EmbyLibraryCursorIndex { index } => {
                if self.active_emby_library_owner_is_latest() {
                    return;
                }
                if lib_idx >= self.app.libs.len() {
                    return;
                }
                let now = Instant::now();
                let idle = now.duration_since(self.app.last_nav_at) >= NAV_IMAGE_FETCH_IDLE_DELAY;
                self.app.last_nav_at = now;
                self.app.mark_library_navigation(now);
                // Keep only the persistence-facing resting value, using the
                // index already resolved by the active control. This is not a
                // live mirror: ordinary content pushes never read it back.
                let valid_index = if self.app.is_feed_home_video_group_view(lib_idx) {
                    self.app.libs[lib_idx]
                        .feed_home_video
                        .as_ref()
                        .is_some_and(|state| index < state.selected_len())
                } else {
                    self.app.libs[lib_idx]
                        .nav_stack
                        .last()
                        .is_some_and(|level| index < level.items.len())
                };
                if valid_index {
                    if self.app.is_feed_home_video_group_view(lib_idx) {
                        if let Some(state) = self.app.libs[lib_idx].feed_home_video.as_mut() {
                            state.video_cursor = index;
                        }
                    } else if let Some(level) = self.app.libs[lib_idx].nav_stack.last_mut() {
                        level.set_resting_cursor(index);
                    }
                    self.app.save_default_library_position(lib_idx);
                }
                // Group pickers are a fixed local list and never paginate.
                if idle && !self.app.is_feed_home_video_group_view(lib_idx) {
                    self.app.maybe_fetch_next_page(lib_idx, index);
                }
            }
            // unreachable: shell/messages.rs top-level dispatch routes only the
            // EmbyLibrary* activate/effect group plus
            // EmbyLibraryCursorIndex into handle_emby_library_request; every one has
            // an arm above.
            _ => {}
        }
    }
}
