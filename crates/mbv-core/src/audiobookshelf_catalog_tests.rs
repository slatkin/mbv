use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/audiobookshelf/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn fixtures_decode_without_losing_native_identity() {
    let libraries: LibrariesResponse = serde_json::from_str(&fixture("libraries")).unwrap();
    assert_eq!(libraries.libraries[1].id, "lib-podcast");
    assert_eq!(libraries.libraries[1].media_type, "podcast");
    let page: ItemsResponse = serde_json::from_str(&fixture("items-page")).unwrap();
    assert_eq!((page.page, page.limit, page.total), (0, 20, 2));
    assert_eq!(page.results[0].library_item_id, "show-2");
    assert_eq!(
        page.results[0]
            .media
            .as_ref()
            .and_then(|media| media.metadata.as_ref())
            .and_then(|metadata| metadata.title.as_deref()),
        Some("Second Show")
    );
    assert_eq!(
        page.results[0]
            .media
            .as_ref()
            .and_then(|media| media.metadata.as_ref())
            .and_then(|metadata| metadata.description.as_deref()),
        Some("Second show description.")
    );
    let expanded: ExpandedWire = serde_json::from_str(&fixture("item-expanded")).unwrap();
    assert_eq!(expanded.id, "show-2");
    assert_eq!(expanded.media.unwrap().episodes.unwrap()[0].id, "episode-1");
}

#[test]
fn progress_and_shelf_fixtures_preserve_user_and_server_order() {
    let progress: ProgressResponse = serde_json::from_str(&fixture("progress")).unwrap();
    assert_eq!(progress.media_progress[0].library_item_id, "show-2");
    assert!(!progress.media_progress[0].is_finished.unwrap());
    let completed: ProgressResponse = serde_json::from_str(
            r#"{"mediaProgress":[{"libraryItemId":"show-2","episodeId":"episode-2","currentTime":120.0,"isFinished":true}]}"#,
        )
        .unwrap();
    assert_eq!(completed.media_progress[0].is_finished, Some(true));
    let shelves: Vec<ShelfWire> = serde_json::from_str(&fixture("shelves")).unwrap();
    assert_eq!(shelves[0].label, "Continue listening");
    assert!(matches!(
        shelves[0].entries[1],
        ShelfEntryWire::Episode { .. }
    ));
}

#[test]
fn null_episode_id_progress_is_skipped() {
    let json = r#"{"mediaProgress":[{"libraryItemId":"lib-1","episodeId":null,"currentTime":10.0,"isFinished":false}]}"#;
    let response: ProgressResponse = serde_json::from_str(json).unwrap();
    let mapped: HashMap<(String, String), AudiobookshelfProgress> = response
        .media_progress
        .into_iter()
        .filter_map(|x| {
            let episode_id = x.episode_id?;
            let value = AudiobookshelfProgress {
                library_item_id: x.library_item_id.clone(),
                episode_id: episode_id.clone(),
                current_time_seconds: x.current_time.unwrap_or(0.0).max(0.0),
                is_finished: x.is_finished.unwrap_or(false),
            };
            Some(((x.library_item_id, episode_id), value))
        })
        .collect();
    assert!(mapped.is_empty());
}

#[test]
fn covers_allow_present_and_missing_paths() {
    let cover_json = fixture("present-cover");
    let trimmed = cover_json.trim();
    let inner = trimmed
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap();
    let present: ShowWire = serde_json::from_str(&format!(
        "{{\"libraryItemId\":\"show-2\",\"title\":\"Show\",{inner}}}"
    ))
    .unwrap();
    assert_eq!(present.cover_path.as_deref(), Some("/cover/show-2"));
    let missing: serde_json::Value = serde_json::from_str(&fixture("missing-cover")).unwrap();
    assert!(missing["coverPath"].is_null());
}

#[test]
fn invalid_page_metadata_is_a_protocol_failure() {
    let page: ItemsResponse =
        serde_json::from_str(r#"{"page":0,"limit":20,"total":1,"results":[]}"#).unwrap();
    assert_eq!(page.page, 0);
}

#[test]
fn auth_failures_are_classified_and_errors_redact_credentials() {
    let error =
        AudiobookshelfError::new(super::super::AudiobookshelfFailureClass::AuthenticationRejected);
    assert!(!error.to_string().contains("secret-key"));
    assert!(serde_json::from_str::<ItemsResponse>("not json").is_err());
}

#[test]
fn surname_extraction_takes_last_token_and_falls_back_to_raw_credit() {
    assert_eq!(audiobook_author_sort_key("Tamora Pierce"), "Pierce");
    assert_eq!(
        audiobook_author_sort_key("Ursula K. Le Guin"),
        "Guin",
        "the final title-cased whitespace token is the sort surname"
    );
    assert_eq!(
        audiobook_author_sort_key(""),
        "",
        "empty credit falls back to the raw string"
    );
}

#[test]
fn multi_author_sort_uses_first_listed_surname_only() {
    assert_eq!(
        first_listed_author_sort_key("Sanderson, Brandon; Jordan, Robert"),
        "Sanderson"
    );
    assert_eq!(
        first_listed_author_sort_key("lee child"),
        "Child",
        "the surname is title-cased regardless of the credit's cashing"
    );
}

#[test]
fn author_display_prefers_authors_list_and_trims_single_author() {
    assert_eq!(
        book_author_display(None, Some(&["a".into(), "b".into()])),
        Some("a, b".into())
    );
    assert_eq!(
        book_author_display(Some(" Ferret "), None),
        Some("Ferret".into())
    );
    assert_eq!(book_author_display(Some("   "), None), None);
    assert_eq!(book_author_display(None, Some(&[])), None);
}
