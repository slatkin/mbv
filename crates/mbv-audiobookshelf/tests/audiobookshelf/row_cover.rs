use super::*;
use mbv_net::mock_http::MockHttp;
use rstest::rstest;

const BOUND: Duration = Duration::from_secs(1);

fn mock_client() -> (AudiobookshelfClient, MockHttp) {
    let http = MockHttp::new();
    let agent = http.agent();
    http.respond(200, "cover-bytes");
    let client = AudiobookshelfClient::new("http://127.0.0.1:1")
        .unwrap()
        .with_test_agent(agent);
    (client, http)
}

// Design D6: a row cover asks for the row's 2x painted width and a JPEG
// response, never the full-size `raw=1` image.
#[rstest]
#[case::book(
    ROW_BOOK_COVER_WIDTH,
    "GET /api/items/book-1/cover?width=128&format=jpeg HTTP/1.1"
)]
#[case::podcast(
    ROW_PODCAST_COVER_WIDTH,
    "GET /api/items/book-1/cover?width=72&format=jpeg HTTP/1.1"
)]
fn row_cover_requests_the_row_width_as_jpeg(#[case] width: u32, #[case] expected_request: &str) {
    let (client, http) = mock_client();

    client
        .row_cover_bounded("secret", "book-1", width, BOUND)
        .unwrap();

    let captured = http.requests();
    assert_eq!(captured.len(), 1);
    assert!(captured[0].starts_with(expected_request));
    assert!(!captured[0].contains("raw=1"));
}
