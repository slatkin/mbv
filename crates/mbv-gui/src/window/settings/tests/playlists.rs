//! The Playlists section's rename and item actions (the contracts of
//! `playlists.test.ts`, design D9): the rename field sends
//! `playlistRename` with the playlist's id and the typed name, and an item's
//! add sends `playlistItemAction` with both ids.

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::bridge::frame::settings::playlists::{GuiOpenPlaylist, GuiPlaylistItem, GuiPlaylistRow};
use crate::bridge::settings::GuiPlaylistAction;
use crate::window::settings::entries::{entry_id, settings_entries};
use crate::window::settings::state::{SettingsForm, SettingsState};

use super::{Facts, run, settings_commands};

/// The Playlists section's content with one open playlist.
fn playlists_content() -> GuiSettingsContent {
    GuiSettingsContent::Playlists {
        playlists: vec![GuiPlaylistRow {
            id: "pl1".to_owned(),
            name: "Mix".to_owned(),
            item_count: 2,
        }],
        open: Some(GuiOpenPlaylist {
            id: "pl1".to_owned(),
            name: "Mix".to_owned(),
            items: vec![
                GuiPlaylistItem {
                    id: "i1".to_owned(),
                    name: "Song A".to_owned(),
                },
                GuiPlaylistItem {
                    id: "i2".to_owned(),
                    name: "Song B".to_owned(),
                },
            ],
        }),
        loading: false,
        open_loading: false,
    }
}

/// Contract (`playlists.test.ts`): a rename sends playlistRename with the id
/// and the typed name.
#[test]
fn a_rename_sends_playlist_rename_with_the_id_and_the_typed_name() {
    let (facts, rx) = run(|state: &SettingsState, _facts: &Facts| {
        state.open_rename_draft("pl1", "Mix");
        state.set_rename_draft("Road mix");
        state.focus_settings_field(SettingsForm::Rename);
        state.submit_settings_form(&playlists_content());
    });
    assert_eq!(*facts, [] as [&str; 0]);
    assert_eq!(
        settings_commands(&rx),
        vec![mbv::GuiSettingsCommand::PlaylistRename {
            id: "pl1".to_owned(),
            name: "Road mix".to_owned(),
        }],
    );
}

/// Contract (`playlists.test.ts`): an item's add sends playlistItemAction
/// with both ids.
#[test]
fn an_items_add_sends_playlist_item_action_with_both_ids() {
    let (facts, rx) = run(|state: &SettingsState, _facts: &Facts| {
        let content = playlists_content();
        let entries = settings_entries(&content, &[]);
        state.press_settings_entry(
            &entries,
            &entry_id::playlist_item_action("pl1", "i2", GuiPlaylistAction::Enqueue),
        );
    });
    assert_eq!(*facts, [] as [&str; 0]);
    assert_eq!(
        settings_commands(&rx),
        vec![mbv::GuiSettingsCommand::PlaylistItemAction {
            id: "pl1".to_owned(),
            item_id: "i2".to_owned(),
            action: mbv_ui_msg::MusicTreeAction::Enqueue,
        }],
    );
}
