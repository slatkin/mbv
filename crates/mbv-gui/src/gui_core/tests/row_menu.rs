//! The GUI Client's Queue row menu (queue-content-shuffle spec, "The Queue
//! list menu offers Shuffle"): the per-row right-click menu the window content
//! paints offers the same Queue list Shuffle entry the TUI offers. The fixture
//! is the core seam a TUI-side Queue list menu test seeds too, so the two
//! Clients' menus are read from one shape.

use super::*;
use mbv_ui_model::context_menu::ContextMenuEntry;

/// The ordered entries of the GUI row menu the GUI Client opens on `slot`:
/// what the frame carries to the window content.
fn gui_row_menu(app: &mut App, slot: mbv_queue::QueueSlotId) -> Vec<ContextMenuEntry> {
    crate::gui_core::test_support::gui_row_menu_entries_for_test(app, slot)
}

/// queue-content-shuffle spec, "The Queue list menu offers Shuffle": the GUI
/// Client's per-row right-click menu offers the same Queue list Shuffle entry
/// the TUI offers, after the Play now entry and before the removals (a music
/// row, which takes no mark or go-to entries, shows the full order).
#[test]
fn the_gui_row_menu_offers_shuffle_before_the_removals() {
    let mut app = queue_menu_app(true, leaf_item("queued track", "Audio", "Audio", false));

    let labels: Vec<_> = gui_row_menu(&mut app, mbv_queue::QueueSlotId::from_raw(1))
        .iter()
        .map(|entry| entry.label)
        .collect();

    assert_eq!(
        labels,
        vec!["Play now", "Shuffle", "Remove from queue", "Clear queue"]
    );
}

/// queue-content-shuffle spec, "The Queue list menu offers Shuffle" (Owner
/// without the capability): the GUI row menu hides the entry when the owner
/// does not advertise the `queue-shuffle` ctrl capability.
#[test]
fn the_gui_row_menu_hides_shuffle_without_the_capability() {
    let mut app = queue_menu_app(false, leaf_item("queued movie", "Movie", "Video", false));

    let labels: Vec<_> = gui_row_menu(&mut app, mbv_queue::QueueSlotId::from_raw(1))
        .iter()
        .map(|entry| entry.label)
        .collect();

    assert!(!labels.contains(&"Shuffle"));
}

/// A stub app over the shuffle-capable or plain owner, with Queue focused and
/// `item` as the one displayed slot (queue-content-shuffle design D6).
fn queue_menu_app(shuffle_capable: bool, item: EmbyItem) -> App {
    let mut app = make_app_stub();
    seed_queue_shuffle_owner_for_test(&mut app, shuffle_capable);
    seed_queue_row_menu_rows_for_test(&mut app, vec![mbv_queue::QueueItem::Emby(Box::new(item))]);
    app
}

/// An `EmbyItem` of `name`, `item_type`, and `media_type`, played or not.
fn leaf_item(name: &str, item_type: &str, media_type: &str, played: bool) -> EmbyItem {
    let mut item = make_item(name, item_type);
    item.media_type = media_type.into();
    item.played = played;
    item
}
