//! Shared Grouped Music tree-owner fixtures: the multi-artist corpus builder,
//! the key/row shorthands, and the painted-frame/geometry readers every tree
//! interaction test drives the owner through.

use super::*;

// ── Task 2.4: tree chord mapping through the component boundary ──────────

/// A multi-artist Grouped Music owner whose tree starts on the first album of
/// the first root (`selected_album` drives the initial adoption). `artists` is
/// `(artist, [album target…])` in settled order; the derived title is the
/// target so assertions can name leaves.
pub(super) fn tree_owner(artists: &[(&str, &[&str])]) -> MusicContent {
    tree_owner_with_tracks(artists, None)
}

pub(super) fn tree_owner_with_tracks(
    artists: &[(&str, &[&str])],
    album_tracks: Option<Vec<EmbyItem>>,
) -> MusicContent {
    let mut items: Vec<EmbyItem> = Vec::new();
    let mut album_info: Vec<(String, String, String)> = Vec::new();
    let mut artist_keys: Vec<crate::app::state::music_grouping::ArtistKey> = Vec::new();
    for (artist, targets) in artists {
        for target in *targets {
            let mut album = make_item(target, "MusicAlbum");
            album.id = (*target).to_string();
            album.artist = (*artist).to_string();
            // Grouped Music album rows are folder targets: shell actions must
            // expand them through the existing per-folder playback effects.
            album.is_folder = true;
            items.push(album);
            album_info.push((
                (*artist).to_string(),
                "2001".to_string(),
                (*target).to_string(),
            ));
            artist_keys.push(crate::app::state::music_grouping::ArtistKey::Service(
                format!("artist-{artist}"),
            ));
        }
    }
    let selected = items.first().cloned();
    let order: Vec<usize> = (0..items.len()).collect();
    let ctx = MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(items, 0),
        selected,
        String::new(),
        Vec::new(),
        0,
        album_info,
        artist_keys,
        order,
        album_tracks,
    );
    let mut owner = MusicContent::new();
    owner.set_content(ctx);
    owner.selection_origin = Some(SelectionOrigin::Library(
        crate::app::components::media_list::LibrarySelectionOrigin::Service(
            crate::app::components::library_panel::owner::LibraryKey::Service {
                service: mbv_core::config::ServiceKind::Emby,
                library_id: "music-library".into(),
                kind: crate::app::components::library_panel::owner::LibraryKind::Music,
            },
        ),
    ));
    owner
}

pub(super) fn press(owner: &mut MusicContent, code: Key) -> Option<Msg> {
    owner.on_key(&KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}
