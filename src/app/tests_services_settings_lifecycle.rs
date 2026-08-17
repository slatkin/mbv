use super::*;
use crate::config::TestStateDirGuard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::config::{EmbySetup, FeedKind, FeedSubscription};
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
    app.activate_service_entry();
    assert_eq!(app.emby_runtime.state, ServiceState::Connecting);
    assert_ne!(app.emby_runtime.generation(), generation);
    let retry_generation = app.emby_runtime.generation();
    app.activate_service_entry();
    assert_eq!(app.emby_runtime.generation(), retry_generation);
    assert!(app.emby_startup_rx.is_some());
}

#[test]
fn unavailable_emby_without_secret_offers_setup_instead_of_placeholder_auth() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().emby_setup =
        Some(EmbySetup::new("https://emby.example.test", "user-id"));
    app.emby_runtime.state = ServiceState::Unavailable;
    app.open_services_settings();
    app.activate_service_entry();
    assert_eq!(app.emby_runtime.state, ServiceState::NeedsAuthentication);
    assert!(app.emby_setup_form.is_some());
    assert!(app.emby_startup_rx.is_none());
}

#[test]
fn auth_rejection_clears_player_even_when_secret_deletion_fails() {
    let mut app = tests::make_app_stub();
    app.player
        .update_emby_credentials("https://emby.example.test".into(), "rejected".into());
    let generation = app.emby_runtime.generation();
    app.apply_emby_completion_with_secret_deleter(
        super::service_startup::Completion {
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
}

#[test]
fn auth_rejection_isolated_cleanup_preserves_setup_owned_content_and_other_secrets() {
    let _guard = TestStateDirGuard::new();
    let mut config = crate::config::Config::default();
    config.emby_setup = Some(EmbySetup::new("https://emby.example", "user-id"));
    config.feeds.push(FeedSubscription {
        name: "News".into(),
        url: "https://feed.example/rss".into(),
        kind: FeedKind::Audio,
    });
    let mut app = tests::make_app_stub();
    *app.config.lock().unwrap() = config.clone();
    let mut client = mbv_core::api::EmbyClient::new(config.clone());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "https://emby.example".into(),
        user_id: "user-id".into(),
        token: "rejected-token".into(),
    });
    let current = std::sync::Arc::new(std::sync::Mutex::new(client));
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(current);
    app.home.continue_items = vec![tests::make_item("owned", "Movie")];
    app.player
        .update_emby_credentials("https://emby.example".into(), "rejected-token".into());
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "rejected-token")
        .unwrap();
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "audiobookshelf-secret",
    )
    .unwrap();
    mbv_core::config::save_control_credential("control-secret").unwrap();

    let generation = app.emby_runtime.generation();
    app.apply_emby_completion(super::service_startup::Completion {
        generation,
        result: Err(mbv_core::service_runtime::EmbyFailure {
            class: mbv_core::service_runtime::EmbyFailureClass::AuthenticationRejected,
            message: "HTTP 403".into(),
        }),
    });
    assert_eq!(app.emby_runtime.state, ServiceState::NeedsAuthentication);
    assert_eq!(app.config.lock().unwrap().emby_setup, config.emby_setup);
    assert_eq!(app.home.continue_items[0].name, "owned");
    assert!(app.emby_runtime.client.is_none());
    assert_eq!(app.player.emby_credentials(), None);
    assert!(mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby).is_none());
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf),
        Some("audiobookshelf-secret".into())
    );
    assert_eq!(
        mbv_core::config::load_control_credential(),
        Some("control-secret".into())
    );
    assert!(!app.config.lock().unwrap().feeds.is_empty());
}

#[test]
fn unavailable_failure_preserves_ready_runtime_player_secret_setup_generation_and_content() {
    let _guard = TestStateDirGuard::new();
    let mut config = crate::config::Config::default();
    config.emby_setup = Some(EmbySetup::new("https://emby.example", "user-id"));
    let mut app = tests::make_app_stub();
    *app.config.lock().unwrap() = config.clone();
    let mut client = mbv_core::api::EmbyClient::new(config.clone());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "https://emby.example".into(),
        user_id: "user-id".into(),
        token: "valid-token".into(),
    });
    let current = std::sync::Arc::new(std::sync::Mutex::new(client));
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(current.clone());
    app.home.continue_items = vec![tests::make_item("owned", "Movie")];
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
    assert_eq!(app.home.continue_items[0].name, "owned");
}

#[test]
fn startup_worker_disconnect_is_generation_aware_and_preserves_secret() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().emby_setup = Some(EmbySetup::new("https://emby.example", "user-id"));
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "valid-token")
        .unwrap();
    app.emby_runtime.state = ServiceState::Connecting;
    let generation = app.emby_runtime.generation();
    app.handle_emby_startup_worker_disconnect(generation);
    assert_eq!(app.emby_runtime.state, ServiceState::Unavailable);
    assert_eq!(app.emby_runtime.generation(), generation);
    assert!(mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby).is_some());

    let newer = app.emby_runtime.begin_retry();
    app.emby_runtime.state = ServiceState::Connecting;
    app.handle_emby_startup_worker_disconnect(generation);
    assert_eq!(app.emby_runtime.state, ServiceState::Connecting);
    assert_eq!(app.emby_runtime.generation(), newer);
}

#[test]
fn startup_worker_disconnect_without_secret_requires_authentication() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().emby_setup = Some(EmbySetup::new("https://emby.example", "user-id"));
    app.emby_runtime.state = ServiceState::Connecting;
    let generation = app.emby_runtime.generation();
    app.handle_emby_startup_worker_disconnect(generation);
    assert_eq!(app.emby_runtime.state, ServiceState::NeedsAuthentication);
    assert_eq!(app.emby_runtime.generation(), generation);
}

#[test]
fn retry_failure_completion_preserves_existing_runtime_and_advances_generation() {
    let _guard = TestStateDirGuard::new();
    let mut config = crate::config::Config::default();
    config.emby_setup = Some(EmbySetup::new("https://emby.example", "user-id"));
    let mut app = tests::make_app_stub();
    *app.config.lock().unwrap() = config.clone();
    let mut client = mbv_core::api::EmbyClient::new(config.clone());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "https://emby.example".into(),
        user_id: "user-id".into(),
        token: "valid-token".into(),
    });
    let current = std::sync::Arc::new(std::sync::Mutex::new(client));
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(current.clone());
    app.emby_runtime.state = ServiceState::Unavailable;
    app.home.continue_items = vec![tests::make_item("owned", "Movie")];
    app.player
        .update_emby_credentials("https://emby.example".into(), "valid-token".into());
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "valid-token")
        .unwrap();
    app.open_services_settings();
    let old_generation = app.emby_runtime.generation();
    app.activate_service_entry();
    let generation = app.emby_runtime.generation();
    assert_ne!(generation, old_generation);
    app.emby_startup_rx = None;
    app.apply_emby_completion(super::service_startup::Completion {
        generation,
        result: Err(mbv_core::service_runtime::EmbyFailure::unavailable(
            "connection refused",
        )),
    });
    assert_eq!(app.emby_runtime.state, ServiceState::Unavailable);
    assert!(std::sync::Arc::ptr_eq(
        app.emby_runtime.client.as_ref().unwrap(),
        &current
    ));
    assert_eq!(
        app.player.emby_credentials(),
        Some(("https://emby.example".into(), "valid-token".into()))
    );
    assert_eq!(app.home.continue_items[0].name, "owned");
}

#[test]
fn stale_auth_completion_cannot_delete_new_secret_or_change_ready_runtime() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    let mut config = crate::config::Config::default();
    config.emby_setup = Some(EmbySetup::new("https://emby.example", "user-id"));
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
    app.home.continue_items = vec![tests::make_item("current", "Movie")];
    app.player
        .update_emby_credentials("https://emby.example".into(), "new-token".into());
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "new-token")
        .unwrap();
    let stale = app.emby_runtime.generation();
    let newer = app.emby_runtime.begin_retry();
    app.emby_runtime.state = ServiceState::Ready;
    app.apply_emby_completion(super::service_startup::Completion {
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
    assert_eq!(app.home.continue_items[0].name, "current");
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby),
        Some("new-token".into())
    );
}

#[test]
fn transient_setup_rejection_preserves_persisted_secret_setup_and_content() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    let setup = EmbySetup::new("https://emby.example", "user-id");
    app.config.lock().unwrap().emby_setup = Some(setup.clone());
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "old-token")
        .unwrap();
    app.home.continue_items = vec![tests::make_item("owned", "Movie")];
    app.emby_runtime.state = ServiceState::NeedsAuthentication;
    app.open_services_settings();
    app.activate_service_entry();
    let generation = app.emby_runtime.begin_setup();
    app.emby_setup_form.as_mut().unwrap().generation = Some(generation);
    app.emby_setup_form.as_mut().unwrap().busy = true;
    app.apply_emby_setup_completion_without_network(super::service_startup::SetupCompletion {
        generation,
        previous_state: ServiceState::NeedsAuthentication,
        result: Err("candidate credential rejected".into()),
    });
    assert_eq!(app.emby_runtime.state, ServiceState::NeedsAuthentication);
    assert_eq!(app.config.lock().unwrap().emby_setup, Some(setup));
    assert_eq!(app.home.continue_items[0].name, "owned");
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby),
        Some("old-token".into())
    );
}

#[test]
fn ready_emby_repair_opens_the_transactional_setup_form() {
    let mut app = tests::make_app_stub();
    app.emby_runtime.state = ServiceState::Ready;
    app.open_services_settings();
    app.activate_service_entry();
    assert!(app.emby_setup_form.is_some());
    assert!(app.confirm_modal.is_none());
}

#[test]
fn replacement_candidate_is_not_persisted_and_escape_drops_it() {
    let _guard = TestStateDirGuard::new();
    let old_setup = EmbySetup::new("https://old.example", "old-user");
    let mut config = crate::config::Config::default();
    config.emby_setup = Some(old_setup.clone());
    let mut app = tests::make_app_stub();
    *app.config.lock().unwrap() = config.clone();
    app.emby_runtime.state = ServiceState::Ready;
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "old-token")
        .unwrap();
    app.open_services_settings();
    app.activate_service_entry();
    let generation = app.emby_runtime.begin_setup();
    app.emby_setup_form.as_mut().unwrap().generation = Some(generation);
    app.emby_setup_form.as_mut().unwrap().busy = true;
    let mut candidate = mbv_core::api::EmbyClient::new(config);
    candidate.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "https://new.example".into(),
        user_id: "new-user".into(),
        token: "new-token".into(),
    });
    app.apply_emby_setup_completion_without_network(super::service_startup::SetupCompletion {
        generation,
        previous_state: ServiceState::Ready,
        result: Ok(super::service_startup::Startup {
            client: candidate,
            bootstrap: Default::default(),
            setup: EmbySetup::new("https://new.example/", "new-user"),
        }),
    });
    assert!(app.pending_emby_replacement.is_some());
    assert!(app.confirm_modal.is_some());
    assert_eq!(app.config.lock().unwrap().emby_setup, Some(old_setup));
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby),
        Some("old-token".into())
    );
    app.handle_key_confirm_modal(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.pending_emby_replacement.is_none());
    assert_eq!(app.emby_runtime.state, ServiceState::Ready);
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby),
        Some("old-token".into())
    );
}

#[test]
fn emby_removal_cancel_is_non_destructive() {
    let _guard = TestStateDirGuard::new();
    let setup = EmbySetup::new("https://old.example", "old-user");
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().emby_setup = Some(setup.clone());
    app.emby_runtime.state = ServiceState::Ready;
    mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "old-token")
        .unwrap();
    app.open_services_settings();
    app.request_emby_removal();
    app.handle_key_confirm_modal(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.config.lock().unwrap().emby_setup, Some(setup));
    assert_eq!(app.emby_runtime.state, ServiceState::Ready);
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby),
        Some("old-token".into())
    );
}
