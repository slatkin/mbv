//! The Playlists screen's control contract: which controls a playlist and an
//! item offer, and the command each one sends.

use super::*;

fn playlist() -> Pick {
    Pick::Playlist("pl".to_owned())
}

fn item() -> Pick {
    Pick::Item {
        playlist: "pl".to_owned(),
        item: "i1".to_owned(),
    }
}

/// A playlist offers the full control set; an item only the three actions.
#[test]
fn a_playlist_offers_six_controls_and_an_item_three() {
    assert_eq!(controls_for(&playlist()).len(), 6);
    assert_eq!(
        controls_for(&item()),
        [Control::Play, Control::Shuffle, Control::Add]
    );
}

/// An item's action names the open playlist and the item by id.
#[test]
fn an_item_action_names_the_playlist_and_the_item() {
    assert_eq!(
        command_for(&item(), Control::Shuffle),
        Some(GuiPlaylistsCommand::ItemAction {
            id: "pl".to_owned(),
            item_id: "i1".to_owned(),
            action: GuiPlaylistAction::Shuffle,
        })
    );
}

/// Delete and Refresh send for a playlist; Rename opens a local draft and
/// sends nothing.
#[test]
fn rename_sends_nothing_and_delete_names_the_playlist() {
    assert_eq!(command_for(&playlist(), Control::Rename), None);
    assert_eq!(
        command_for(&playlist(), Control::Delete),
        Some(GuiPlaylistsCommand::Delete {
            id: "pl".to_owned()
        })
    );
}
