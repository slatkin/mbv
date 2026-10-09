mod audiobookshelf {
    use mbv_audiobookshelf::*;
    use rstest::rstest;
    use std::time::Duration;

    /// Issue #894 item 3: the constructor takes `impl AsRef<str>`, so an owned
    /// `String` validates exactly like a borrow.
    #[rstest]
    #[case::owned_valid_url("http://127.0.0.1:1/".to_string(), true)]
    #[case::owned_blank_url("   ".to_string(), false)]
    fn new_accepts_owned_server_urls(#[case] server_url: String, #[case] valid: bool) {
        assert_eq!(AudiobookshelfClient::new(server_url).is_ok(), valid);
    }

    #[test]
    fn invalid_setup_candidate_does_not_change_persisted_setup() {
        let _guard = mbv_config::TestStateDirGuard::new();
        mbv_config::persist_audiobookshelf_setup_and_secret(
            &mbv_config::AudiobookshelfSetup::new("https://working.example"),
            "working-secret",
        )
        .unwrap();
        let config_before = std::fs::read(mbv_config::config_path()).unwrap();
        let secret_before = mbv_config::load_service_secret(mbv_queue::ServiceKind::Audiobookshelf);

        AudiobookshelfClient::validate_setup_bounded(
            "",
            "candidate-secret",
            std::time::Duration::from_millis(1),
        )
        .unwrap_err();

        assert_eq!(
            std::fs::read(mbv_config::config_path()).unwrap(),
            config_before
        );
        assert_eq!(
            mbv_config::load_service_secret(mbv_queue::ServiceKind::Audiobookshelf),
            secret_before
        );
    }

    #[test]
    fn configuration_failure_keeps_a_non_connectivity_kind_and_original_message() {
        let source = mbv_config::ConfigError::from(std::io::Error::other("save failed"));
        let error = AudiobookshelfError::from(source);

        assert!(error.is_persistence());
        assert_eq!(error.to_string(), "save failed");
    }

    #[test]
    fn validated_setup_debug_redacts_api_key() {
        let setup = AudiobookshelfValidatedSetup::new(
            mbv_config::AudiobookshelfSetup::new("http://abs:13378"),
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

    mod failure;
    mod playback;
    mod progress;
}
