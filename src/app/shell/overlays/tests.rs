use super::*;
use crate::app::components::{
    FeedsManageComponent, LibraryRoutesComponent, Msg, MultiselectComponent, ShellRequest,
    UserEvent,
};
use crate::app::state::types::context_menu::{LibraryRoutePopup, LibraryRouteStage};
use crate::app::state::types::context_menu::{MultiSelectKind, MultiSelectPopup};
use crate::app::tests::make_app_stub;
use tuirealm::component::AppComponent;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

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
/// keystroke time. The test expires that deadline by writing the
/// crate-visible field directly instead of sleeping out the 300ms
/// wall clock: no `Clock` seam in prod is needed, and the duration
/// itself is not under test — only that a past-due deadline dispatches
/// through the sweep.
#[test]
fn search_sidebar_debounce_dispatches_in_a_mounted_shell() {
    use crate::app::components::{SearchSidebarComponent, ServiceRequest};
    use std::time::Instant;

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
    assert!(matches!(
        dispatch(&mut model, &type_key('a')),
        Some(Msg::TerminalEvent(
            crate::app::components::msg::TerminalObserverEvent::KeyClaimed
        ))
    ));
    assert!(matches!(
        dispatch(&mut model, &type_key('b')),
        Some(Msg::TerminalEvent(
            crate::app::components::msg::TerminalObserverEvent::KeyClaimed
        ))
    ));

    // Sweep before the 300 ms deadline: should not fire.
    assert!(model.tick_search_clock(Instant::now()).is_none());

    // Expire the deadline directly (see doc comment) instead of sleeping.
    model
        .application
        .get_component_mut(&search_id)
        .expect("search sidebar mounted")
        .as_any_mut()
        .downcast_mut::<SearchSidebarComponent>()
        .expect("search sidebar type")
        .debounce_deadline = Some(Instant::now() - std::time::Duration::from_millis(1));

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
    use crate::app::state::types::context_menu::{
        ContextAction, ContextMenu, ContextMenuAnchor, ContextMenuEntry,
    };
    use ratatui::layout::Rect;
    use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};

    let mut model = Model::new(make_app_stub());
    model.app.pending_overlay = Some(
        crate::app::state::types::overlay::OverlayRequest::ContextMenu(ContextMenu {
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
    assert!(matches!(request, ShellRequest::ContextMenuSelect(0)));

    model.handle_terminal_message(Msg::Shell(request), &mut false, &mut false);
    assert!(
        !model.application.mounted(&id),
        "executing a menu entry must close the menu"
    );
}
