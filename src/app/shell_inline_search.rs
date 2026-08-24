use super::components::{BrowserKey, BrowserKind, ComponentId, InlineSearchComponent, SearchPool};
use super::shell::Model;
use super::{AlbumIndexState, PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;
use ratatui::layout::Rect;

impl Model {
    fn inline_search_component_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.libs.get(index)?;
        library.search.as_ref()?;
        Some(ComponentId::InlineSearch(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: BrowserKind::from_collection_type(&library.library.collection_type),
        }))
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

        let Some(id) = self.inline_search_id.as_ref() else {
            return;
        };
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let Some(search) = self.app.libs[index].search.as_ref() else {
            return;
        };
        let recursive = self.app.recursive_album_search_enabled(index);
        let library_id = self.app.libs[index].library.id.clone();
        let (pool, loading) = if recursive {
            match self.app.album_indexes.get(&library_id) {
                Some(AlbumIndexState::Ready(entries)) => {
                    let entries = search
                        .results
                        .iter()
                        .filter_map(|idx| entries.get(*idx).cloned())
                        .collect();
                    (SearchPool::Albums(entries), search.loading)
                }
                Some(AlbumIndexState::Loading { .. }) => (SearchPool::Albums(Vec::new()), true),
                _ => (SearchPool::Albums(Vec::new()), search.loading),
            }
        } else {
            let items = search
                .results
                .iter()
                .filter_map(|idx| search.items.get(*idx).cloned())
                .collect();
            (SearchPool::Items(items), search.loading)
        };
        let area = self.inline_search_area();
        let focused = matches!(self.app.effective_panel_focus(), PanelFocus::Library);
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(search_component) =
                comp.as_any_mut().downcast_mut::<InlineSearchComponent>()
            {
                search_component.set_content(
                    search.query.clone(),
                    pool,
                    loading,
                    search.cursor,
                    search.scroll,
                    focused,
                    area,
                );
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
    use tuirealm::component::AppComponent;
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
