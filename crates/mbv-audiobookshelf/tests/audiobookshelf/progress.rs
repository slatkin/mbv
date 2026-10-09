use super::*;
use rstest::rstest;

use mbv_net::mock_http::MockHttp;

const BOUND: Duration = Duration::from_secs(1);

fn mock_client(status: u16) -> (AudiobookshelfClient, MockHttp) {
    let http = MockHttp::new();
    let agent = http.agent();
    http.respond(status, "{}");
    let client = AudiobookshelfClient::new("http://127.0.0.1:1")
        .unwrap()
        .with_test_agent(agent);
    (client, http)
}

fn mark_episode_played(client: &AudiobookshelfClient) -> Result<(), AudiobookshelfError> {
    client.set_finished_bounded(
        "secret",
        "<LIBRARY_ITEM_ID>",
        Some("<EPISODE_ID>"),
        true,
        BOUND,
    )
}

fn mark_book_unplayed(client: &AudiobookshelfClient) -> Result<(), AudiobookshelfError> {
    client.set_finished_bounded("secret", "<LIBRARY_ITEM_ID>", None, false, BOUND)
}

fn batch_mark_episode_and_book(client: &AudiobookshelfClient) -> Result<(), AudiobookshelfError> {
    client.batch_set_finished_bounded(
        "secret",
        &[
            ProgressFinishedUpdate {
                library_item_id: "<LIBRARY_ITEM_ID>".to_string(),
                episode_id: Some("<EPISODE_ID>".to_string()),
                is_finished: true,
            },
            ProgressFinishedUpdate {
                library_item_id: "<BOOK_LIBRARY_ITEM_ID>".to_string(),
                episode_id: None,
                is_finished: false,
            },
        ],
        BOUND,
    )
}

// Contract (standard-media-context-menus tasks 1.1/1.2, audiobookshelf-played-state
// "Marking one item writes its finished state to the server" and "Marking a
// multi-selection uses one batch request"): each finished-state request goes out
// as PATCH on the provider-native path, carries the bearer key, and names the
// item by `libraryItemId` plus `episodeId` only for episodes; a 401 maps to the
// authentication-rejected class.
#[rstest]
#[case::episode(
    mark_episode_played,
    "PATCH /api/me/progress/%3CLIBRARY_ITEM_ID%3E/%3CEPISODE_ID%3E HTTP/1.1",
    &["\"isFinished\":true"],
)]
#[case::book(
    mark_book_unplayed,
    "PATCH /api/me/progress/%3CLIBRARY_ITEM_ID%3E HTTP/1.1",
    &["\"isFinished\":false"],
)]
#[case::batch(
    batch_mark_episode_and_book,
    "PATCH /api/me/progress/batch/update HTTP/1.1",
    &[
        "{\"libraryItemId\":\"<LIBRARY_ITEM_ID>\",\"episodeId\":\"<EPISODE_ID>\",\"isFinished\":true}",
        "{\"libraryItemId\":\"<BOOK_LIBRARY_ITEM_ID>\",\"isFinished\":false}",
    ],
)]
fn progress_requests_use_patch_the_provider_path_and_the_json_shape(
    #[case] request: fn(&AudiobookshelfClient) -> Result<(), AudiobookshelfError>,
    #[case] request_line: &str,
    #[case] body_fragments: &[&str],
) {
    let (client, http) = mock_client(200);
    request(&client).unwrap();

    let captured = http.requests();
    assert_eq!(captured.len(), 1);
    assert!(captured[0].starts_with(request_line));
    assert!(
        captured[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer secret\r\n")
    );
    // ureq 3.x pretty-prints JSON; strip whitespace before matching.
    let body = captured[0]
        .split_once("\r\n\r\n")
        .map_or("", |(_, body)| body);
    let no_ws = body
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    for fragment in body_fragments {
        assert!(no_ws.contains(fragment));
    }

    let (client, _) = mock_client(401);
    let error = request(&client).unwrap_err();
    assert_eq!(
        error.class,
        AudiobookshelfFailureClass::AuthenticationRejected
    );
}
