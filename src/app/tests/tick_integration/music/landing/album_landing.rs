use super::*;

fn landed_album_level(
    parent_id: &str,
    title: &str,
    items: Vec<mbv_core::api::EmbyItem>,
) -> crate::app::BrowseLevel {
    crate::app::BrowseLevel {
        fetched_rows: 0,
        parent_id: parent_id.into(),
        title: title.into(),
        total_count: items.len(),
        items,
        resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

/// The landed nav stack the recursive album activation builds for a grouped
/// library: root artist level + the artist's album level, the navigated album
/// at the album level's resting cursor. The sibling album is present (the
/// activation fetches the whole parent listing), so preserving the owner's
/// prior target would miss the navigated album without the re-anchor.
pub(super) fn landed_grouped_album_stack(album_id: &str) -> Vec<crate::app::BrowseLevel> {
    let mut artist = crate::app::tests::make_item("Alpha", "MusicArtist");
    artist.id = "group-0".into();
    artist.is_folder = true;
    let mut album = crate::app::tests::make_item("First Album", "MusicAlbum");
    album.id = album_id.into();
    album.artist = "Alpha".into();
    let mut sibling = crate::app::tests::make_item("Second Album", "MusicAlbum");
    sibling.id = "album-2".into();
    sibling.artist = "Alpha".into();
    vec![
        landed_album_level("lib-music", "Music", vec![artist]),
        landed_album_level("group-0", "Alpha", vec![album, sibling]),
    ]
}

/// Task 3.2 (grouped shape): a navigated album lands through the same
/// `RecursiveAlbumActivated` shell arm Inline Search activation uses, so the
/// retained Music owner's workspace re-anchors onto the navigated album and
/// shows its track list -- even after the user moved the owner's own cursor
/// elsewhere.
#[test]
fn navigated_album_reanchors_the_grouped_owner_workspace() {
    let (mut harness, id) = wide_music_harness();
    // A sibling album so the owner's own cursor can move off the navigated
    // one before the landing.
    {
        let app = &mut harness.model_mut().app;
        let mut second = crate::app::tests::make_item("Second Album", "MusicAlbum");
        second.id = "album-2".into();
        second.artist = "Alpha".into();
        let level = app.libs[0].nav_stack.last_mut().expect("album level");
        level.items.push(second);
        level.total_count = 2;
        app.album_tracks_cache.insert(
            "album-2".into(),
            vec![crate::app::tests::make_item("Other Track", "Audio")],
        )
    };
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(key(Key::Down));
    harness.step();
    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-2"),
        "the owner's local cursor moved off the navigated album"
    );

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the owner workspace re-anchors onto the navigated album"
    );
    assert_eq!(
        music_track_focus_row(&harness, &id),
        Some(0),
        "track-selection mode is entered for the navigated album's track list"
    );
}

/// Task 6.2 (design D6): "Go to Library" on a queued track selects the track
/// in the workspace track list once the activated album's track rows arrive —
/// including when the fetch lands after the landing.
#[test]
fn navigated_track_is_selected_in_the_workspace_track_list() {
    let (mut harness, id) = wide_music_harness();
    // The queued track: the navigation carries it as deep selection bound to
    // the activated album.
    harness.model_mut().app.pending_track_selection = Some((0, "track-2".into()));

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the album landing stands"
    );
    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(1),
        "the chosen track is selected in the track list"
    );
    assert!(
        harness.model().pending_music_track_selection.is_none(),
        "the deep selection is fully consumed"
    );
    assert_ne!(
        harness.model().app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Error,
        "a successful deep selection does not flash: {}",
        harness.model().app.status
    );
}

/// Task 6.2: the track is absent from the fetched track list — the album
/// landing stands with the default selection and no error.
#[test]
fn absent_track_keeps_the_landing_with_default_selection() {
    let (mut harness, id) = wide_music_harness();
    harness.model_mut().app.pending_track_selection = Some((0, "track-gone".into()));

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the album landing stands"
    );
    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(0),
        "default selection: first track"
    );
    assert!(
        harness.model().pending_music_track_selection.is_none(),
        "the absent-track pending is cleared"
    );
    assert_ne!(
        harness.model().app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Error,
        "absence is not failure: {}",
        harness.model().app.status
    );
}

/// Task 6.2: the album's tracks have not arrived when the activation drains —
/// the pending selection stays armed and applies on the tracks re-push.
#[test]
fn navigated_track_selection_waits_for_the_album_tracks() {
    let mut app = crate::app::render::make_music_group_app();
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _id = ComponentId::Library;

    harness.model_mut().app.pending_track_selection = Some((0, "track-2".into()));
    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();
    assert!(
        harness.model().pending_music_track_selection.is_some(),
        "the pending selection stays armed while the track fetch is in flight"
    );

    // The tracks arrive (the `AlbumTracksFetched` drain re-pushes).
    let mut first = crate::app::tests::make_item("Track One", "Audio");
    first.id = "track-1".into();
    let mut second = crate::app::tests::make_item("Track Two", "Audio");
    second.id = "track-2".into();
    harness
        .model_mut()
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![first, second]);
    harness.model_mut().sync_mounted_surfaces();

    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(1),
        "the chosen track is selected once its rows arrive"
    );
    assert!(harness.model().pending_music_track_selection.is_none());
}
