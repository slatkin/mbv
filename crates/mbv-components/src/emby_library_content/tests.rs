//! Launch-state contract tests for the Emby library owner: the persisted
//! pill scope is always Latest for a pill-bearing library, other pill
//! choices and the selected item are session memory (change
//! `latest-pill-restart-default`).

use super::{BrowserOwnerPush, EmbyLibraryContent, EmbySelectorMode};
use crate::library_panel::owner::{LaunchSelector, LibraryContentOwner};
use mbv_config::{
    EmbyLetterBucket, EmbySelectorKey, LaunchPanelFocus, SelectorIdentity,
    TUI_LAUNCH_STATE_VERSION, TabIdentity, TuiLaunchState,
};
use mbv_emby_model::test_support::make_item;
use mbv_ui_model::library::LibraryKind;
use mbv_ui_model::sort_filter::{LetterFilter, LetterFilterKind};
use mbv_ui_msg::{Msg, ShellRequest};
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
