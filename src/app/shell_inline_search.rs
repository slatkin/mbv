use super::components::inline_search::InlineSearchHost;
use super::components::{
    BrowserComponent, BrowserKey, BrowserKind, ComponentId, InlineSearchComponent,
    MusicWorkspaceComponent, SearchPool, TvWorkspaceComponent,
};
use super::shell::Model;
use super::{AlbumIndexState, PanelFocus, TabSelection};
use crate::app::render::shared_hero_presentation;
use mbv_core::config::ServiceKind;
use ratatui::layout::Rect;

impl Model {
    pub(crate) fn active_inline_search_is_open(&self) -> bool {
        let Some(id) = self.active_inline_search_host() else {
            return false;
        };
        self.application
            .get_component(&id)
            .is_some_and(|component| {
                component
                    .as_any()
                    .downcast_ref::<BrowserComponent>()
                    .is_some_and(|host| host.inline_search().is_active())
                    || component
                        .as_any()
                        .downcast_ref::<MusicWorkspaceComponent>()
                        .is_some_and(|host| host.inline_search().is_active())
                    || component
                        .as_any()
                        .downcast_ref::<TvWorkspaceComponent>()
                        .is_some_and(|host| host.inline_search().is_active())
            })
    }

    fn active_inline_search_host(&self) -> Option<ComponentId> {
        if let Some(id) = self.tv_workspace_component_id() {
            if self.application.mounted(&id) {
                return Some(id);
            }
        }
        if let Some(id) = self.music_workspace_component_id() {
            if self.application.mounted(&id) {
                return Some(id);
            }
        }
        self.emby_browser_component_id()
            .filter(|id| self.application.mounted(id))
    }

    fn with_active_inline_search_host(
        &mut self,
        f: impl FnOnce(&mut dyn InlineSearchHost),
    ) -> bool {
        let Some(id) = self.active_inline_search_host() else {
            return false;
        };
        let Some(component) = self.application.get_component_mut(&id) else {
            return false;
        };
        if let Some(host) = component.as_any_mut().downcast_mut::<BrowserComponent>() {
            f(host);
            return true;
        }
        if let Some(host) = component
            .as_any_mut()
            .downcast_mut::<MusicWorkspaceComponent>()
        {
            f(host);
            return true;
        }
        if let Some(host) = component
            .as_any_mut()
            .downcast_mut::<TvWorkspaceComponent>()
        {
            f(host);
            return true;
        }
        false
    }

    fn inline_search_expected_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.libs.get(index)?;
        Some(ComponentId::InlineSearch(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: BrowserKind::from_collection_type(&library.library.collection_type),
        }))
    }

    pub(super) fn inline_search_component_id(&self, index: usize) -> Option<ComponentId> {
        let expected = self.inline_search_expected_id(index)?;
        // `Some` exactly when the search for this library is mounted.
        self.application.mounted(&expected).then_some(expected)
    }

    /// Remove searches left mounted for a library that is no longer active.
    /// TuiRealm has no component enumeration, so derive every possible inline
    /// search id from the current library list and use `mounted()` as the
    /// registry source of truth.
    fn unmount_stale_inline_searches(&mut self, keep: Option<&ComponentId>) {
        let stale_ids: Vec<_> = self
            .app
            .libs
            .iter()
            .map(|library| {
                ComponentId::InlineSearch(BrowserKey {
                    service: ServiceKind::Emby,
                    library_id: library.library.id.clone(),
                    kind: BrowserKind::from_collection_type(&library.library.collection_type),
                })
            })
            .filter(|id| keep != Some(id) && self.application.mounted(id))
            .collect();
        for id in stale_ids {
            let _ = self.application.umount(&id);
            self.unregister_destination(&id);
        }
    }

    fn inline_search_area(&self) -> Rect {
        let main = &self.app.layout.main;
        if main.tv_wide_right_area.width > 0 {
            main.tv_wide_right_area
        } else if main.left_area.width > 0 && main.left_area.height > 0 {
            main.left_area
        } else {
            main.wide_music_browser_area
        }
    }

    /// Event-scoped projection replacing the deleted per-frame
    /// `sync_inline_search`. Runs only where the search's inputs actually
    /// change: `open_inline_search`, `activate_inline_search_item`, the
    /// `SearchItemsLoaded` whole-library fetch completion (re-homed from the
    /// deleted direct flat-result projector), async library completions in
    /// the shell's `lib_rx` drain, and terminal resize. The projection is
    /// deterministic in `App` state, so pushing the same value again is
    /// idempotent; because the mounted search swallows keyboard and mouse
    /// (D16), panel-focus, panel-mode and tab transitions are unreachable
    /// while it is open except at those boundaries.
    ///
    /// Whether the flat (non-recursive) inline search needs its
    /// whole-library fetch: `all_items` -- the full unfiltered pool backing
    /// fuzzy search -- is missing from the current nav level, and the level
    /// is not already fully materialized in `items`. The identical predicate
    /// arms the load at `open_inline_search` and drives the component's
    /// loading flag on every push: pending until the `SearchItemsLoaded`
    /// completion writes `all_items`, then cleared (5.3d.20c).
    fn inline_search_needs_full_load(&self, index: usize) -> bool {
        self.app.libs[index].nav_stack.last().is_some_and(|level| {
            level.all_items.is_none()
                && (level.letter_filter.is_some() || level.items.len() < level.total_count)
        })
    }

    pub(super) fn push_inline_search_content(&mut self) {
        let expected = match self.app.tab {
            TabSelection::EmbyLibrary(index) => self.inline_search_expected_id(index),
            _ => None,
        };
        let Some(id) = expected.as_ref().filter(|id| self.application.mounted(id)) else {
            self.unmount_stale_inline_searches(expected.as_ref());
            return;
        };
        let id = id.clone();
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        // Flat path: this push only projects the flat `Items` pool. Loading
        // is exactly while the whole-library fetch backing `all_items` is
        // outstanding (see `inline_search_needs_full_load`). Intermediate
        // pushes -- resize, browse completion, activation -- keep the spinner
        // up; the completion push (all_items now present) clears it.
        let loading = self.inline_search_needs_full_load(index);
        let pool = {
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
                // Drive loading from the projection on every push: the
                // completion push (flat `all_items` landed) is what clears
                // it (`set_content` only ever turns it on, so an intermediate
                // push never wedges or clears early).
                search_component.set_loading(loading);
            }
        }
    }

    pub(super) fn open_inline_search(&mut self) {
        if self.with_active_inline_search_host(|host| host.open_inline_search()) {
            self.push_inline_search_content();
            return;
        }
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            self.unmount_stale_inline_searches(None);
            return;
        };
        let Some(id) = self.inline_search_expected_id(index) else {
            return;
        };
        if self.application.mounted(&id) {
            return;
        }
        self.unmount_stale_inline_searches(Some(&id));
        self.application
            .mount(id.clone(), Box::new(InlineSearchComponent::new()), vec![])
            .expect("mount inline library Search");
        self.register_destination(&id);
        self.application
            .active(&id)
            .expect("activate inline library Search");
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

    pub(super) fn dismiss_inline_search(&mut self) {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        if let Some(id) = self.inline_search_component_id(index) {
            let _ = self.application.umount(&id);
            self.unregister_destination(&id);
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
                | super::LibEvent::SearchItemsLoaded { .. }
        );
        self.app.handle_lib_event(ev);
        if pushes_inline_search {
            self.push_inline_search_content();
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
        let wide = shared_hero_presentation(self.app.layout.main.left_area).is_some();
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(search) = comp.as_any_mut().downcast_mut::<InlineSearchComponent>() {
                search.set_wide(wide);
            }
        }
        self.application.view(&id, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::render::make_movie_app;
    use crate::app::tests::make_item;
    use crate::app::{LibEvent, LibraryTab};
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
        assert_eq!(message, None);
        assert!(model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<InlineSearchComponent>()
            .is_some());
    }

    #[test]
    fn inline_search_tab_switch_unmounts_stale_component_before_open() {
        let mut app = make_movie_app();
        let mut stale_library = LibraryTab::new(app.libs[0].library.clone());
        stale_library.library.id = "lib-stale".into();
        app.libs.push(stale_library);
        app.tab = TabSelection::EmbyLibrary(1);

        let mut model = Model::new(app);
        model.open_inline_search();
        let stale_id = model
            .inline_search_component_id(1)
            .expect("stale inline Search component mounted");

        model.handle_inline_search_lib_event(LibEvent::NavigateTo {
            lib_idx: 0,
            nav_stack: Vec::new(),
            switch_tab: true,
        });
        assert!(!model.application.mounted(&stale_id));

        model.open_inline_search();
        let current_id = model
            .inline_search_component_id(0)
            .expect("current inline Search component mounted");
        assert!(model.application.mounted(&current_id));
    }

    fn search_component<'a>(model: &'a Model, id: &ComponentId) -> &'a InlineSearchComponent {
        model
            .application
            .get_component(id)
            .unwrap()
            .as_any()
            .downcast_ref::<InlineSearchComponent>()
            .unwrap()
    }

    #[test]
    fn inline_search_items_loaded_rehomes_all_items_and_clears_loading() {
        // Flat search that needs a whole-library fetch: the stub level is
        // fully loaded (2/2), so under-cut its `total_count` to arm the
        // `SearchItemsLoaded` load at open.
        let mut app = make_movie_app();
        app.libs[0]
            .nav_stack
            .last_mut()
            .expect("stub browse level")
            .total_count = 10;
        let mut model = Model::new(app);
        model.open_inline_search();
        let id = model
            .inline_search_component_id(0)
            .expect("inline Search component mounted");
        assert!(
            search_component(&model, &id).test_loading(),
            "load armed at open"
        );

        // Stale completion (nav level moved on): `all_items` untouched and
        // the spinner stays up -- a stale write or early clear would wedge
        // or corrupt the pool.
        model.handle_inline_search_lib_event(LibEvent::SearchItemsLoaded {
            lib_idx: 0,
            parent_id: "stale-parent".into(),
            items: vec![make_item("Stale", "Movie")],
        });
        assert!(
            model.app.libs[0]
                .nav_stack
                .last()
                .expect("browse level")
                .all_items
                .is_none(),
            "stale completion must not write all_items"
        );
        assert!(
            search_component(&model, &id).test_loading(),
            "stale completion keeps loading"
        );

        // Correct completion: full items land in `all_items`, the component
        // projects them, and loading clears (no wedge).
        let fresh = vec![make_item("Alpha", "Movie"), make_item("Beta", "Movie")];
        model.handle_inline_search_lib_event(LibEvent::SearchItemsLoaded {
            lib_idx: 0,
            parent_id: "lib-movies".into(),
            items: fresh.clone(),
        });
        let level = model.app.libs[0].nav_stack.last().expect("browse level");
        assert_eq!(
            level.all_items.as_deref(),
            Some(fresh.as_slice()),
            "correct completion writes all_items"
        );
        assert!(
            !search_component(&model, &id).test_loading(),
            "correct completion must clear loading"
        );
        assert_eq!(
            search_component(&model, &id).test_pool_item_ids(),
            fresh.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
            "full items projected into the component"
        );
    }
}
