#[test]
fn all_audio_accepts_audio_items() {
    assert!(all_audio(&[
        item("song1", "Audio", "Audio"),
        item("song2", "Audio", "Audio"),
    ]));
}

#[test]
fn all_audio_rejects_video_items() {
    assert!(!all_audio(&[
        item("song", "Audio", "Audio"),
        item("movie", "Video", "Movie"),
    ]));
}

#[test]
fn audio_only_daemon_rejects_non_audio_play_request() {
    let fetched = [item("movie", "Video", "Movie")];
    let rejection = audio_only_rejection(true, &fetched);
    assert!(rejection.is_some_and(|r| !r.is_empty()));
}

#[test]
fn audio_only_daemon_accepts_audio_play_request() {
    let fetched = [item("song", "Audio", "Audio")];
    assert!(audio_only_rejection(true, &fetched).is_none());
}

#[test]
fn non_audio_only_daemon_never_rejects() {
    let fetched = [item("movie", "Video", "Movie")];
    assert!(audio_only_rejection(false, &fetched).is_none());
}
