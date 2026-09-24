// Audio-only admission for the daemon submission predicate: `all_audio` and
// `audio_only_rejection` are pure functions exercised directly, independent of
// the socket/player harness.

#[test]
fn all_audio_accepts_audio_items() {
    assert!(all_audio(&[
        emby_qi("song1", "Audio", "Audio"),
        emby_qi("song2", "Audio", "Audio"),
    ]));
}

#[test]
fn all_audio_rejects_video_items() {
    assert!(!all_audio(&[
        emby_qi("song", "Audio", "Audio"),
        emby_qi("movie", "Video", "Movie"),
    ]));
}

#[test]
fn all_audio_rejects_video_feed_items() {
    assert!(!all_audio(&[video_feed_qi("feed-1")]));
}

#[test]
fn audio_only_daemon_rejects_video_feed_play_request() {
    let fetched = [video_feed_qi("feed-1")];
    let rejection = audio_only_rejection(true, &fetched);
    assert!(rejection.is_some_and(|r| !r.is_empty()));
}

#[test]
fn audio_only_daemon_rejects_non_audio_play_request() {
    let fetched = [emby_qi("movie", "Video", "Movie")];
    let rejection = audio_only_rejection(true, &fetched);
    assert!(rejection.is_some_and(|r| !r.is_empty()));
}

#[test]
fn audio_only_daemon_accepts_audio_play_request() {
    let fetched = [emby_qi("song", "Audio", "Audio")];
    assert!(audio_only_rejection(true, &fetched).is_none());
}

#[test]
fn non_audio_only_daemon_never_rejects() {
    let fetched = [emby_qi("movie", "Video", "Movie")];
    assert!(audio_only_rejection(false, &fetched).is_none());
}
