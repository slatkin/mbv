use super::*;

fn wide_music_harness() -> (TickHarness, ComponentId) {
    let mut app = crate::app::render::make_music_group_app();
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut first = crate::app::tests::make_item("Track One", "Audio");
    first.id = "track-1".into();
    let mut second = crate::app::tests::make_item("Track Two", "Audio");
    second.id = "track-2".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![first, second]);
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    (harness, ComponentId::Library)
}

/// The focused track-pane row, or `None` when the track pane is unfocused
/// (design.md D5: focus is parent state, the owner holds the selection).
fn music_track_focus_row(harness: &TickHarness, _id: &ComponentId) -> Option<usize> {
    let music = harness.model().test_music_owner();
    music
        .track_focused()
        .then(|| music.track_selected_row())
        .flatten()
}

fn music_workspace<'a>(harness: &'a TickHarness, _id: &ComponentId) -> &'a MusicContent {
    harness.model().test_music_owner()
}

fn music_album_cursor(harness: &TickHarness, id: &ComponentId) -> usize {
    music_workspace(harness, id).album_cursor()
}

fn music_selected_album_id(harness: &TickHarness, id: &ComponentId) -> Option<String> {
    music_workspace(harness, id)
        .selected_item()
        .map(|item| item.id)
}

fn album_row(id: &str, name: &str) -> mbv_core::api::EmbyItem {
    let mut album = crate::app::tests::make_item(name, "MusicAlbum");
    album.id = id.into();
    album
}

mod artist;
mod focus_routing;
mod inline_search;
mod landing;
mod playlists;
