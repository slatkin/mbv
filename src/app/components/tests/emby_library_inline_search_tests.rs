use crate::app::components::emby_library_content::{
    BrowserOwnerPush, EmbyLibraryContent as BrowserOwner,
};
use crate::app::components::inline_search::{InlineSearchHost, SearchPool};
use crate::app::components::library_panel::content::{ListSlot, PanelList, PanelListPaintPolicy};
use crate::app::components::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use crate::app::components::library_panel::LibraryKind;
use crate::app::components::media_list::{MediaListOperation, MediaListSurfaceInput};
use crate::app::components::msg::{Msg, ShellRequest};
use crate::app::render::LetterFilter;
use crate::app::tests::{make_item, make_items};
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use std::time::{Duration, Instant};
use tuirealm::event::{Key, KeyEvent as TuiKeyEvent, KeyModifiers};

// The wide-Movies right-rail Inline Search painting this file used to cover
// through the former standalone browser moved with Movies/HomeVideos/Generic to the
// embedded `EmbyLibraryContent` owner (task 6.1): the panel's Wide/Narrow
// skeletons already prove `ListSlot::Search` painting generically
// (`library_panel::wide`/`narrow` tests), so the owner-level tests below
// prove only this owner's translation of the slot/key events into `Msg`s.

fn owner_push(items: Vec<mbv_core::api::EmbyItem>) -> BrowserOwnerPush {
    let total_count = items.len();
    BrowserOwnerPush {
        items,
        latest_items: Vec::new(),
        total_count,
        library_total: None,
        letter_filter: None,
        loading: false,
        group_pills: false,
        show_letter_pills: false,
        feed_groups: Vec::new(),
        feed_group_ids: Vec::new(),
        feed_group_cursor: 0,
    }
}

/// `/` opens the embedded Inline Search control on the migrated owner
/// (design D1/D3/D4) and offers `ListSlot::Search` instead of the ordinary
/// media list; the shell-side `OpenInlineSearch` load request is unchanged.
#[test]
fn browser_owner_slash_opens_inline_search_as_a_list_slot() {
    let mut owner = BrowserOwner::new(LibraryKind::Movies);
    owner.set_content(owner_push(make_items(3)));
    assert!(matches!(owner.content().list, ListSlot::Media(_)));

    let message = owner.on_key(&TuiKeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(
        message,
        Some(Msg::Shell(Box::new(ShellRequest::OpenInlineSearch)))
    );
    assert!(matches!(owner.content().list, ListSlot::Search(_)));
}

/// While search is open, a character that is otherwise a list shortcut (`r`
/// -> `EmbyLibraryRefresh`) is appended to the query instead of running the
/// shortcut; the first keystroke reports the query-start edge the shell
/// turns into the corpus load (design.md D4).
#[test]
fn browser_owner_search_open_shortcut_letter_becomes_query_text() {
    let mut owner = BrowserOwner::new(LibraryKind::Generic);
    owner.set_content(owner_push(make_items(3)));
    owner.on_key(&TuiKeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });

    let message = owner.on_key(&TuiKeyEvent {
        code: Key::Char('r'),
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(owner.inline_search().query(), "r");
    assert_eq!(
        message,
        Some(Msg::Shell(Box::new(ShellRequest::InlineSearchQueryStarted))),
        "the first keystroke reports the query-start edge"
    );
    // Subsequent keystrokes are not edges: no repeat corpus-load request.
    let message = owner.on_key(&TuiKeyEvent {
        code: Key::Char('u'),
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(owner.inline_search().query(), "ru");
    assert_eq!(
        message, None,
        "only the empty-to-non-empty edge emits the request"
    );
}

/// Hit geometry follows the carrier's latest retained rects (task 6.3): a
/// repaint re-anchors the flow's rect, a click resolves against what the
/// carrier retained from the newest paint, and a point from the previous
/// frame's geometry resolves nothing.
#[test]
fn browser_owner_search_pointer_resolves_against_the_latest_repaint() {
    let mut owner = BrowserOwner::new(LibraryKind::Movies);
    owner.set_content(owner_push(make_items(2)));
    owner.on_key(&TuiKeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });
    let mut alpha = make_item("Search Result Alpha", "Movie");
    alpha.id = "ida".into();
    let mut beta = make_item("Search Result Beta", "Movie");
    beta.id = "idb".into();
    owner
        .inline_search_mut()
        .set_pool(SearchPool::Items(vec![alpha, beta]));
    owner.on_key(&TuiKeyEvent {
        code: Key::Char('a'),
        modifiers: KeyModifiers::NONE,
    });
    assert!(
        owner
            .inline_search_mut()
            .handle_clock(Instant::now() + Duration::from_millis(301)),
        "the deadline fires the armed re-score"
    );

    let paint = |owner: &mut BrowserOwner, rect: Rect| {
        let height = rect.y + rect.height;
        let mut terminal = Terminal::new(TestBackend::new(rect.width, height)).unwrap();
        terminal
            .draw(|f| {
                let search = owner.inline_search_mut();
                search.clamp_viewport(rect.height as usize);
                search.set_paint_policy(PanelListPaintPolicy::Wide { focused: true });
                search.set_geometry(rect, rect);
                search.view(f, rect);
            })
            .unwrap();
    };

    // First paint, then move the cursor to the second row.
    paint(&mut owner, Rect::new(0, 0, 40, 10));
    owner
        .inline_search_mut()
        .results_mut()
        .delegate_operation(MediaListOperation::Move(1));
    assert_eq!(owner.inline_search().test_cursor(), 1);

    // Repaint with new geometry; the carrier's retained rects re-anchor.
    paint(&mut owner, Rect::new(0, 5, 40, 5));

    // A stale point from the previous frame's geometry resolves nothing.
    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
        Position::new(0, 0),
    )));
    assert!(message.is_none());
    assert_eq!(
        owner.inline_search().test_cursor(),
        1,
        "a point from the previous frame's geometry does not move the selection"
    );

    // A click resolves against the newest retained rects: row 0 sits at y=5.
    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
        Position::new(0, 5),
    )));
    assert!(message.is_none(), "a plain click emits no Msg");
    assert_eq!(
        owner.inline_search().test_cursor(),
        0,
        "the click selected the repainted row 0"
    );

    // A double-click on the repainted row 1 activates that row's target.
    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
        Position::new(0, 6),
    )));
    assert_eq!(
        message,
        Some(Msg::Shell(Box::new(ShellRequest::InlineSearchActivate {
            id: "idb".into(),
            item_type: "Movie".into(),
        }))),
        "the double-click activated the row the latest paint retained"
    );
}

/// Pointer input against a painted search result row (design D3): the
/// owner's `handle_search_pointer` translation resolves click/double-click/
/// right-click/wheel through the embedded control's own retained geometry,
/// exactly as the prior TV/Movies browse owner did before this
/// task, minus the raw-event "press in the bar, release on a row"
/// cross-region gesture the panel's normalized `MediaListSurfaceInput` cannot carry
/// (documented deviation, task 6.1 report).
#[test]
fn browser_owner_search_pointer_resolves_against_painted_rows() {
    let mut owner = BrowserOwner::new(LibraryKind::Movies);
    owner.set_content(owner_push(vec![make_item("Focused Movie", "Movie")]));
    owner.on_key(&TuiKeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });
    owner.inline_search_mut().set_pool(SearchPool::Items(vec![
        make_item("Search Result Alpha", "Movie"),
        {
            let mut beta = make_item("Search Result Beta", "Movie");
            // Stable targets must be unique: the target-addressed list cannot
            // distinguish two rows sharing the default fixture id.
            beta.id = "search-beta".into();
            beta
        },
    ]));
    // Score a query so the result rows exist (an empty query shows none):
    // type a character, then fire the debounce with a clock tick past the
    // deadline.
    owner.on_key(&TuiKeyEvent {
        code: Key::Char('a'),
        modifiers: KeyModifiers::NONE,
    });
    assert!(
        owner
            .inline_search_mut()
            .handle_clock(Instant::now() + Duration::from_millis(301)),
        "the deadline fires the armed re-score"
    );
    // The panel would have painted the session through its PanelList surface;
    // seed the carrier's retained geometry the same way so the owner's point
    // resolution has real painted rows to read.
    let row_area = Rect::new(0, 0, 40, 10);
    let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
    terminal
        .draw(|f| {
            let search = owner.inline_search_mut();
            search.clamp_viewport(row_area.height as usize);
            search.set_paint_policy(PanelListPaintPolicy::Wide { focused: true });
            search.set_geometry(row_area, row_area);
            search.view(f, row_area);
        })
        .unwrap();

    let at = Position::new(0, 0);
    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(at)));
    assert!(message.is_none(), "a plain click emits no Msg");
    assert_eq!(owner.inline_search().test_cursor(), 0);

    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at,
        delta: 1,
    }));
    assert!(matches!(message, Some(Msg::TerminalEvent(_))));
    assert_eq!(owner.inline_search().test_cursor(), 1);

    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
        at,
    )));
    assert_eq!(
        message,
        Some(Msg::Shell(Box::new(ShellRequest::InlineSearchActivate {
            id: "id".into(),
            item_type: "Movie".into(),
        }))),
        "double-click activates the row it selected"
    );

    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ContextClick(
        at,
    )));
    assert!(
        matches!(
            message,
            Some(Msg::Shell(ref shell_boxed))
                 if matches!(shell_boxed.as_ref(), ShellRequest::RowContextMenu(crate::app::state::types::context_menu::ContextMenuTargets::Browser(ref items), _) if items.len() == 1 && items[0] == "id")),
        "a right click on a result row opens its context menu: {message:?}"
    );
}

// Task 2.1: the bounded read-only launch-state query on the generic-Emby
// owner. Pill identities are stable keys — the closed letter bucket's fixed
// value or the selected feed/home-video group's folder content ID — and the
// item identity is the shared carrier's stable target. No pill index, group
// display name, or row position crosses; a shown unfiltered scope has an
// explicit identity, while pill-less and empty views report absence.
#[test]
fn browser_owner_latest_pill_switches_rows_and_restores_home_video_group_position() {
    use crate::app::components::library_panel::content::ListSlot;
    use crate::app::components::library_panel::LibraryContentOwner;
    use mbv_core::config::{EmbySelectorKey, SelectorIdentity};

    let mut push = owner_push(vec![
        {
            let mut item = make_item("Group One First", "Movie");
            item.id = "group-first".into();
            item
        },
        {
            let mut item = make_item("Group One Selected", "Movie");
            item.id = "group-selected".into();
            item
        },
    ]);
    push.latest_items = vec![{
        let mut item = make_item("Latest Movie", "Movie");
        item.id = "latest-movie".into();
        item
    }];
    push.group_pills = true;
    push.feed_groups = vec!["Group One".into()];
    push.feed_group_ids = vec!["group-one".into()];
    push.feed_group_cursor = 1;
    let mut owner = BrowserOwner::new(LibraryKind::HomeVideos);
    owner.set_content(push);
    owner.apply_position(1, 0);

    let selector = owner.content().selector.expect("selector row");
    assert_eq!(selector.pills, vec!["Latest", "All", "Group One"]);
    assert_eq!(selector.active, Some(2));
    let launch_state = mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(SelectorIdentity::Emby {
            key: EmbySelectorKey::Latest,
        }),
        item: None,
    };
    assert_eq!(
        owner.launch_selector(&launch_state),
        Some(crate::app::components::library_panel::owner::LaunchSelector::EmbyLatest)
    );
    let selected = owner.on_slot_event(LibrarySlotEvent::SelectorPicked(0));
    assert_eq!(
        selected,
        Some(Msg::Shell(Box::new(
            ShellRequest::EmbyLibraryLatestSelected
        )))
    );
    assert!(owner.latest_mode());
    assert_eq!(
        owner.launch_snapshot().0,
        Some(SelectorIdentity::Emby {
            key: EmbySelectorKey::Latest,
        })
    );
    assert_eq!(owner.cursor(), 0);
    assert!(matches!(owner.content().list, ListSlot::Media(_)));

    let selected = owner.on_slot_event(LibrarySlotEvent::SelectorPicked(2));
    assert_eq!(
        selected,
        Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryLatestExit {
            target: 1
        })))
    );
    assert!(!owner.latest_mode());
    owner.set_content({
        let mut push = owner_push(vec![
            {
                let mut item = make_item("Group One First", "Movie");
                item.id = "group-first".into();
                item
            },
            {
                let mut item = make_item("Group One Selected", "Movie");
                item.id = "group-selected".into();
                item
            },
        ]);
        push.latest_items = vec![make_item("Latest Movie", "Movie")];
        push.group_pills = true;
        push.feed_groups = vec!["Group One".into()];
        push.feed_group_ids = vec!["group-one".into()];
        push.feed_group_cursor = 1;
        push
    });
    assert_eq!(
        owner.cursor(),
        1,
        "the previous group's selected row is restored"
    );
    assert_eq!(
        owner.launch_snapshot().0,
        Some(SelectorIdentity::Emby {
            key: EmbySelectorKey::Group("group-one".into()),
        })
    );
}

#[test]
fn browser_owner_latest_participates_in_letter_and_group_cycle() {
    let mut push = owner_push(make_items(2));
    push.show_letter_pills = true;
    let mut owner = BrowserOwner::new(LibraryKind::Movies);
    owner.set_content(push);
    assert_eq!(
        owner.on_key(&TuiKeyEvent {
            code: Key::Char('['),
            modifiers: KeyModifiers::NONE,
        }),
        Some(Msg::Shell(Box::new(
            ShellRequest::EmbyLibraryLatestSelected
        )))
    );
    assert!(owner.latest_mode());
    assert_eq!(
        owner.on_key(&TuiKeyEvent {
            code: Key::Char(']'),
            modifiers: KeyModifiers::NONE,
        }),
        Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryLatestExit {
            target: usize::MAX
        })))
    );
    assert!(!owner.latest_mode());

    let mut push = owner_push(make_items(2));
    push.group_pills = true;
    push.feed_groups = vec!["Group".into()];
    push.feed_group_ids = vec!["group-id".into()];
    push.feed_group_cursor = 0;
    owner.set_content(push);
    assert_eq!(
        owner.on_key(&TuiKeyEvent {
            code: Key::Char('['),
            modifiers: KeyModifiers::NONE,
        }),
        Some(Msg::Shell(Box::new(
            ShellRequest::EmbyLibraryLatestSelected
        )))
    );
    assert!(owner.latest_mode());
    assert_eq!(
        owner.on_key(&TuiKeyEvent {
            code: Key::Char(']'),
            modifiers: KeyModifiers::NONE,
        }),
        Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryLatestExit {
            target: 0
        })))
    );
    assert!(!owner.latest_mode());
}

#[test]
fn browser_owner_launch_snapshot_reports_letter_pill_and_item_identities() {
    use mbv_core::config::{
        EmbyLetterBucket, EmbySelectorKey, LibraryItemIdentity, SelectorIdentity,
    };

    let bucket = LetterFilter::for_index_for_kind(2, crate::app::render::LetterFilterKind::Movie)
        .expect("letter buckets exist");
    let mut push = owner_push(make_items(3));
    push.show_letter_pills = true;
    push.letter_filter = Some(bucket);
    let mut owner = BrowserOwner::new(LibraryKind::Movies);
    owner.set_content(push);

    let (selector, item) = owner.launch_snapshot();
    assert_eq!(
        selector,
        Some(SelectorIdentity::Emby {
            key: EmbySelectorKey::Letter(EmbyLetterBucket::GToI),
        }),
        "the letter pill resolves to its fixed bucket identity, never its label"
    );
    assert_eq!(
        item,
        Some(LibraryItemIdentity::Emby {
            id: "id0".to_string(),
        }),
        "the item resolves to the carrier's stable target"
    );
}

#[test]
fn browser_owner_reanchors_selector_before_item_and_falls_back_when_item_is_missing() {
    use mbv_core::config::{
        EmbyLetterBucket, EmbySelectorKey, LibraryItemIdentity, SelectorIdentity, TuiLaunchState,
    };

    let mut owner = BrowserOwner::new(LibraryKind::Movies);
    let mut push = owner_push(make_items(3));
    push.show_letter_pills = true;
    push.letter_filter =
        LetterFilter::for_index_for_kind(2, crate::app::render::LetterFilterKind::Movie);
    owner.set_content(push);
    let state = TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(SelectorIdentity::Emby {
            key: EmbySelectorKey::Letter(EmbyLetterBucket::GToI),
        }),
        item: Some(LibraryItemIdentity::Emby { id: "gone".into() }),
    };
    assert!(owner.reanchor_launch_state(&state));
    assert_eq!(owner.launch_snapshot().0, state.selector);
    assert_eq!(
        owner.launch_snapshot().1,
        Some(LibraryItemIdentity::Emby { id: "id0".into() })
    );
}

#[test]
fn browser_owner_launch_snapshot_reports_group_content_id_never_the_display_name() {
    use mbv_core::config::{EmbySelectorKey, SelectorIdentity};

    let mut push = owner_push(make_items(2));
    push.group_pills = true;
    push.feed_groups = vec!["Displayed Name".to_string()];
    push.feed_group_ids = vec!["folder-id-7".to_string()];
    push.feed_group_cursor = 1;
    let mut owner = BrowserOwner::new(LibraryKind::Generic);
    owner.set_content(push);

    let (selector, _) = owner.launch_snapshot();
    assert_eq!(
        selector,
        Some(SelectorIdentity::Emby {
            key: EmbySelectorKey::Group("folder-id-7".to_string()),
        }),
        "a dynamic group pill resolves to its folder content ID, not its label"
    );
}

#[test]
fn browser_owner_launch_snapshot_reports_absence_for_unfiltered_and_empty_views() {
    use mbv_core::config::{EmbySelectorKey, LibraryItemIdentity, SelectorIdentity};

    // No pills at all (a small library): no selector, but the item remains.
    let mut owner = BrowserOwner::new(LibraryKind::Movies);
    owner.set_content(owner_push(make_items(2)));
    assert_eq!(
        owner.launch_snapshot(),
        (
            None,
            Some(LibraryItemIdentity::Emby {
                id: "id0".to_string(),
            })
        )
    );

    // The "All" group pill is the unfiltered scope: it has an explicit
    // selector identity so it is distinct from a destination with no pills.
    let mut push = owner_push(make_items(2));
    push.group_pills = true;
    push.feed_groups = vec!["Displayed Name".to_string()];
    push.feed_group_ids = vec!["folder-id-7".to_string()];
    push.feed_group_cursor = 0;
    owner.set_content(push);
    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Unfiltered,
            }),
            Some(LibraryItemIdentity::Emby {
                id: "id0".to_string(),
            })
        ),
        "the All pill reports an explicit unfiltered selector"
    );

    // Letter pills shown but no bucket filtered: the unfiltered scope again.
    let mut push = owner_push(make_items(2));
    push.show_letter_pills = true;
    push.letter_filter = None;
    owner.set_content(push);
    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Unfiltered,
            }),
            Some(LibraryItemIdentity::Emby {
                id: "id0".to_string(),
            })
        )
    );

    // An empty list: no item either.
    owner.set_content(owner_push(Vec::new()));
    assert_eq!(owner.launch_snapshot(), (None, None));
}
