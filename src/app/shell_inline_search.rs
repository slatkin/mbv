use super::components::{BrowserKey, BrowserKind, ComponentId, InlineSearchComponent, SearchPool};
use super::shell::Model;
use super::{AlbumIndexState, PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;
use ratatui::layout::Rect;

impl Model {
    fn inline_search_component_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.libs.get(index)?;
        let expected = ComponentId::InlineSearch(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: BrowserKind::from_collection_type(&library.library.collection_type),
        });
        (self.inline_search_id.as_ref() == Some(&expected)).then_some(expected)
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

    pub(super) fn sync_inline_search(&mut self) {
        let next_id = match self.app.tab {
            TabSelection::EmbyLibrary(index) => self.inline_search_component_id(index),
            _ => None,
        };
        if self.inline_search_id != next_id {
            if let Some(id) = self.inline_search_id.take() {
                let _ = self.application.umount(&id);
            }
            if let Some(id) = next_id.clone() {
                self.application
                    .mount(id.clone(), Box::new(InlineSearchComponent::new()), vec![])
                    .expect("mount inline library Search");
                self.application
                    .active(&id)
                    .expect("activate inline library Search");
                self.inline_search_id = Some(id);
            }
        }

        let Some(id) = self.inline_search_id.as_ref().cloned() else {
            return;
        };
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let recursive = self.app.recursive_album_search_enabled(index);
        let library_id = self.app.libs[index].library.id.clone();
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
        let area = self.inline_search_area();
        let focused = matches!(self.app.effective_panel_focus(), PanelFocus::Library);
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(search_component) =
                comp.as_any_mut().downcast_mut::<InlineSearchComponent>()
            {
                search_component.set_content(pool, false, focused, area);
            }
        }
    }

    pub(super) fn open_inline_search(&mut self) {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        if self.inline_search_id.is_some() {
            return;
        }
        let Some(id) = self.app.libs.get(index).and_then(|library| {
            Some(ComponentId::InlineSearch(BrowserKey {
                service: ServiceKind::Emby,
                library_id: library.library.id.clone(),
                kind: BrowserKind::from_collection_type(&library.library.collection_type),
            }))
        }) else {
            return;
        };
        self.application
            .mount(id.clone(), Box::new(InlineSearchComponent::new()), vec![])
            .expect("mount inline library Search");
        self.application
            .active(&id)
            .expect("activate inline library Search");
        self.inline_search_id = Some(id);
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
        self.sync_inline_search();
        if recursive
            && matches!(
                self.app.album_indexes.get(&self.app.libs[index].library.id),
                Some(AlbumIndexState::Loading { .. })
            )
        {
            self.set_inline_search_loading(true);
        } else if needs_full_load {
            self.set_inline_search_loading(true);
        }
    }

    pub(super) fn dismiss_inline_search(&mut self) {
        if let Some(id) = self.inline_search_id.take() {
            let _ = self.application.umount(&id);
        }
    }

    pub(super) fn activate_inline_search_item(&mut self, id: String, item_type: String) {
        let TabSelection::EmbyLibrary(lib_idx) = self.app.tab else {
            return;
        };
        let Some(component_id) = self.inline_search_id.as_ref() else {
            return;
        };
        let selected = self
            .application
            .get_component(component_id)
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
    }

    fn set_inline_search_loading(&mut self, loading: bool) {
        let Some(id) = self.inline_search_id.as_ref() else {
            return;
        };
        if let Some(component) = self.application.get_component_mut(id) {
            if let Some(search) = component
                .as_any_mut()
                .downcast_mut::<InlineSearchComponent>()
            {
                search.set_loading(loading);
            }
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
        let Some(id) = self.inline_search_id.as_ref() else {
            return;
        };
        let area = self.inline_search_area();
        if let Some(component) = self.application.get_component_mut(id) {
            if let Some(search) = component
                .as_any_mut()
                .downcast_mut::<InlineSearchComponent>()
            {
                search.set_content(SearchPool::Items(items), false, true, area);
                search.set_loading(false);
            }
        }
    }

    pub(super) fn render_inline_search_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.inline_search_id.as_ref() else {
            return;
        };
        let area = self.inline_search_area();
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(id, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{InlineSearchComponent, LegacyTerminalEvent, Msg};
    use crate::app::render::make_movie_app;
    use crate::app::LibSearch;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn inline_library_search_shell_mounts_and_routes() {
        let mut model = Model::new(make_movie_app());
        model.app.libs[0].search = Some(LibSearch {
            query: "movie".into(),
            items: vec![crate::app::tests::make_item("Movie", "Movie")],
            results: vec![0],
            cursor: 0,
            scroll: 0,
            loading: false,
        });
        model.sync_inline_search();
        let id = model
            .inline_search_id
            .clone()
            .expect("inline Search component mounted");
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
            Some(Msg::Legacy(LegacyTerminalEvent::Key(_)))
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
