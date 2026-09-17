use crate::app::components::emby_library_content::{
    BrowserOwnerPush, EmbyLibraryContent as BrowserOwner,
};
use crate::app::components::inline_search::{InlineSearchHost, SearchPool};
use crate::app::components::library_panel::content::{ListSlot, PanelList, PanelListPaintPolicy};
use crate::app::components::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use crate::app::components::library_panel::LibraryKind;
use crate::app::components::media_list::{MediaListOperation, MediaListSurfaceInput};
use crate::app::components::msg::{Msg, ShellRequest};
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
        total_count,
        library_total: None,
        letter_filter: None,
        loading: false,
        group_pills: false,
        home_video: false,
        show_letter_pills: false,
        feed_groups: Vec::new(),
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
    assert_eq!(message, Some(Msg::Shell(ShellRequest::OpenInlineSearch)));
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
        Some(Msg::Shell(ShellRequest::InlineSearchQueryStarted)),
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
                search.sync_viewport(rect.height as usize);
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
        Some(Msg::Shell(ShellRequest::InlineSearchActivate {
            id: "idb".into(),
            item_type: "Movie".into(),
        })),
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
        make_item("Search Result Beta", "Movie"),
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
            search.sync_viewport(row_area.height as usize);
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
        Some(Msg::Shell(ShellRequest::InlineSearchActivate {
            id: "id".into(),
            item_type: "Movie".into(),
        })),
        "double-click activates the row it selected"
    );

    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ContextClick(
        at,
    )));
    assert!(
        matches!(
            message,
            Some(Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Browser(ref items), _)))
                if items.len() == 1 && items[0] == "id"
        ),
        "a right click on a result row opens its context menu: {message:?}"
    );
}
