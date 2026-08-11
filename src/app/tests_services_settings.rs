use super::*;
use crate::config::TestStateDirGuard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::config::{EmbySetup, FeedKind, FeedSubscription};
use mbv_core::service_runtime::ServiceState;

fn enter() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
}

#[test]
fn services_destination_is_programmatically_addressable() {
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    assert!(app.show_settings);
    assert_eq!(app.settings_destination, SettingsDestination::Services);
    assert_eq!(app.services_cursor, 0);
}

#[test]
fn new_independent_empty_setup_opens_services_without_startup_worker() {
    let _guard = TestStateDirGuard::new();
    let app = App::new_independent(crate::config::Config::default());

    assert!(app.show_settings);
    assert_eq!(app.settings_destination, SettingsDestination::Services);
    assert!(app.emby_startup_request.is_none());
}

#[test]
fn validated_emby_setup_preserves_navigation_and_schedules_startup() {
    let _guard = TestStateDirGuard::new();
    let mut config = crate::config::Config::default();
    config.emby_setup = Some(EmbySetup::new("https://emby.example.test", "user-id"));
    let app = App::new_independent(config);

    assert!(!app.show_settings);
    assert!(app.emby_startup_request.is_some());
}

#[test]
fn legacy_server_url_without_setup_opens_services_without_startup_worker() {
    let _guard = TestStateDirGuard::new();
    let mut config = crate::config::Config::default();
    config.server_url = "https://legacy.example.test".into();
    let app = App::new_independent(config);

    assert!(app.show_settings);
    assert_eq!(app.settings_destination, SettingsDestination::Services);
    assert!(app.emby_startup_request.is_none());
}

#[test]
fn feed_only_setup_preserves_navigation_without_emby_startup() {
    let _guard = TestStateDirGuard::new();
    let mut config = crate::config::Config::default();
    config.feeds.push(FeedSubscription {
        name: "News".into(),
        url: "https://example.test/news".into(),
        kind: FeedKind::Audio,
    });
    let app = App::new_independent(config);

    assert!(!app.show_settings);
    assert!(app.emby_startup_request.is_none());
}

#[test]
fn services_has_exact_singleton_order_and_names() {
    assert_eq!(SERVICE_ENTRIES.len(), 3);
    let names: Vec<_> = SERVICE_ENTRIES
        .iter()
        .map(|entry| App::service_entry_name(*entry))
        .collect();
    assert_eq!(names, ["Emby", "Audiobookshelf", "Feeds"]);
}

#[test]
fn emby_state_labels_and_applicable_actions_are_truthful() {
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    for (state, label, action) in [
        (ServiceState::NotConfigured, "Not configured", "Set up Emby"),
        (ServiceState::Connecting, "Connecting", ""),
        (ServiceState::Ready, "Ready", "Repair / replace (d removes)"),
        (
            ServiceState::NeedsAuthentication,
            "Needs authentication",
            "Set up Emby",
        ),
        (ServiceState::Unavailable, "Unavailable", "Retry connection"),
    ] {
        app.emby_runtime.state = state;
        assert_eq!(app.service_state_label(ServiceEntry::Emby), label);
        assert_eq!(app.service_action_label(ServiceEntry::Emby), action);
    }
}

#[test]
fn feeds_is_always_present_and_enter_opens_existing_manager() {
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    app.services_cursor = 2;
    app.config.lock().unwrap().feeds = vec![mbv_core::config::FeedSubscription {
        name: "News".into(),
        url: "https://example.test/news".into(),
        kind: mbv_core::config::FeedKind::Audio,
    }];
    app.handle_key_services_settings(enter());
    assert!(app.feeds_manage_popup.is_some());
    assert_eq!(app.service_context(ServiceEntry::Feeds), "1 subscription");
}

#[test]
fn audiobookshelf_exposes_setup_action_when_not_configured() {
    let app = tests::make_app_stub();
    assert_eq!(
        app.service_state_label(ServiceEntry::Audiobookshelf),
        "Not configured"
    );
    assert_eq!(
        app.service_action_label(ServiceEntry::Audiobookshelf),
        "Set up Audiobookshelf"
    );
}

#[test]
fn services_cursor_bounds_and_escape_back() {
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    app.handle_key_services_settings(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.services_cursor, 0);
    app.handle_key_services_settings(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key_services_settings(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key_services_settings(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.services_cursor, 2);
    app.handle_key_services_settings(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.settings_destination, SettingsDestination::Main);
    assert!(app.show_settings);
    app.handle_key_settings(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.show_settings);
}

#[test]
fn emby_setup_opens_inside_services_and_escape_clears_password() {
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    app.activate_service_entry();
    assert!(app.emby_setup_form.is_some());
    let generation = app.emby_runtime.begin_setup();
    let form = app.emby_setup_form.as_mut().unwrap();
    form.generation = Some(generation);
    form.previous_state = ServiceState::NotConfigured;
    form.busy = true;
    app.emby_setup_form.as_mut().unwrap().fields[2] = "secret".into();
    app.handle_key_services_settings(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.emby_setup_form.is_none());
    assert_eq!(app.settings_destination, SettingsDestination::Services);
    assert_eq!(app.emby_runtime.state, ServiceState::NotConfigured);
    assert_ne!(app.emby_runtime.generation(), generation);
}

#[test]
fn busy_emby_setup_ignores_duplicate_submit() {
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    app.activate_service_entry();
    let generation = app.emby_runtime.begin_setup();
    let form = app.emby_setup_form.as_mut().unwrap();
    form.busy = true;
    form.fields = ["http://server".into(), "alice".into(), String::new()];
    form.focus = 2;
    form.generation = Some(generation);
    form.previous_state = ServiceState::NotConfigured;
    let fields = form.fields.clone();
    let focus = form.focus;
    let generation = app.emby_runtime.generation();
    app.handle_key_services_settings(enter());
    assert!(app.emby_setup_rx.is_none());
    assert!(app.emby_setup_form.as_ref().unwrap().busy);
    assert_eq!(app.emby_setup_form.as_ref().unwrap().fields, fields);
    assert_eq!(app.emby_setup_form.as_ref().unwrap().focus, focus);
    assert_eq!(app.emby_runtime.generation(), generation);
    assert_eq!(app.emby_runtime.state, ServiceState::Connecting);
    assert!(app.emby_setup_form.as_ref().unwrap().fields[2].is_empty());
}

#[test]
fn busy_emby_setup_is_read_only_except_escape() {
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    app.activate_service_entry();
    let form = app.emby_setup_form.as_mut().unwrap();
    form.fields = ["http://server".into(), "alice".into(), "password".into()];
    form.focus = 1;
    form.busy = true;
    let before = form.fields.clone();
    for key in [KeyCode::Tab, KeyCode::Up, KeyCode::Down, KeyCode::Backspace] {
        app.handle_key_services_settings(KeyEvent::new(key, KeyModifiers::NONE));
    }
    assert_eq!(app.emby_setup_form.as_ref().unwrap().fields, before);
    assert_eq!(app.emby_setup_form.as_ref().unwrap().focus, 1);
    assert!(app.emby_setup_form.is_some());
}

#[test]
fn setup_worker_disconnect_restores_state_and_keeps_safe_form_open() {
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    app.activate_service_entry();
    let generation = app.emby_runtime.begin_setup();
    let form = app.emby_setup_form.as_mut().unwrap();
    form.generation = Some(generation);
    form.previous_state = ServiceState::NotConfigured;
    form.busy = true;
    form.fields[2] = "password".into();
    app.handle_emby_setup_worker_disconnect();
    assert_eq!(app.emby_runtime.state, ServiceState::NotConfigured);
    assert_ne!(app.emby_runtime.generation(), generation);
    let form = app.emby_setup_form.as_ref().unwrap();
    assert!(!form.busy);
    assert!(form.fields[2].is_empty());
    assert!(form.error.contains("retry"));
}

#[test]
fn setup_identity_accepts_same_normalized_identity_and_defers_replacement() {
    let existing = EmbySetup::new("https://emby.example/", "user-id");
    let same = EmbySetup::new("https://emby.example", "user-id");
    let replacement = EmbySetup::new("https://other.example", "user-id");
    assert!(super::service_startup::setup_identity_allows_commit(
        Some(&existing),
        &same
    ));
    assert!(!super::service_startup::setup_identity_allows_commit(
        Some(&existing),
        &replacement
    ));
}

#[test]
fn setup_completion_success_commits_ready_runtime_and_local_player_without_network() {
    let _guard = TestStateDirGuard::new();
    std::fs::write(
        mbv_core::config::config_path(),
        "[server]\nurl = \"old\"\nusername = \"alice\"\npassword = \"old-secret\"\napi_key = \"old-key\"\n[general]\nkeep = true\n",
    )
    .unwrap();
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    app.activate_service_entry();
    let generation = app.emby_runtime.begin_setup();
    let form = app.emby_setup_form.as_mut().unwrap();
    form.generation = Some(generation);
    form.previous_state = ServiceState::NotConfigured;
    form.busy = true;
    form.fields[2] = "password".into();
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "https://emby.example".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    app.apply_emby_setup_completion_without_network(super::service_startup::SetupCompletion {
        generation,
        previous_state: ServiceState::NotConfigured,
        result: Ok(super::service_startup::Startup {
            client,
            bootstrap: mbv_core::service_runtime::EmbyBootstrap::default(),
            setup: EmbySetup::new("https://emby.example/", "user-id"),
        }),
    });
    assert_eq!(app.emby_runtime.state, ServiceState::Ready);
    assert!(app.emby_runtime.client.is_some());
    assert!(app.emby_setup_form.is_none());
    assert_eq!(
        app.player.emby_credentials(),
        Some(("https://emby.example".into(), "token".into()))
    );
    let config = std::fs::read_to_string(mbv_core::config::config_path()).unwrap();
    assert!(!config.contains("username"));
    assert!(!config.contains("password"));
    assert!(!config.contains("api_key"));
    assert!(config.contains("keep = true"));
    assert!(config.contains("user_id = \"user-id\""));
}

#[test]
fn setup_persistence_failure_preserves_previous_active_client() {
    let _guard = TestStateDirGuard::new();
    std::fs::write(mbv_core::config::config_path(), "not = [valid").unwrap();
    let mut app = tests::make_app_stub();
    let prior = std::sync::Arc::new(std::sync::Mutex::new(mbv_core::api::EmbyClient::new(
        crate::config::Config::default(),
    )));
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(prior.clone());
    app.emby_runtime.state = ServiceState::NeedsAuthentication;
    app.open_services_settings();
    app.activate_service_entry();
    let generation = app.emby_runtime.begin_setup();
    let form = app.emby_setup_form.as_mut().unwrap();
    form.generation = Some(generation);
    form.previous_state = ServiceState::NeedsAuthentication;
    form.busy = true;
    form.fields[2] = "password".into();
    app.apply_emby_setup_completion_without_network(super::service_startup::SetupCompletion {
        generation,
        previous_state: ServiceState::NeedsAuthentication,
        result: Ok(super::service_startup::Startup {
            client: mbv_core::api::EmbyClient::new(crate::config::Config::default()),
            bootstrap: mbv_core::service_runtime::EmbyBootstrap::default(),
            setup: EmbySetup::new("https://emby.example", "user-id"),
        }),
    });
    assert_eq!(app.emby_runtime.state, ServiceState::NeedsAuthentication);
    assert!(std::sync::Arc::ptr_eq(
        app.emby_runtime.client.as_ref().unwrap(),
        &prior
    ));
    assert!(app.emby_setup_form.is_some());
    assert!(app.emby_setup_form.as_ref().unwrap().fields[2].is_empty());
}

#[test]
fn setup_completion_persistence_failure_retains_runtime_and_open_form() {
    let _guard = TestStateDirGuard::new();
    std::fs::write(mbv_core::config::config_path(), "not = [valid").unwrap();
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    app.activate_service_entry();
    let generation = app.emby_runtime.begin_setup();
    let form = app.emby_setup_form.as_mut().unwrap();
    form.generation = Some(generation);
    form.previous_state = ServiceState::NotConfigured;
    form.busy = true;
    form.fields[2] = "password".into();
    app.apply_emby_setup_completion_without_network(super::service_startup::SetupCompletion {
        generation,
        previous_state: ServiceState::NotConfigured,
        result: Ok(super::service_startup::Startup {
            client: mbv_core::api::EmbyClient::new(crate::config::Config::default()),
            bootstrap: mbv_core::service_runtime::EmbyBootstrap::default(),
            setup: EmbySetup::new("https://emby.example", "user-id"),
        }),
    });
    assert_eq!(app.emby_runtime.state, ServiceState::NotConfigured);
    assert!(app.emby_runtime.client.is_none());
    let form = app.emby_setup_form.as_ref().unwrap();
    assert!(!form.busy);
    assert!(form.fields[2].is_empty());
    assert!(!form.error.is_empty());
}

#[test]
fn stale_setup_completion_cannot_persist_or_overwrite_runtime() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.open_services_settings();
    app.activate_service_entry();
    let generation = app.emby_runtime.begin_setup();
    app.emby_runtime
        .cancel_setup(generation, ServiceState::NotConfigured);
    app.home.continue_items = vec![tests::make_item("current", "Audio")];
    app.apply_emby_setup_completion_without_network(super::service_startup::SetupCompletion {
        generation,
        previous_state: ServiceState::NotConfigured,
        result: Ok(super::service_startup::Startup {
            client: mbv_core::api::EmbyClient::new(crate::config::Config::default()),
            bootstrap: mbv_core::service_runtime::EmbyBootstrap::default(),
            setup: EmbySetup::new("https://emby.example", "user-id"),
        }),
    });
    assert!(!mbv_core::config::config_path().exists());
    assert_eq!(app.home.continue_items[0].name, "current");
    assert_eq!(app.emby_runtime.state, ServiceState::NotConfigured);
}

#[test]
fn general_and_feed_navigation_do_not_depend_on_an_emby_client() {
    let mut app = tests::make_app_stub();
    let stay_alive = (0..settings::settings_total_rows())
        .find(|&idx| settings::settings_cursor_to_key(idx) == SettingKey::StayAlive)
        .expect("StayAlive setting row must exist");

    for state in [
        ServiceState::NotConfigured,
        ServiceState::Connecting,
        ServiceState::Ready,
        ServiceState::NeedsAuthentication,
        ServiceState::Unavailable,
    ] {
        app.emby_runtime.state = state;
        app.emby_runtime.client = None;
        app.settings_cursor = stay_alive;
        let before = app.config.lock().unwrap().stay_alive;
        app.handle_settings_activate();
        assert_ne!(app.config.lock().unwrap().stay_alive, before);

        app.open_services_settings();
        app.services_cursor = 2;
        app.handle_key_services_settings(enter());
        assert!(app.feeds_manage_popup.is_some());
        app.feeds_manage_popup = None;
    }
}

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
