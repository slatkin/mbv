use super::components::{ComponentId, Msg, ServiceRequest};
use super::types_overlay::OverlayRequest;
use super::{Model, SettingsDestination, SidebarId};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[test]
fn services_settings_raise_is_a_shell_request() {
    let mut app = super::tests::make_app_stub();
    app.open_services_settings();

    assert!(matches!(
        app.pending_overlay,
        Some(OverlayRequest::OpenSidebar(SidebarId::Settings))
    ));
    assert_eq!(app.settings_destination, SettingsDestination::Services);
}

#[test]
fn settings_component_service_key_crosses_the_shell_boundary() {
    let mut app = super::tests::make_app_stub();
    app.open_services_settings();
    let mut model = Model::new(app);
    model.sync_modal_requests();
    model.update_settings_content();

    let id = ComponentId::Overlay(super::components::OverlayId::Settings);
    let message = model
        .application
        .get_component_mut(&id)
        .expect("Settings component mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        message,
        Some(Msg::Service(ServiceRequest::SettingsKey { .. }))
    ));
}

#[test]
fn settings_component_dismissal_is_a_typed_shell_message() {
    let mut app = super::tests::make_app_stub();
    app.pending_overlay = Some(OverlayRequest::OpenSidebar(SidebarId::Settings));
    let mut model = Model::new(app);
    model.sync_modal_requests();
    model.update_settings_content();

    let id = ComponentId::Overlay(super::components::OverlayId::Settings);
    let message = model
        .application
        .get_component_mut(&id)
        .expect("Settings component mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Esc,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        message,
        Some(Msg::Persist(
            super::components::PersistRequest::SettingsKey { .. }
        ))
    ));
    let Some(Msg::Persist(request)) = message else {
        unreachable!();
    };
    assert!(!model.handle_persist_request(request));
    assert!(matches!(
        model.app.pending_overlay,
        Some(OverlayRequest::DismissSidebar(SidebarId::Settings))
    ));
}

#[test]
fn settings_shell_keeps_service_setup_effects_out_of_component() {
    let mut app = super::tests::make_app_stub();
    app.open_services_settings();
    app.activate_service_entry();
    let mut model = Model::new(app);

    model.handle_service_request(ServiceRequest::SubmitEmbySetup {
        server_url: String::new(),
        username: String::new(),
        password: String::new(),
    });
    assert!(model
        .app
        .emby_setup_form
        .as_ref()
        .is_some_and(|form| !form.error.is_empty()));
}
