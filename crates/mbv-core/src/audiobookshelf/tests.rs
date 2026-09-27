use super::*;
use std::collections::HashMap;

mod catalog;
mod playback;

#[test]
fn invalid_setup_candidate_does_not_change_persisted_setup() {
    let _guard = crate::config::TestStateDirGuard::new();
    crate::config::persist_audiobookshelf_setup_and_secret(
        &crate::config::AudiobookshelfSetup::new("https://working.example"),
        "working-secret",
    )
    .unwrap();
    let config_before = std::fs::read(crate::config::config_path()).unwrap();
    let secret_before =
        crate::config::load_service_secret(crate::config::ServiceKind::Audiobookshelf);

    AudiobookshelfClient::validate_setup_bounded(
        "",
        "candidate-secret",
        std::time::Duration::from_millis(1),
    )
    .unwrap_err();

    assert_eq!(
        std::fs::read(crate::config::config_path()).unwrap(),
        config_before
    );
    assert_eq!(
        crate::config::load_service_secret(crate::config::ServiceKind::Audiobookshelf),
        secret_before
    );
}

#[test]
fn validated_setup_debug_redacts_api_key() {
    let setup = AudiobookshelfValidatedSetup::new(
        crate::config::AudiobookshelfSetup::new("http://abs:13378"),
        AudiobookshelfUser {
            id: "user-id".to_string(),
            username: "user".to_string(),
        },
        "abs-secret-key".to_string(),
    );
    let rendered = format!("{setup:?}");
    assert!(rendered.contains("AudiobookshelfValidatedSetup"));
    assert!(!rendered.contains("abs-secret-key"));
}
