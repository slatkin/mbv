#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{
        Msg, MultiselectComponent, PlaybackPromptComponent, SelectionModalComponent,
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
        model.app.pending_overlay = Some(crate::app::types_overlay::OverlayRequest::SelectionModal(
            SelectionModal {
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
            },
        ));
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
    fn settings_popup_multiselect_shell_syncs_and_commits_component_choices() {
        let mut model = Model::new(make_app_stub());
        model.app.multiselect_popup = Some(MultiSelectPopup {
            kind: MultiSelectKind::HiddenLibraries,
            items: vec![("movies".into(), "Movies".into(), false)],
            cursor: 0,
        });
        model.sync_multiselect();

        let id = ComponentId::Popup(PopupId::Multiselect);
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
        let Some(Msg::Shell(request)) = message else {
            panic!("Multiselect should emit a shell request");
        };
        model.handle_multiselect_commit();
        assert!(matches!(request, ShellRequest::MultiselectCommit { .. }));
        model.sync_multiselect();
        assert!(model.app.multiselect_popup.is_none());
        assert!(!model.application.mounted(&id));
    }

    #[test]
    fn settings_popup_library_routes_shell_syncs_and_routes_escape() {
        let mut model = Model::new(make_app_stub());
        model.app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary {
                items: vec![("movies".into(), "Movies".into(), None)],
            },
            cursor: 0,
        });
        model.sync_library_routes();

        let id = ComponentId::Popup(PopupId::LibraryRoutes);
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
        let Some(Msg::Shell(request)) = message else {
            panic!("Library routes should emit a shell request");
        };
        model.handle_library_routes_request(request);
        model.sync_library_routes();

        assert!(model.app.library_routes_popup.is_none());
        assert!(!model.application.mounted(&id));
    }

    #[test]
    fn settings_popup_feeds_manage_shell_syncs_and_routes_escape() {
        let mut model = Model::new(make_app_stub());
        model.app.feeds_manage_popup = Some(FeedsManagePopup::new());
        model.sync_feeds_manage();

        let id = ComponentId::Popup(PopupId::FeedManage);
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
        model.sync_feeds_manage();

        assert!(model.app.feeds_manage_popup.is_none());
        assert!(!model.application.mounted(&id));
    }
}
