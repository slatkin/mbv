use super::components::inline_search::InlineSearchHost;
use super::components::library_panel::LibraryPanel;
use super::components::{ComponentId, SearchPool};
use super::shell::Model;
use super::{AlbumIndexState, PanelFocus, TabSelection};

impl Model {
    /// The panel's active owner's Inline Search session, when one is
    /// embedded (Emby browser, TV, and Music owners do; Home/Feeds/podcast/
    /// book owners do not). The host path resolves through this so every
    /// owner that opens inline search gets the same shell load/push flow.
    fn active_inline_search_session(&mut self) -> Option<&mut dyn InlineSearchHost> {
        self.panel_mut()?.active_inline_search_session()
    }

    fn active_inline_search_session_ref(&self) -> Option<&dyn InlineSearchHost> {
        self.panel()?.active_inline_search_session_ref()
    }

    /// The mounted `LibraryPanel`, typed.
    fn panel(&self) -> Option<&LibraryPanel> {
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
    }

    fn panel_mut(&mut self) -> Option<&mut LibraryPanel> {
        self.application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
    }

    pub(crate) fn active_inline_search_is_open(&self) -> bool {
        self.active_inline_search_session_ref()
            .is_some_and(|host| host.inline_search().is_active())
    }

    fn with_active_inline_search_host(
        &mut self,
        f: impl FnOnce(&mut dyn InlineSearchHost),
    ) -> bool {
        let Some(host) = self.active_inline_search_session() else {
            return false;
        };
        f(host);
        true
    }

    fn inline_search_needs_full_load(&self, index: usize) -> bool {
        self.app.libs[index].nav_stack.last().is_some_and(|level| {
            level.all_items.is_none()
                && (level.letter_filter.is_some() || level.items.len() < level.total_count)
        })
    }

    pub(super) fn dismiss_active_inline_search(&mut self) {
        let _ = self.with_active_inline_search_host(|host| host.close_inline_search());
    }

    pub(super) fn push_inline_search_content(&mut self) {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let has_session = self
            .panel_mut()
            .is_some_and(|panel| panel.active_inline_search_session().is_some());
        if !has_session {
            return;
        }
        // Flat path: this push only projects the flat `Items` pool. Loading
        // is exactly while the whole-library fetch backing `all_items` is
        // outstanding (see `inline_search_needs_full_load`). Intermediate
        // pushes -- resize, browse completion, activation -- keep the spinner
        // up; the completion push (all_items now present) clears it.
        let loading = self.inline_search_needs_full_load(index);
        let recursive = self.app.recursive_album_search_enabled(index);
        let pool = if recursive {
            let library_id = self.app.libs[index].library.id.clone();
            match self.app.album_indexes.get(&library_id) {
                Some(AlbumIndexState::Ready(entries)) => SearchPool::Albums(entries.clone()),
                _ => SearchPool::Albums(Vec::new()),
            }
        } else {
            let items = self.app.libs[index]
                .nav_stack
                .last()
                .map(|level| {
                    level
                        .all_items
                        .clone()
                        .unwrap_or_else(|| level.items.clone())
                })
                .unwrap_or_default();
            SearchPool::Items(items)
        };
        let focused = matches!(self.app.effective_panel_focus(), PanelFocus::Library);
        self.with_active_inline_search_host(|host| {
            host.set_inline_search_content(pool, loading, focused);
        });
    }

    pub(super) fn open_inline_search(&mut self) {
        if !self.with_active_inline_search_host(|host| host.open_inline_search()) {
            return;
        }
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let recursive = self.app.recursive_album_search_enabled(index);
        let mut needs_full_load = false;
        if recursive {
            self.app.start_album_index(index, false);
        } else {
            needs_full_load = self.inline_search_needs_full_load(index);
            if needs_full_load {
                self.app.spawn_search_items_load(index);
            }
        }
        // Initial pool/loading/focus push (the deleted mirror's first-frame
        // projection, at the open event).
        self.push_inline_search_content();
        if (recursive
            && matches!(
                self.app.album_indexes.get(&self.app.libs[index].library.id),
                Some(AlbumIndexState::Loading { .. })
            ))
            || needs_full_load
        {
            self.set_inline_search_loading(true);
        }
    }

    pub(super) fn activate_inline_search_item(&mut self, id: String, item_type: String) {
        let TabSelection::EmbyLibrary(lib_idx) = self.app.tab else {
            return;
        };
        let selected = self
            .active_inline_search_session_ref()
            .and_then(|host| host.selected_inline_search_item());
        if self.app.recursive_album_search_enabled(lib_idx) {
            let library_id = self.app.libs[lib_idx].library.id.clone();
            let entry = match self.app.album_indexes.get(&library_id) {
                Some(AlbumIndexState::Ready(entries)) => entries
                    .iter()
                    .find(|entry| entry.album.id == id && entry.album.item_type == item_type)
                    .cloned(),
                _ => None,
            };
            if let Some(entry) = entry {
                // Enter on an album result returns to the standard library
                // presentation. `activate_recursive_album` is fully async: it
                // replaces the nav stack only once `LibEvent::RecursiveAlbumActivated`
                // drains, and that arm (shell_run.rs) solely owns the Music
                // workspace re-anchor, the track-selection one-shot, and the
                // content push -- all against the updated nav stack. Do the
                // dismiss here only on a successful spawn; on failure leave the
                // search open and change nothing else.
                if self.app.activate_recursive_album(lib_idx, entry) {
                    self.dismiss_active_inline_search();
                }
                return;
            }
        } else if let Some(item) =
            selected.filter(|item| item.id == id && item.item_type == item_type)
        {
            self.app.select_item(lib_idx, item);
        }
        // Activation may have navigated (flat folder push) or queued
        // playback; re-project the pool/focus at this event point, exactly
        // as the deleted per-frame mirror did on the following tick.
        self.push_inline_search_content();
    }

    fn set_inline_search_loading(&mut self, loading: bool) {
        self.with_active_inline_search_host(|host| {
            host.inline_search_mut().set_loading(loading);
        });
    }

    /// Drain tail for the inline search (called from the shell's `lib_rx`
    /// loop): completions that can change the mounted search's projected pool
    /// — flat nav_stack completions (`Loaded`), flat
    /// items/`all_items`, or recursive `album_indexes` —
    /// re-push it after the App handles the event. The deleted per-frame
    /// mirror's projection is driven at async event boundaries.
    pub(in crate::app) fn handle_inline_search_lib_event(&mut self, ev: super::LibEvent) {
        let pushes_inline_search = matches!(
            ev,
            super::LibEvent::Loaded { .. }
                | super::LibEvent::Refreshed { .. }
                | super::LibEvent::AllItemsPrefetched { .. }
                | super::LibEvent::AlbumIndexBuilt { .. }
                | super::LibEvent::NavigateTo { .. }
                | super::LibEvent::SearchItemsLoaded { .. }
        );
        if matches!(
            &ev,
            super::LibEvent::NavigateTo {
                switch_tab: true,
                ..
            }
        ) {
            self.dismiss_active_inline_search();
        }
        self.app.handle_lib_event(ev);
        if pushes_inline_search {
            self.push_inline_search_content();
        }
    }
}
