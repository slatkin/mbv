#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{
        FeedsManageComponent, LibraryRoutesComponent, Msg, MultiselectComponent,
        PlaybackPromptComponent, SelectionModalComponent, ShellRequest,
    };
    use crate::app::tests::make_app_stub;
    use crate::app::types_context_menu::{LibraryRoutePopup, LibraryRouteStage};
    use crate::app::types_context_menu::{MultiSelectKind, MultiSelectPopup};
    use crate::app::types_feeds_manage::FeedsManagePopup;
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalItem, SelectionModalListState, SelectionModalRow,
        SelectionModalSource,
    };
    use tuirealm::component::AppComponent;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn playback_prompt_shell_sync_mounts_and_mirrors_status() {
        let mut model = Model::new(make_app_stub());
        model.app.skip_intro_end_ticks = Some(100);
        model.app.status = "Skip intro? (Y/n)".into();

        model.sync_playback_prompt();

        let id = ComponentId::PlaybackPrompt;
        assert!(model.application.mounted(&id));
        let component = model
            .application
            .get_component(&id)
            .expect("Playback prompt mounted")
            .as_any()
            .downcast_ref::<PlaybackPromptComponent>()
            .expect("Playback prompt type");
        assert_eq!(component.status(), "Skip intro? (Y/n)");

        model.app.skip_intro_end_ticks = None;
        model.sync_playback_prompt();
        assert!(!model.application.mounted(&id));
    }

    #[test]
    fn selection_modal_shell_syncs_and_routes_dismissal() {
        let mut model = Model::new(make_app_stub());
        model.app.pending_overlay = Some(
            crate::app::types_overlay::OverlayRequest::SelectionModal(SelectionModal {
                source: SelectionModalSource::Album {
                    album_id: "album-1".into(),
                },
                title: "Tracks".into(),
                state: SelectionModalListState::Ready(vec![SelectionModalRow::Item(
                    SelectionModalItem {
                        name: "Track".into(),
                        meta: String::new(),
                        id: "track-1".into(),
                    },
                )]),
                cursor: 0,
                filter: None,
            }),
        );
        model.sync_modal_requests();

        let id = ComponentId::Overlay(OverlayId::SelectionModal);
        assert!(model.application.mounted(&id));
        let message = {
            let component = model
                .application
                .get_component_mut(&id)
                .expect("Selection modal mounted")
                .as_any_mut()
                .downcast_mut::<SelectionModalComponent>()
                .expect("Selection modal type");
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Esc,
                modifiers: KeyModifiers::NONE,
            }))
        };
        let Some(Msg::Shell(request)) = message else {
            panic!("Selection modal should emit a shell request");
        };
        model.handle_selection_modal_request(request);
        model.sync_modal_requests();

        assert!(!model.application.mounted(&id));
    }

    #[test]
    fn series_season_completion_refreshes_the_selected_modal_pill_in_place() {
        let mut model = Model::new(make_app_stub());
        let mut season_one = crate::app::tests::make_item("Season 1", "Season");
        season_one.id = "season-1".into();
        let mut season_two = crate::app::tests::make_item("Season 2", "Season");
        season_two.id = "season-2".into();
        let mut episode_one = crate::app::tests::make_item("Pilot", "Episode");
        episode_one.id = "episode-1".into();
        let mut episodes = std::collections::HashMap::new();
        episodes.insert("season-1".into(), vec![episode_one]);
        model.app.series_detail_cache.insert(
            "series-1".into(),
            crate::app::SeriesDetail {
                seasons: vec![season_one, season_two],
                episodes,
            },
        );
        model.app.pending_overlay = Some(
            crate::app::types_overlay::OverlayRequest::SelectionModal(SelectionModal {
                source: SelectionModalSource::Series {
                    series_id: "series-1".into(),
                },
                title: "Series".into(),
                state: SelectionModalListState::Loading,
                cursor: 0,
                filter: Some(crate::app::types_selection_modal::SelectionModalFilter {
                    labels: vec!["01".into(), "02".into()],
                    selected: 1,
                }),
            }),
        );
        model.sync_modal_requests();

        let mut finale = crate::app::tests::make_item("Finale", "Episode");
        finale.id = "episode-2".into();
        model
            .app
            .handle_lib_event(crate::app::LibEvent::SeriesSeasonEpisodesFetched {
                series_id: "series-1".into(),
                season_id: "season-2".into(),
                episodes: vec![finale],
            });
        model.sync_modal_requests();
        model.sync_modal_requests();

        let id = ComponentId::Overlay(OverlayId::SelectionModal);
        let component = model
            .application
            .get_component(&id)
            .expect("Selection modal mounted")
            .as_any()
            .downcast_ref::<SelectionModalComponent>()
            .expect("Selection modal type");
        assert_eq!(component.filter_selected(), Some(1));
        assert_eq!(component.selected_id(), Some("episode-2"));
    }

    #[test]
    fn settings_popup_multiselect_shell_syncs_and_commits_component_choices() {
        let mut model = Model::new(make_app_stub());
        let id = ComponentId::Popup(PopupId::Multiselect);
        model
            .application
            .mount(id.clone(), Box::new(MultiselectComponent::new()), vec![])
            .expect("mount Multiselect");
        model.application.active(&id).expect("activate Multiselect");
        let popup = MultiSelectPopup {
            kind: MultiSelectKind::HiddenLibraries,
            items: vec![
                ("movies".into(), "Movies".into(), true),
                ("shows".into(), "Shows".into(), false),
            ],
            cursor: 0,
        };
        if let Some(comp) = model.application.get_component_mut(&id) {
            if let Some(multiselect) = comp.as_any_mut().downcast_mut::<MultiselectComponent>() {
                multiselect.set_content(&popup);
            }
        }

        let message = {
            let component = model
                .application
                .get_component_mut(&id)
                .expect("Multiselect mounted")
                .as_any_mut()
                .downcast_mut::<MultiselectComponent>()
                .expect("Multiselect type");
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Enter,
                modifiers: KeyModifiers::NONE,
            }))
        };
        let Some(Msg::Shell(ShellRequest::MultiselectCommit { .. })) = message else {
            panic!("Multiselect should emit a shell request");
        };
        model.handle_multiselect_commit();
        assert_eq!(
            model.app.config.lock().unwrap().hidden_libraries,
            vec!["movies".to_string()],
            "hidden selection must persist to config"
        );
        assert!(!model.application.mounted(&id));
    }

    #[test]
    fn settings_popup_library_routes_shell_syncs_and_routes_escape() {
        let mut model = Model::new(make_app_stub());
        let id = ComponentId::Popup(PopupId::LibraryRoutes);
        model
            .application
            .mount(id.clone(), Box::new(LibraryRoutesComponent::new()), vec![])
            .expect("mount Library routes");
        model
            .application
            .active(&id)
            .expect("activate Library routes");
        let popup = LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary {
                items: vec![("movies".into(), "Movies".into(), None)],
            },
            cursor: 0,
        };
        if let Some(comp) = model.application.get_component_mut(&id) {
            if let Some(routes) = comp.as_any_mut().downcast_mut::<LibraryRoutesComponent>() {
                routes.set_content(&popup);
            }
        }

        let message = {
            let component = model
                .application
                .get_component_mut(&id)
                .expect("Library routes mounted")
                .as_any_mut()
                .downcast_mut::<LibraryRoutesComponent>()
                .expect("Library routes type");
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Esc,
                modifiers: KeyModifiers::NONE,
            }))
        };
        let Some(Msg::Shell(ShellRequest::LibraryRoutesEsc)) = message else {
            panic!("Library routes should emit a shell request");
        };
        model.handle_library_routes_request(ShellRequest::LibraryRoutesEsc);

        assert!(!model.application.mounted(&id));
    }

    #[test]
    fn settings_popup_feeds_manage_shell_syncs_and_routes_escape() {
        let mut model = Model::new(make_app_stub());
        model.open_feeds_manage();
        let id = ComponentId::Popup(PopupId::FeedManage);
        assert!(model.application.mounted(&id));

        let message = {
            let component = model
                .application
                .get_component_mut(&id)
                .expect("Feed management mounted")
                .as_any_mut()
                .downcast_mut::<FeedsManageComponent>()
                .expect("Feed management type");
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Esc,
                modifiers: KeyModifiers::NONE,
            }))
        };
        let Some(Msg::Shell(ShellRequest::FeedsManageKey(key))) = message else {
            panic!("Feed management should emit a shell request");
        };
        model.handle_feeds_manage_request(key);

        assert!(model.feeds_manage.is_none());
        assert!(!model.application.mounted(&id));
    }
}
