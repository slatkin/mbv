//! The Playlists screen's control contract: which controls a playlist and an
//! item offer, the command each one sends, and the command a double-click on a
//! playlist row sends.

use rstest::rstest;

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

/// A double-click opens a closed playlist and collapses the open one
/// (gui-tree-double-click design D1, D5).
#[rstest]
#[case::closed(false, GuiPlaylistsCommand::Open { id: "pl".to_owned() })]
#[case::open(true, GuiPlaylistsCommand::Collapse)]
fn a_double_click_toggles_the_playlist_row(
    #[case] is_open: bool,
    #[case] expected: GuiPlaylistsCommand,
) {
    assert_eq!(toggle_command(is_open, "pl"), expected);
}
