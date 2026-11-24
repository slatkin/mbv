#[test]
fn fetch_album_tracks_is_a_no_op_when_already_loading() {
    let mut app = crate::app::tests::make_app_stub();
    app.album_tracks_loading.insert("album-1".into());

    app.fetch_album_tracks("album-1".into());

    assert!(
        !app.album_tracks_cache.contains_key("album-1"),
        "a duplicate call while a fetch is already in flight must not \
         spawn a second fetch or fabricate a cache entry"
    );
}
