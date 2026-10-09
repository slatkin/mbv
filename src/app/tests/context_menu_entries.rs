//! The standard action set for Emby context menus (context-menu spec,
//! "Library lists share one standard action set", the Played label, and the
//! music rule). Standard-media-context-menus tasks 2.1-2.3.
use super::*;
use crate::app::state::context_menu_capabilities::{ItemCapabilities, emby_item_capabilities};
use mbv_ui_model::overlay::OverlayRequest;
use rstest::rstest;

fn emby_library_app() -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = mbv_ui_model::tab_selection::TabSelection::EmbyLibrary(0);
    app
}

/// Open the menu for `item` on an Emby library tab and return the ordered
/// entry labels.
fn menu_labels_for(app: &mut App, item: EmbyItem) -> Vec<&'static str> {
    app.open_context_menu_for(item);
    match app.pending_overlay.take() {
        Some(OverlayRequest::ContextMenu(menu)) => {
            menu.entries.iter().map(|entry| entry.label).collect()
        }
        other => panic!("expected a context menu overlay, got {other:?}"),
    }
}

fn leaf_item(name: &str, item_type: &str, media_type: &str, played: bool) -> EmbyItem {
    let mut item = make_item(name, item_type);
    item.media_type = media_type.into();
    item.played = played;
    item
}

fn folder_item(name: &str, item_type: &str, unplayed_children: u32) -> EmbyItem {
    let mut item = make_item(name, item_type);
    item.is_folder = true;
    item.unplayed_item_count = unplayed_children;
    item
}

/// One case per item shape: the standard action set for that shape (context
/// menu spec, "Library lists share one standard action set", the Played label,
/// and the music rule).
#[rstest]
#[case::unplayed_movie(
    leaf_item("movie", "Movie", "Video", false),
    &["Play", "Add to Queue", "Mark Played"],
)]
#[case::played_movie(
    leaf_item("movie", "Movie", "Video", true),
    &["Play", "Add to Queue", "Mark Unplayed"],
)]
#[case::season_with_unplayed_children(
    folder_item("Season 1", "Season", 5),
    &["Play All", "Shuffle", "Add to Queue", "Mark Played"],
)]
#[case::music_album(
    folder_item("Album", "MusicAlbum", 0),
    &["Play All", "Shuffle", "Add to Queue"],
)]
#[case::music_track(leaf_item("Track", "Audio", "Audio", false), &["Play", "Add to Queue"])]
fn emby_menu_follows_the_standard_action_set_for(
    #[case] item: EmbyItem,
    #[case] expected: &[&str],
) {
    let mut app = emby_library_app();
    assert_eq!(menu_labels_for(&mut app, item), expected);
}

#[test]
fn mixed_selection_with_a_music_track_drops_the_mark_entries() {
    let mut app = emby_library_app();
    let items = vec![
        leaf_item("movie", "Movie", "Video", false),
        leaf_item("Track", "Audio", "Audio", false),
    ];
    let capabilities: Vec<ItemCapabilities> = items.iter().map(emby_item_capabilities).collect();

    app.open_context_menu_for_selection(
        &items,
        None,
        PanelFocus::Library,
        capabilities,
        Vec::new(),
    );

    let labels = match app.pending_overlay.take() {
        Some(OverlayRequest::ContextMenu(menu)) => menu
            .entries
            .iter()
            .map(|entry| entry.label)
            .collect::<Vec<_>>(),
        other => panic!("expected a context menu overlay, got {other:?}"),
    };
    assert!(
        labels.iter().all(|label| !label.starts_with("Mark ")),
        "music in the selection must drop both mark entries: {labels:?}"
    );
}
