//! Contract (issue #1040): the one authority parser reads the host after any
//! userinfo, so `normalize_feed_url` and `FeedPlatform::of_url` agree on a
//! URL. A userinfo that names `YouTube` reaches the host only from a real
//! `YouTube` authority, and a URL with no path after the host keeps its
//! pre-#1040 pass-through: each URL below is left as it is.

use mbv_feed::normalize_feed_url;
use rstest::rstest;

/// Each URL here is outside the pathed `YouTube` authority, so
/// `normalize_feed_url` leaves it as it is. The path case fails on the
/// pre-fix parser (it read host `youtube.com` and rewrote the `channel`
/// form to `YouTube`'s feed URL); the query and fragment cases fail on a
/// parser that ends the authority at a `?` or `#`; the no-path case fails
/// on one that reads the query as the path, which errored the add flow's
/// `invalid_youtube_url`.
#[rstest]
#[case::userinfo_in_path("https://youtube.com@evil.example/channel/UCabc")]
#[case::userinfo_in_query("https://evil.example?@youtube.com/channel/UCabc")]
#[case::userinfo_in_fragment("https://evil.example#@youtube.com/channel/UCabc")]
#[case::no_path("https://www.youtube.com?si=x")]
fn a_url_that_is_not_a_pathed_youtube_url_passes_through(#[case] url: &str) {
    assert_eq!(normalize_feed_url(url).unwrap(), url);
}

/// Userinfo before a real `YouTube` host still normalizes: the host is
/// `www.youtube.com`, so the `channel` form rewrites with no network call.
#[test]
fn userinfo_before_the_youtube_host_still_normalizes() {
    let url = "https://user:pass@www.youtube.com/channel/UCabc";
    assert_eq!(
        normalize_feed_url(url).unwrap(),
        "https://www.youtube.com/feeds/videos.xml?channel_id=UCabc"
    );
}
