mod armed_entry;
mod discography_activation;
mod neighbour_prefetch;
mod workspace_entry;

use super::*;

/// A grouped Music app with five Alpha albums in settled order
/// (album-1..album-5), one cached track each: deterministic artist groups,
/// album titles, and neighbour window.
fn mounted_neighbour_app() -> crate::app::App {
    let mut app = crate::app::render::make_music_group_app();
    app.image_protocol_enabled = true;
    {
        let level = app.libs[0].nav_stack.last_mut().expect("album level");
        level.items[0].name = "Album 1".into();
        for number in 2..=5 {
            let mut album = crate::app::tests::make_item(&format!("Album {number}"), "MusicAlbum");
            album.id = format!("album-{number}");
            album.artist = "Alpha".into();
            album.production_year = 2001;
            level.items.push(album);
        }
        level.total_count = 5;
    }
    for number in 1..=5 {
        let mut track = crate::app::tests::make_item(&format!("Track {number}"), "Audio");
        track.id = format!("track-{number}");
        track.album_id = format!("album-{number}");
        app.album_tracks_cache
            .insert(format!("album-{number}"), vec![track]);
    }
    app
}
/// A two-root Grouped Music app (Alpha over `album-1`, Beta over two albums)
/// with no cached track rows anywhere: every artist Workspace entry arms with
/// its rows still in flight, so the arrival path is observable hermetically.
fn two_artist_app() -> crate::app::App {
    let mut app = crate::app::render::make_music_group_app();
    let level = app.libs[0].nav_stack.last_mut().expect("album level");
    for (name, id) in [
        ("Beta Session", "album-beta-1"),
        ("Beta Nights", "album-beta-2"),
    ] {
        let mut album = crate::app::tests::make_item(name, "MusicAlbum");
        album.id = id.into();
        album.artist = "Beta".into();
        level.items.push(album);
    }
    level.total_count = level.items.len();
    app
}
/// Deposits one cached track for `album_id` (the fallback artist Workspace's
/// aggregation source) and re-projects the workspace: the rows "arrive".
fn arrive_album_tracks(harness: &mut TickHarness, album_id: &str, track_id: &str) {
    let mut track = crate::app::tests::make_item(track_id, "Audio");
    track.album_id = album_id.into();
    harness
        .model_mut()
        .app
        .album_tracks_cache
        .insert(album_id.to_string(), vec![track]);
    harness.model_mut().push_music_workspace_content();
    harness.model_mut().sync_mounted_surfaces();
}
