//! Launch-state contract tests for the Emby library owner: the persisted
//! pill scope is always Latest for a pill-bearing library, other pill
//! choices and the selected item are session memory (change
//! `latest-pill-restart-default`). The wheel group owns the owner's
//! wheel-scrolls-viewport reach contract (task 4.1).

use super::{BrowserOwnerPush, EmbyLibraryContent, EmbySelectorMode};
use crate::library_panel::owner::{LaunchSelector, LibraryContentOwner, LibrarySlotEvent};
use crate::media_list::MediaListSurfaceInput;
use mbv_config::{
    EmbyLetterBucket, EmbySelectorKey, LaunchPanelFocus, SelectorIdentity,
    TUI_LAUNCH_STATE_VERSION, TabIdentity, TuiLaunchState,
};
use mbv_emby_model::test_support::make_item;
use mbv_ui_model::library::LibraryKind;
use mbv_ui_model::sort_filter::{LetterFilter, LetterFilterKind};
use mbv_ui_msg::{Msg, ShellRequest};
use ratatui::layout::{Position, Rect};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

fn key(code: Key) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }
}

fn push_with_letters() -> BrowserOwnerPush {
    let mut item = make_item("Item", "Movie");
    item.id = "item-a".into();
    BrowserOwnerPush {
        items: vec![item],
        latest_items: Vec::new(),
        total_count: 1,
        library_total: Some(400),
        letter_filter: Some(
            LetterFilter::for_index_for_kind(0, LetterFilterKind::Movie)
                .expect("movie letter index 0"),
        ),
        loading: false,
        selector_mode: EmbySelectorMode::Letters,
        feed_groups: Vec::new(),
        feed_group_ids: Vec::new(),
        feed_group_cursor: 0,
    }
}

fn launch_state(selector: Option<mbv_config::SelectorIdentity>) -> TuiLaunchState {
    TuiLaunchState {
        version: TUI_LAUNCH_STATE_VERSION,
        tab: TabIdentity::Home,
        panel_focus: LaunchPanelFocus::Library,
        selector,
        item: None,
    }
}

#[test]
fn launch_snapshot_records_latest_and_no_item_despite_an_active_letter_pill() {
    let mut owner = EmbyLibraryContent::new(LibraryKind::Movies);
    owner.set_content(push_with_letters());

    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Latest,
            }),
            None,
        )
    );
}

#[test]
fn launch_snapshot_records_latest_for_a_feed_group_library() {
    let mut owner = EmbyLibraryContent::new(LibraryKind::HomeVideos);
    let mut push = push_with_letters();
    push.selector_mode = EmbySelectorMode::FeedGroups;
    push.letter_filter = None;
    push.feed_groups = vec!["Group A".into()];
    push.feed_group_ids = vec!["group-a".into()];
    push.feed_group_cursor = 1;
    owner.set_content(push);

    assert_eq!(
        owner.launch_snapshot().0,
        Some(SelectorIdentity::Emby {
            key: EmbySelectorKey::Latest,
        })
    );
}

#[test]
fn a_pill_less_library_records_no_selector() {
    let mut owner = EmbyLibraryContent::new(LibraryKind::Movies);
    let mut push = push_with_letters();
    push.selector_mode = EmbySelectorMode::None;
    push.letter_filter = None;
    owner.set_content(push);

    assert_eq!(owner.launch_snapshot().0, None);
}

#[test]
fn restore_applies_only_a_latest_selector() {
    // Latest applies while the owner is not already in Latest mode...
    let mut owner = EmbyLibraryContent::new(LibraryKind::Movies);
    owner.set_content(push_with_letters());
    assert_eq!(
        owner.launch_selector(&launch_state(Some(SelectorIdentity::Emby {
            key: EmbySelectorKey::Latest,
        }))),
        Some(LaunchSelector::EmbyLatest)
    );
    // ...and a legacy letter selector in an old snapshot applies nothing,
    // leaving the library default.
    assert_eq!(
        owner.launch_selector(&launch_state(Some(SelectorIdentity::Emby {
            key: EmbySelectorKey::Letter(EmbyLetterBucket::from_index(0).expect("bucket 0")),
        }))),
        None
    );
}

/// Context-menu spec, "Every painted library row resolves its own menu"
/// (standard-media-context-menus task 3.3, design D7): a homevideos
/// feed-group row's menu targets the video the row paints, though the nav
/// level holds only the channel folders.
#[test]
fn feed_group_video_row_menu_key_emits_the_painted_item() {
    let mut owner = EmbyLibraryContent::new(LibraryKind::HomeVideos);
    let mut video = make_item("Video", "Movie");
    video.id = "video-a".into();
    owner.set_content(BrowserOwnerPush {
        items: vec![video.clone()],
        latest_items: Vec::new(),
        total_count: 1,
        library_total: None,
        letter_filter: None,
        loading: false,
        selector_mode: EmbySelectorMode::FeedGroups,
        feed_groups: vec!["Group A".into()],
        feed_group_ids: vec!["group-a".into()],
        feed_group_cursor: 0,
    });

    assert_eq!(
        owner.handle_key(&key(Key::Char('.'))),
        Some(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            mbv_ui_model::context_menu::ContextMenuTargets::Emby(vec![video]),
            None,
        ))))
    );
}

/// Wheel-scrolls-viewport task 4.1 (D4): the wheel scrolls the viewport
/// freely, the selection stays put, and the reach report resolves the last
/// painted selectable row into the same item index space the owner's cursor
/// reports use — structural grouping rows (heading, spacer) never shift it.
#[test]
fn wheel_reach_resolves_the_last_painted_selectable_row_as_an_item_index() {
    let mut owner = EmbyLibraryContent::new(LibraryKind::Movies);
    let items: Vec<mbv_emby_model::EmbyItem> = ["Alpha", "Bravo", "Charlie", "Delta"]
        .iter()
        .zip([0, 1, 2, 3])
        .map(|(name, i)| {
            let mut item = make_item(name, "Movie");
            item.id = format!("item-{i}");
            item
        })
        .collect();
    // A large total forces the letter-grouped projection: the painted flow
    // grows heading/spacer rows the reach must not resolve through.
    owner.set_content(BrowserOwnerPush {
        items,
        latest_items: Vec::new(),
        total_count: 4,
        library_total: Some(60),
        letter_filter: None,
        loading: false,
        selector_mode: EmbySelectorMode::None,
        feed_groups: Vec::new(),
        feed_group_ids: Vec::new(),
        feed_group_cursor: 0,
    });

    // Complete one painted frame with a three-row viewport so the wheel
    // resolves its reach from retained geometry.
    let list = Rect::new(0, 0, 40, 3);
    let geometry = owner.carrier.wide().row_geometry(3);
    owner
        .carrier
        .wide_mut()
        .finish_view(list, list, &geometry, None);

    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at: Position::new(1, 1),
        delta: 3,
    }));

    // Painted flow after the 3-row scroll: Charlie, Spacer, Heading(D–F) —
    // the last selectable painted row is "Charlie", the items index the
    // selection would have reported: 2.
    assert_eq!(
        message,
        Some(Msg::Shell(Box::new(ShellRequest::LibraryViewportReach {
            index: 2
        })))
    );
    assert_eq!(owner.cursor(), 0, "the wheel never moves the selection");
    assert_eq!(owner.scroll(), 3, "the free viewport offset took the step");
}

/// Regression (wheel-scrolls-viewport 4.1 review P1): a wheel over an
/// empty library with a painted frame must not panic; the empty list
/// resolves no reach, so no `LibraryViewportReach` request leaves the
/// owner.
#[test]
fn wheel_over_an_empty_library_reports_no_reach_and_never_panics() {
    let mut owner = EmbyLibraryContent::new(LibraryKind::Movies);
    owner.set_content(BrowserOwnerPush {
        items: Vec::new(),
        latest_items: Vec::new(),
        total_count: 0,
        library_total: None,
        letter_filter: None,
        loading: false,
        selector_mode: EmbySelectorMode::None,
        feed_groups: Vec::new(),
        feed_group_ids: Vec::new(),
        feed_group_cursor: 0,
    });

    let list = Rect::new(0, 0, 40, 3);
    let geometry = owner.carrier.wide().row_geometry(3);
    owner
        .carrier
        .wide_mut()
        .finish_view(list, list, &geometry, None);

    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at: Position::new(1, 1),
        delta: 3,
    }));

    assert_eq!(message, None);
}
