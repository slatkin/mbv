use super::components::{BrowserKey, BrowserKind, ComponentId, InlineSearchComponent, SearchPool};
use super::shell::Model;
use super::{AlbumIndexState, PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;
use ratatui::layout::Rect;

impl Model {
    pub(super) fn inline_search_component_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.libs.get(index)?;
        let expected = ComponentId::InlineSearch(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: BrowserKind::from_collection_type(&library.library.collection_type),
        });
        // `Some` exactly when the search for this library is mounted.
        self.application.mounted(&expected).then_some(expected)
    }

    fn inline_search_area(&self) -> Rect {
        let main = &self.app.layout.main;
        if main.left_area.width > 0 && main.left_area.height > 0 {
            main.left_area
        } else if main.tv_wide_right_area.width > 0 {
            main.tv_wide_right_area
        } else if main.movies_wide_right_area.width > 0 {
            main.movies_wide_right_area
        } else {
            main.wide_music_browser_area
        }
    }

    /// Event-scoped projection replacing the deleted per-frame
    /// `sync_inline_search`. Runs only where the search's inputs actually
    /// change: `open_inline_search`, `activate_inline_search_item`, the
    /// `apply_inline_search_items` flat-result push (kept verbatim below),
    /// async library completions in the shell's `lib_rx` drain, and terminal
    /// resize. The projection is deterministic in `App` state, so pushing the
    /// same value again is idempotent; because the mounted search swallows
    /// keyboard and mouse (D16), panel-focus, panel-mode and tab transitions
    /// are unreachable while it is open except at those boundaries.
    ///
    pub(super) fn push_inline_search_content(&mut self) {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let Some(id) = self.inline_search_component_id(index) else {
            return;
        };
        let recursive = self.app.recursive_album_search_enabled(index);
        let library_id = self.app.libs[index].library.id.clone();
        let loading = recursive
            && matches!(
                self.app.album_indexes.get(&library_id),
                Some(AlbumIndexState::Loading { .. })
            );
        let pool = if recursive {
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
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(search_component) =
                comp.as_any_mut().downcast_mut::<InlineSearchComponent>()
            {
                search_component.set_content(pool, loading, focused);
                if recursive {
                    search_component.set_loading(loading);
                }
            }
        }
    }

    pub(super) fn open_inline_search(&mut self) {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        if self.inline_search_component_id(index).is_some() {
            return;
        }
        let Some(id) = self.app.libs.get(index).map(|library| {
            ComponentId::InlineSearch(BrowserKey {
                service: ServiceKind::Emby,
                library_id: library.library.id.clone(),
                kind: BrowserKind::from_collection_type(&library.library.collection_type),
            })
        }) else {
            return;
        };
        self.application
            .mount(id.clone(), Box::new(InlineSearchComponent::new()), vec![])
            .expect("mount inline library Search");
        self.application
            .active(&id)
            .expect("activate inline library Search");
        let recursive = self.app.recursive_album_search_enabled(index);
        let mut needs_full_load = false;
        if recursive {
            self.app.start_album_index(index, false);
        } else {
            needs_full_load = self.app.libs[index].nav_stack.last().is_some_and(|level| {
                level.all_items.is_none()
                    && (level.letter_filter.is_some() || level.items.len() < level.total_count)
            });
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

    pub(super) fn dismiss_inline_search(&mut self) {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        if let Some(id) = self.inline_search_component_id(index) {
            let _ = self.application.umount(&id);
        }
    }

    pub(super) fn activate_inline_search_item(&mut self, id: String, item_type: String) {
        let TabSelection::EmbyLibrary(lib_idx) = self.app.tab else {
            return;
        };
        let Some(component_id) = self.inline_search_component_id(lib_idx) else {
            return;
        };
        let selected = self
            .application
            .get_component(&component_id)
            .and_then(|component| component.as_any().downcast_ref::<InlineSearchComponent>())
            .and_then(InlineSearchComponent::selected_item);
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
                self.app.activate_recursive_album(lib_idx, entry);
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
        let Some(id) = self.inline_search_component_id(match self.app.tab {
            TabSelection::EmbyLibrary(index) => index,
            _ => return,
        }) else {
            return;
        };
        if let Some(component) = self.application.get_component_mut(&id) {
            if let Some(search) = component
                .as_any_mut()
                .downcast_mut::<InlineSearchComponent>()
            {
                search.set_loading(loading);
            }
        }
    }

    /// Drain tail for the inline search (called from the shell's `lib_rx`
    /// loop): completions that can change the mounted search's projected pool
    /// — flat `nav_stack` items/`all_items`, or recursive `album_indexes` —
    /// re-push it after the App handles the event. The deleted per-frame
    /// mirror's projection is driven at async event boundaries.
    pub(super) fn handle_inline_search_lib_event(&mut self, ev: super::LibEvent) {
        let pushes_inline_search = matches!(
            ev,
            super::LibEvent::Refreshed { .. }
                | super::LibEvent::AllItemsPrefetched { .. }
                | super::LibEvent::AlbumIndexBuilt { .. }
                | super::LibEvent::NavigateTo { .. }
        );
        self.app.handle_lib_event(ev);
        if pushes_inline_search {
            self.push_inline_search_content();
        }
    }

    pub(super) fn apply_inline_search_items(
        &mut self,
        lib_idx: usize,
        parent_id: String,
        items: Vec<mbv_core::api::EmbyItem>,
    ) {
        let TabSelection::EmbyLibrary(current_idx) = self.app.tab else {
            return;
        };
        if current_idx != lib_idx
            || self
                .app
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.nav_stack.last())
                .map(|level| level.parent_id.as_str())
                != Some(parent_id.as_str())
        {
            return;
        }
        let Some(id) = self.inline_search_component_id(lib_idx) else {
            return;
        };
        if let Some(component) = self.application.get_component_mut(&id) {
            if let Some(search) = component
                .as_any_mut()
                .downcast_mut::<InlineSearchComponent>()
            {
                search.set_content(SearchPool::Items(items), false, true);
                search.set_loading(false);
            }
        }
    }

    pub(super) fn render_inline_search_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.inline_search_component_id(match self.app.tab {
            TabSelection::EmbyLibrary(index) => index,
            _ => return,
        }) else {
            return;
        };
        let area = self.inline_search_area();
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(&id, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{InlineSearchComponent, LegacyTerminalEvent, Msg};
    use crate::app::render::make_movie_app;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn inline_library_search_shell_mounts_and_routes() {
        let mut model = Model::new(make_movie_app());
        model.open_inline_search();
        let id = model
            .inline_search_component_id(0)
            .expect("inline Search component mounted");
        assert!(model.application.mounted(&id));
        let message = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            message,
            Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
        ));
        assert!(model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<InlineSearchComponent>()
            .is_some());
    }
}
