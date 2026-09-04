#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{
        FeedsManageComponent, LibraryRoutesComponent, Msg, MultiselectComponent,
        SelectionModalComponent, ShellRequest, UserEvent,
    };
    use crate::app::tests::make_app_stub;
    use crate::app::types_context_menu::{LibraryRoutePopup, LibraryRouteStage};
    use crate::app::types_context_menu::{MultiSelectKind, MultiSelectPopup};
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalItem, SelectionModalListState, SelectionModalRow,
        SelectionModalSource,
    };
    use tuirealm::component::AppComponent;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};


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
        let Some(Msg::Shell(ShellRequest::FeedsManageIntent(intent))) = message else {
            panic!("Feed management should emit a shell request");
        };
        model.handle_feeds_manage_intent(intent);

        assert!(model.feeds_manage.is_none());
        assert!(!model.application.mounted(&id));
    }

    /// Production-style acceptance test for #609 / #607: the search sidebar
    /// debounce must dispatch in a real Model shell, not just in the
    /// component's `handle_clock`-via-unit-test shortcut. The shell's
    /// `tick_search_clock` sweep calls the component's `tick_clock(Instant::
    /// now())`, and any emitted `Msg` flows through `handle_service_request`
    /// — exactly mirroring the main-loop wiring at `shell_run.rs`'s
    /// `drain_search_results` block.
    ///
    /// The component anchors `debounce_deadline` to `Instant::now()` at
    /// keystroke time, so this test uses one real `sleep(310 ms)` rather
    /// than fudging timestamps; 305 / 10 ms of slack for scheduling jitter.
    /// #[cfg(miri)] could swap to an injectable clock, but a 310 ms test
    /// wake-up is cheaper than a `Clock` seam that exists only for tests.
    #[test]
    fn search_sidebar_debounce_dispatches_in_a_mounted_shell() {
        use crate::app::components::{SearchSidebarComponent, ServiceRequest};
        use std::time::{Duration, Instant};

        let mut model = Model::new(make_app_stub());
        model.mount_sidebar(super::super::SidebarId::Search);
        let search_id = ComponentId::Overlay(OverlayId::Search);
        assert!(model.application.mounted(&search_id));

        // Type 'a' 'b' via the component's keyboard arm so the debounce
        // is armed the same way it is in production (FreeInstance / keyboard
        // input path). `dispatch` resolves the downcast so the search
        // component receives the event exactly the way TuiRealm's `tick`
        // would hand it off.
        let type_key = |c: char| -> Event<UserEvent> {
            Event::Keyboard(KeyEvent {
                code: Key::Char(c),
                modifiers: KeyModifiers::NONE,
            })
        };

        let dispatch = |model: &mut Model, ev: &Event<UserEvent>| {
            model
                .application
                .get_component_mut(&search_id)
                .expect("search sidebar mounted")
                .as_any_mut()
                .downcast_mut::<SearchSidebarComponent>()
                .expect("search sidebar type")
                .on(ev)
        };
        assert!(dispatch(&mut model, &type_key('a')).is_none());
        assert!(dispatch(&mut model, &type_key('b')).is_none());

        // Sweep before the 300 ms deadline: should not fire.
        assert!(model.tick_search_clock(Instant::now()).is_none());

        std::thread::sleep(Duration::from_millis(310));

        // Sweep after the deadline: the production run loop calls
        // handle_service_request on the returned Msg. With no Emby client
        // in the stub, the dispatch is a no-op (same code path as a user
        // without a configured service), but the Msg must traverse the
        // service-request router so the wiring is exercised end-to-end.
        let dispatched = model
            .tick_search_clock(Instant::now())
            .expect("tick_search_clock must emit after deadline");
        let Msg::Service(request) = dispatched else {
            panic!("search debounce must emit Msg::Service, got {dispatched:?}");
        };
        model.handle_service_request(request);

        // The component cleared both `debounce_pending` and
        // `debounce_deadline` on fire; this is the proof point that the
        // production sweep path took the dispatch branch (vs an early
        // return None).
        let component = model
            .application
            .get_component(&search_id)
            .expect("search sidebar still mounted")
            .as_any()
            .downcast_ref::<SearchSidebarComponent>()
            .expect("search sidebar type");
        assert!(
            component.debounce_pending.is_none(),
            "debounce_pending must clear after dispatch"
        );
        assert!(
            component.debounce_deadline.is_none(),
            "debounce_deadline must clear after dispatch"
        );

        // A second sweep after fire returns None — the debounce is now
        // empty until the next keystroke re-arms it.
        assert!(
            model.tick_search_clock(Instant::now()).is_none(),
            "post-dispatch sweep must not re-fire"
        );

        // Pin the expected request variant so the chained rename / shape
        // change of ServiceRequest::SearchQuery blows up here, not at the
        // assertion site of an unrelated caller.
        let _ = ServiceRequest::SearchQuery;
    }

    /// Task 5.4: the context menu's mouse click path must execute the entry
    /// *and* close the menu (the shell owns the dismissal, task 5.3c).
    #[test]
    fn context_menu_click_select_executes_and_closes_the_menu() {
        use crate::app::components::ContextMenuComponent;
        use crate::app::types_context_menu::{
            ContextAction, ContextMenu, ContextMenuAnchor, ContextMenuEntry,
        };
        use ratatui::layout::Rect;
        use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};

        let mut model = Model::new(make_app_stub());
        model.app.pending_overlay = Some(
            crate::app::types_overlay::OverlayRequest::ContextMenu(ContextMenu {
                anchor: ContextMenuAnchor::SelectedItem(crate::app::PanelFocus::Library),
                entries: vec![ContextMenuEntry {
                    label: "Play",
                    action: Some(ContextAction::Play),
                }],
                cursor: 0,
            }),
        );
        model.sync_modal_requests();
        let id = ComponentId::Overlay(OverlayId::ContextMenu);
        assert!(model.application.mounted(&id));

        let message = {
            let component = model
                .application
                .get_component_mut(&id)
                .expect("context menu mounted")
                .as_any_mut()
                .downcast_mut::<ContextMenuComponent>()
                .expect("context menu type");
            component.set_rect(Rect::new(10, 5, 10, 4));
            component.on(&Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 12,
                row: 6, // inner row 1 -> entry index 0
                modifiers: KeyModifiers::NONE,
            }))
        };
        let Some(Msg::Shell(request)) = message else {
            panic!("menu click must select the entry");
        };
        assert!(matches!(
            request,
            ShellRequest::ContextMenuSelect(0)
        ));

        model.handle_terminal_message(Msg::Shell(request), None, &mut false, &mut false);
        assert!(
            !model.application.mounted(&id),
            "executing a menu entry must close the menu"
        );
    }
}
