use super::*;
use crate::config::TestStateDirGuard;
use mbv_core::config::AudiobookshelfSetup;
use mbv_core::service_runtime::ServiceState;

fn user() -> mbv_core::audiobookshelf::AudiobookshelfUser {
    mbv_core::audiobookshelf::AudiobookshelfUser {
        id: "user-id".into(),
        username: "reader".into(),
    }
}

fn completion(
    generation: mbv_core::service_runtime::SetupGeneration,
    kind: super::service_startup::AudiobookshelfCompletionKind,
    result: Result<
        mbv_core::audiobookshelf::AudiobookshelfUser,
        mbv_core::audiobookshelf::AudiobookshelfError,
    >,
) -> super::service_startup::AudiobookshelfCompletion {
    super::service_startup::AudiobookshelfCompletion {
        generation,
        kind,
        result,
    }
}

#[test]
fn configured_startup_is_independent_and_reaches_ready() {
    let _guard = TestStateDirGuard::new();
    let mut config = crate::config::Config::default();
    config.audiobookshelf_setup = Some(AudiobookshelfSetup::new("https://books.example"));
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "book-secret",
    )
    .unwrap();
    let mut app = App::new_independent(config);
    assert!(app.audiobookshelf_startup_request.is_some());
    let generation = app.audiobookshelf_runtime.generation();
    app.apply_audiobookshelf_completion(completion(
        generation,
        super::service_startup::AudiobookshelfCompletionKind::Startup,
        Ok(user()),
    ));
    assert_eq!(app.audiobookshelf_runtime.state, ServiceState::Ready);
    assert_eq!(app.audiobookshelf_runtime.user.unwrap().username, "reader");
}

#[test]
fn rejected_key_clears_only_secret_and_unavailable_retains_it() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(AudiobookshelfSetup::new("https://books.example"));
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "book-secret",
    )
    .unwrap();
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.apply_audiobookshelf_completion(completion(
        generation,
        super::service_startup::AudiobookshelfCompletionKind::Startup,
        Err(mbv_core::audiobookshelf::AudiobookshelfError {
            class: mbv_core::audiobookshelf::AudiobookshelfFailureClass::AuthenticationRejected,
        }),
    ));
    assert_eq!(
        app.audiobookshelf_runtime.state,
        ServiceState::NeedsAuthentication
    );
    assert!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
            .is_none()
    );
    assert!(app.config.lock().unwrap().audiobookshelf_setup.is_some());

    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "book-secret",
    )
    .unwrap();
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.apply_audiobookshelf_completion(completion(
        generation,
        super::service_startup::AudiobookshelfCompletionKind::Startup,
        Err(mbv_core::audiobookshelf::AudiobookshelfError {
            class: mbv_core::audiobookshelf::AudiobookshelfFailureClass::Connectivity,
        }),
    ));
    assert_eq!(app.audiobookshelf_runtime.state, ServiceState::Unavailable);
    assert_eq!(
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
            .as_deref(),
        Some("book-secret")
    );
}

#[test]
fn stale_completion_after_removal_cannot_mutate_runtime() {
    let mut app = tests::make_app_stub();
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.audiobookshelf_runtime.remove_setup();
    app.apply_audiobookshelf_completion(completion(
        generation,
        super::service_startup::AudiobookshelfCompletionKind::Test,
        Ok(user()),
    ));
    assert_eq!(
        app.audiobookshelf_runtime.state,
        ServiceState::NotConfigured
    );
    assert!(app.audiobookshelf_runtime.user.is_none());
}

#[test]
fn test_success_reports_server_and_user_without_secret_details() {
    let _guard = TestStateDirGuard::new();
    let mut app = tests::make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(AudiobookshelfSetup::new("https://books.example"));
    let generation = app.audiobookshelf_runtime.begin_validation();
    app.apply_audiobookshelf_completion(completion(
        generation,
        super::service_startup::AudiobookshelfCompletionKind::Test,
        Ok(user()),
    ));
    assert_eq!(app.audiobookshelf_runtime.state, ServiceState::Ready);
    assert!(app.status.contains("https://books.example"));
    assert!(app.status.contains("reader"));
    assert!(!app.status.contains("Authorization"));
    assert!(!app.status.contains("book-secret"));
}
