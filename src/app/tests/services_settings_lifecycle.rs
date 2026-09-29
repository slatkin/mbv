use super::*;
use crate::config::TestStateDirGuard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_config::EmbySetup;
use mbv_core::service_runtime::ServiceState;
use mbv_ui_model::settings::ServiceEntry;

#[test]
fn unavailable_emby_retry_is_one_bounded_generation() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().emby_setup = Some(EmbySetup::new("http://127.0.0.1:1", "user-id"));
    mbv_config::save_service_secret(mbv_queue::ServiceKind::Emby, "token").unwrap();
    app.emby_runtime.state = ServiceState::Unavailable;
    app.open_services_settings();
    let generation = app.emby_runtime.generation();
    app.activate_service_entry(ServiceEntry::Emby);
    assert_eq!(app.emby_runtime.state, ServiceState::Connecting);
    assert_ne!(app.emby_runtime.generation(), generation);
    let retry_generation = app.emby_runtime.generation();
    app.activate_service_entry(ServiceEntry::Emby);
    assert_eq!(app.emby_runtime.generation(), retry_generation);
    assert!(app.setup.emby_startup_rx.is_some());
}

#[test]
fn unavailable_emby_without_secret_offers_setup_instead_of_placeholder_auth() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().emby_setup =
        Some(EmbySetup::new("https://emby.example.test", "user-id"));
    app.emby_runtime.state = ServiceState::Unavailable;
    app.open_services_settings();
    app.activate_service_entry(ServiceEntry::Emby);
    assert_eq!(app.emby_runtime.state, ServiceState::NeedsAuthentication);
    assert!(app.setup.emby_setup_form.is_some());
    assert!(app.setup.emby_startup_rx.is_none());
}

#[test]
fn replacement_candidate_is_not_persisted_and_escape_drops_it() {
    let _guard = TestStateDirGuard::new();
    let old_setup = EmbySetup::new("https://old.example", "old-user");
    let config = crate::config::Config {
        emby_setup: Some(old_setup.clone()),
        ..Default::default()
    };
    let mut app = tests::make_app_stub();
    *app.config.lock().unwrap() = config.clone();
    app.emby_runtime.state = ServiceState::Ready;
    mbv_config::save_service_secret(mbv_queue::ServiceKind::Emby, "old-token").unwrap();
    app.open_services_settings();
    app.activate_service_entry(ServiceEntry::Emby);
    let generation = app.emby_runtime.begin_setup();
    app.setup.emby_setup_form.as_mut().unwrap().generation = Some(generation);
    app.setup.emby_setup_form.as_mut().unwrap().busy = true;
    let mut candidate = mbv_emby::EmbyClient::new(config);
    candidate.apply_credential_exchange(&mbv_emby::EmbyCredentialExchange {
        server_url: "https://new.example".into(),
        user_id: "new-user".into(),
        token: "new-token".into(),
    });
    // A rejected identity lands in `pending_emby_replacement` (no Home
    // content snapshot is computed on this early-return path, task 5.3d).
    let content = app.apply_emby_setup_completion_without_network(
        crate::app::dispatch::session::service_startup::SetupCompletion {
            generation,
            previous_state: ServiceState::Ready,
            result: Ok(crate::app::dispatch::session::service_startup::Startup {
                client: candidate,
                bootstrap: mbv_emby::EmbyBootstrap::default(),
                setup: EmbySetup::new("https://new.example/", "new-user"),
            }),
        },
    );
    assert!(content.is_none());
    assert!(app.setup.pending_emby_replacement.is_some());
    assert!(matches!(
        app.pending_overlay,
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(_))
    ));
    assert_eq!(app.config.lock().unwrap().emby_setup, Some(old_setup));
    assert_eq!(
        mbv_config::load_service_secret(mbv_queue::ServiceKind::Emby),
        Some("old-token".into())
    );
    let action = match app.pending_overlay.as_ref() {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => modal.on_confirm.clone(),
        _ => panic!("confirmation request missing"),
    };
    app.apply_confirm_action(action, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.dismiss_confirm();
    assert!(app.setup.pending_emby_replacement.is_none());
    assert_eq!(app.emby_runtime.state, ServiceState::Ready);
    assert_eq!(
        mbv_config::load_service_secret(mbv_queue::ServiceKind::Emby),
        Some("old-token".into())
    );
}
