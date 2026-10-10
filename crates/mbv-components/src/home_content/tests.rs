//! Home's interactive input-translation tests (row 5.2 of
//! `home-recently-added-tree`): the owner's key handling and pointer-gesture
//! translation against the shared tree, one contract per test.

use super::*;
use mbv_emby_model::test_support::make_item;
use mbv_ui_model::targets::{HomeRoot, RecentAge};
use mbv_ui_msg::ShellRequest;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use tuirealm::component::Component;
use tuirealm::event::{Key, KeyModifiers};

/// A recent epoch keeps the fixture arithmetic in range (the tree.rs
/// projection fixture's window).
fn window() -> HomeLatestLaunchWindow {
    let day = 24 * 60 * 60;
    HomeLatestLaunchWindow {
        previous: Some(1_500_000_000 - day),
        current: 1_500_000_000,
    }
}

/// One Emby item dated inside the launch window's `New` branch.
fn new_item(name: &str) -> QueueItem {
    let mut item = make_item(name, "Movie");
    item.id = name.to_string();
    item.date_added = iso_secs(window().current - 60);
    QueueItem::Emby(Box::new(item))
}

fn iso_secs(secs: u64) -> String {
    use time::format_description::well_known::Iso8601;
    time::OffsetDateTime::from_unix_timestamp(i64::try_from(secs).unwrap_or(i64::MAX))
        .expect("in-range fixture timestamp")
        .format(&Iso8601::DEFAULT)
        .expect("ISO-8601 formatting always succeeds")
}

fn recent_row(item: QueueItem) -> HomeRecentRow {
    HomeRecentRow {
        item,
        feed_name: None,
    }
}

fn key(code: Key, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent { code, modifiers }
}

/// An owner whose `Recently added` root holds the `New` branch with the
/// given items; `Continue watching` is empty and omitted.
fn owner_with_new_items(items: Vec<QueueItem>) -> HomeContent {
    let mut owner = HomeContent::new();
    owner.set_recent_rows(items.into_iter().map(recent_row).collect(), window());
    owner
}

/// Paint one tree frame so the pointer resolves rows from retained
/// geometry, mirroring the shared tree-owner paint seam.
fn paint_tree(owner: &mut HomeContent, height: u16) {
    let mut terminal = Terminal::new(TestBackend::new(20, height)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut owner.tree, frame, Rect::new(1, 0, 18, height)))
        .unwrap();
}

fn new_branch() -> HomeTreeTarget {
    HomeTreeTarget::Age(RecentAge::New)
}

fn recent_target(item: &QueueItem) -> HomeTreeTarget {
    HomeTreeTarget::Item(HomeItemTarget::Recent(item.content_id()))
}

/// Design D6 activation (spec scenario "Collapse a branch"): Enter on the
/// expanded `New` branch collapses it and keeps it selected.
#[test]
fn enter_on_expanded_branch_collapses_it_and_keeps_it_selected() {
    let mut owner = owner_with_new_items(vec![new_item("alpha")]);
    owner.tree.apply(TreeOperation::Select(new_branch()));
    assert!(owner.tree.is_expanded(&new_branch()));

    owner.handle_key(&key(Key::Enter, KeyModifiers::NONE));

    assert!(!owner.tree.is_expanded(&new_branch()));
    assert_eq!(owner.tree.selected_target(), Some(&new_branch()));
}

/// Design D6 (Right never collapses): Right on the expanded
/// `Recently added` root keeps it expanded and keeps it selected.
#[test]
fn right_on_expanded_root_keeps_it_expanded() {
    let mut owner = owner_with_new_items(vec![new_item("alpha")]);
    owner.tree.apply(TreeOperation::Select(HomeTreeTarget::Root(
        HomeRoot::RecentlyAdded,
    )));
    let root = HomeTreeTarget::Root(HomeRoot::RecentlyAdded);
    assert!(owner.tree.is_expanded(&root));

    owner.handle_key(&key(Key::Right, KeyModifiers::NONE));

    assert!(owner.tree.is_expanded(&root));
    assert_eq!(owner.tree.selected_target(), Some(&root));
}

/// Design D6 action chords (spec scenario "Mark a branch and enqueue"):
/// marking the `New` branch and pressing Ctrl+Enter emits its item targets
/// in display order.
#[test]
fn ctrl_enter_after_marking_new_branch_emits_its_item_targets_in_order() {
    let alpha = new_item("alpha");
    let beta = new_item("beta");
    let mut owner = owner_with_new_items(vec![alpha.clone(), beta.clone()]);
    paint_tree(&mut owner, 5);
    // Row 1 is the `New` branch under the `Recently added` root.
    owner.on_slot_event(crate::library_panel::LibrarySlotEvent::List(
        crate::media_list::MediaListSurfaceInput::ToggleClick(Position::new(2, 1)),
    ));

    let message = owner.handle_key(&key(Key::Enter, KeyModifiers::CONTROL));

    let Some(Msg::Shell(request)) = message else {
        panic!("Ctrl+Enter with marks must emit a shell request");
    };
    assert_eq!(
        *request,
        ShellRequest::HomeEnqueue(vec![
            HomeItemTarget::Recent(alpha.content_id()),
            HomeItemTarget::Recent(beta.content_id()),
        ])
    );
}

/// Spec scenario "Same item in both roots": an in-progress episode that is
/// also newly added yields two section-scoped targets, and selecting one
/// occurrence does not select the other.
#[test]
fn item_under_both_roots_yields_two_separately_selectable_targets() {
    let shared = new_item("shared");
    let mut owner = HomeContent::new();
    owner.set_content(vec![shared.clone()], false);
    owner.set_recent_rows(vec![recent_row(shared.clone())], window());
    paint_tree(&mut owner, 6);
    // Rows: 0 `Continue watching` root, 1 the Continue occurrence, 2 the
    // `Recently added` root, 3 the `New` branch, 4 the Recent occurrence.
    owner.on_slot_event(crate::library_panel::LibrarySlotEvent::List(
        crate::media_list::MediaListSurfaceInput::Click(Position::new(2, 1)),
    ));
    let continue_target = HomeTreeTarget::Item(HomeItemTarget::continue_item("shared"));
    assert_eq!(owner.tree.selected_target(), Some(&continue_target));
    assert_ne!(owner.tree.selected_target(), Some(&recent_target(&shared)));

    // The selection change released the retained geometry; the panel
    // repaints every frame, so the next click resolves against a fresh one.
    paint_tree(&mut owner, 6);

    owner.on_slot_event(crate::library_panel::LibrarySlotEvent::List(
        crate::media_list::MediaListSurfaceInput::Click(Position::new(2, 4)),
    ));

    assert_eq!(owner.tree.selected_target(), Some(&recent_target(&shared)));
    assert_ne!(owner.tree.selected_target(), Some(&continue_target));
}
