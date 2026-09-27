use super::*;
use crate::app::state::types::settings::ServiceEntry;
use crate::config::TestStateDirGuard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::config::EmbySetup;
use mbv_core::service_runtime::ServiceState;

#[test]
fn unavailable_emby_retry_is_one_bounded_generation() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().emby_setup = Some(EmbySetup::new("http://127.0.0.1:1", "user-id"));
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "token").unwrap();
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
fn auth_rejection_clears_player_even_when_secret_deletion_fails() {
    let mut app = tests::make_app_stub();
    app.player
        .update_emby_credentials("https://emby.example.test".into(), "rejected".into());
    let generation = app.emby_runtime.generation();
    // No content snapshot is delivered on the failure path, so the
    // Model-owned Home content (task 5.3d) is untouched.
    let content = app.apply_emby_completion_with_secret_deleter(
        crate::app::dispatch::session::service_startup::Completion {
            generation,
            result: Err(mbv_core::service_runtime::EmbyFailure {
                class: mbv_core::service_runtime::EmbyFailureClass::AuthenticationRejected,
                message: "HTTP 401".into(),
            }),
        },
        |_| Err("secret store unavailable".into()),
    );
    assert_eq!(app.emby_runtime.state, ServiceState::NeedsAuthentication);
    assert_eq!(app.player.emby_credentials(), None);
    assert!(app.status.contains("could not remove"));
    assert!(content.is_none());
}

#[test]
fn unavailable_failure_preserves_ready_runtime_player_secret_setup_generation_and_content() {
    let _guard = TestStateDirGuard::new();
    let config = crate::config::Config {
        emby_setup: Some(EmbySetup::new("https://emby.example", "user-id")),
        ..Default::default()
    };
    let mut app = tests::make_app_stub();
    *app.config.lock().unwrap() = config.clone();
    let mut client = mbv_core::api::EmbyClient::new(config.clone());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "https://emby.example".into(),
        user_id: "user-id".into(),
        token: "valid-token".into(),
    });
    let current = std::sync::Arc::new(std::sync::Mutex::new(client));
    app.emby_runtime =
        mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::clone(&current));
    app.player
        .update_emby_credentials("https://emby.example".into(), "valid-token".into());
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "valid-token")
        .unwrap();
    let generation = app.emby_runtime.generation();

    app.handle_emby_runtime_failure_with_secret_deleter(
        mbv_core::service_runtime::EmbyFailure::unavailable("HTTP 503"),
        |_| panic!("unavailable must not delete a secret"),
    );
    assert_eq!(app.emby_runtime.state, ServiceState::Unavailable);
    assert_eq!(app.emby_runtime.generation(), generation);
    assert!(std::sync::Arc::ptr_eq(
        app.emby_runtime.client.as_ref().unwrap(),
        &current
    ));
    assert_eq!(
        app.player.emby_credentials(),
        Some(("https://emby.example".into(), "valid-token".into()))
    );
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby),
        Some("valid-token".into())
    );
    assert_eq!(app.config.lock().unwrap().emby_setup, config.emby_setup);
}

#[test]
fn stale_auth_completion_cannot_delete_new_secret_or_change_ready_runtime() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    let config = crate::config::Config {
        emby_setup: Some(EmbySetup::new("https://emby.example", "user-id")),
        ..Default::default()
    };
    *app.config.lock().unwrap() = config;
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "https://emby.example".into(),
        user_id: "user-id".into(),
        token: "new-token".into(),
    });
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    app.player
        .update_emby_credentials("https://emby.example".into(), "new-token".into());
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "new-token")
        .unwrap();
    let stale = app.emby_runtime.generation();
    let newer = app.emby_runtime.begin_retry();
    app.emby_runtime.state = ServiceState::Ready;
    // No content snapshot is delivered on the stale/failure path, so the
    // Model-owned Home content (task 5.3d) is untouched.
    let content =
        app.apply_emby_completion(crate::app::dispatch::session::service_startup::Completion {
            generation: stale,
            result: Err(mbv_core::service_runtime::EmbyFailure {
                class: mbv_core::service_runtime::EmbyFailureClass::AuthenticationRejected,
                message: "HTTP 401".into(),
            }),
        });
    assert_eq!(app.emby_runtime.generation(), newer);
    assert_eq!(app.emby_runtime.state, ServiceState::Ready);
    assert_eq!(
        app.player.emby_credentials(),
        Some(("https://emby.example".into(), "new-token".into()))
    );
    assert!(content.is_none());
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby),
        Some("new-token".into())
    );
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
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "old-token")
        .unwrap();
    app.open_services_settings();
    app.activate_service_entry(ServiceEntry::Emby);
    let generation = app.emby_runtime.begin_setup();
    app.setup.emby_setup_form.as_mut().unwrap().generation = Some(generation);
    app.setup.emby_setup_form.as_mut().unwrap().busy = true;
    let mut candidate = mbv_core::api::EmbyClient::new(config);
    candidate.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
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
                bootstrap: mbv_core::service_runtime::EmbyBootstrap::default(),
                setup: EmbySetup::new("https://new.example/", "new-user"),
            }),
        },
    );
    assert!(content.is_none());
    assert!(app.setup.pending_emby_replacement.is_some());
    assert!(matches!(
        app.pending_overlay,
        Some(crate::app::state::types::overlay::OverlayRequest::Confirm(
            _
        ))
    ));
    assert_eq!(app.config.lock().unwrap().emby_setup, Some(old_setup));
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby),
        Some("old-token".into())
    );
    let action = match app.pending_overlay.as_ref() {
        Some(crate::app::state::types::overlay::OverlayRequest::Confirm(modal)) => {
            modal.on_confirm.clone()
        }
        _ => panic!("confirmation request missing"),
    };
    app.apply_confirm_action(action, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.dismiss_confirm();
    assert!(app.setup.pending_emby_replacement.is_none());
    assert_eq!(app.emby_runtime.state, ServiceState::Ready);
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby),
        Some("old-token".into())
    );
}
