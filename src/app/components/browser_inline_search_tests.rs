use super::*;

/// `/` opens the embedded Inline Search control locally (design.md D1/D4)
/// and still emits `OpenInlineSearch` for the shell-side full-library-load
/// work the control has no authority over.
#[test]
fn emby_browser_slash_opens_inline_search() {
    let mut browser = BrowserComponent::new();
    browser.set_content(BrowserContent::from_items(make_items(3)));
    browser.set_focused(true);
    assert!(!browser.inline_search().is_active());

    let message = browser.handle_tui_key(TuiKeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });

    assert!(browser.inline_search().is_active());
    assert_eq!(message, Some(Msg::Shell(ShellRequest::OpenInlineSearch)));
}

/// While search is open, a character that is otherwise a list shortcut (`r`
/// -> `BrowserRefresh`) is appended to the query instead of running the
/// shortcut, and the component returns immediately without an ordinary
/// `Msg` (design.md D4).
#[test]
fn emby_browser_search_open_shortcut_letter_becomes_query_text() {
    let mut browser = BrowserComponent::new();
    browser.set_content(BrowserContent::from_items(make_items(3)));
    browser.set_focused(true);
    browser.handle_tui_key(TuiKeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });

    let message = browser.handle_tui_key(TuiKeyEvent {
        code: Key::Char('r'),
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(browser.inline_search().query(), "r");
    assert_eq!(
        message, None,
        "shortcut letter must not reach the ordinary handler while search is open"
    );
}

// The wide-Movies right-rail Inline Search painting this file used to cover
// through `BrowserComponent` moved with Movies/HomeVideos/Generic to the
// embedded `BrowserContent` owner (task 6.1): the panel's Wide/Narrow
// skeletons already prove `ListSlot::Search` painting generically
// (`library_panel::wide`/`narrow` tests), so the owner-level tests below
// prove only this owner's translation of the slot/key events into `Msg`s.

use crate::app::components::browser_content::{BrowserContent as BrowserOwner, BrowserOwnerPush};
use crate::app::components::library_panel::content::ListSlot;
use crate::app::components::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use crate::app::components::media_list::RowLocalInput;
use ratatui::layout::{Position, Rect};

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
    let mut owner = BrowserOwner::new(BrowserKind::Movies);
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
/// -> `BrowserRefresh`) is appended to the query instead of running the
/// shortcut, and the owner returns immediately without an ordinary `Msg`
/// (design.md D4).
#[test]
fn browser_owner_search_open_shortcut_letter_becomes_query_text() {
    let mut owner = BrowserOwner::new(BrowserKind::Generic);
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
        message, None,
        "shortcut letter must not reach the ordinary handler while search is open"
    );
}

/// Pointer input against a painted search result row (design D3): the
/// owner's `handle_search_pointer` translation resolves click/double-click/
/// right-click/wheel through the embedded control's own retained geometry,
/// exactly as the mounted `BrowserComponent` did for TV/Movies before this
/// task, minus the raw-event "press in the bar, release on a row"
/// cross-region gesture the panel's normalized `RowLocalInput` cannot carry
/// (documented deviation, task 6.1 report).
#[test]
fn browser_owner_search_pointer_resolves_against_painted_rows() {
    let mut owner = BrowserOwner::new(BrowserKind::Movies);
    owner.set_content(owner_push(vec![make_item("Focused Movie", "Movie")]));
    owner.on_key(&TuiKeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });
    owner.inline_search_mut().set_pool(SearchPool::Items(vec![
        make_item("Search Result Alpha", "Movie"),
        make_item("Search Result Beta", "Movie"),
    ]));
    // The panel would have set this from its own paint; seed it directly so
    // the owner's point resolution has real painted geometry to read.
    owner.inline_search_mut().layout_mut().left_area = Rect::new(0, 0, 40, 10);

    let at = Position::new(0, 0);
    let message = owner.on_slot_event(LibrarySlotEvent::List(RowLocalInput::Click(at)));
    assert!(matches!(message, None), "a plain click emits no Msg");
    assert_eq!(owner.inline_search().cursor(), 0);

    let message = owner.on_slot_event(LibrarySlotEvent::List(RowLocalInput::Wheel {
        at,
        delta: 1,
    }));
    assert!(matches!(message, Some(Msg::TerminalEvent(_))));
    assert_eq!(owner.inline_search().cursor(), 1);

    let message = owner.on_slot_event(LibrarySlotEvent::List(RowLocalInput::DoubleClick(at)));
    assert_eq!(
        message,
        Some(Msg::Shell(ShellRequest::InlineSearchActivate {
            id: "id".into(),
            item_type: "Movie".into(),
        })),
        "double-click activates the row it selected"
    );

    let message = owner.on_slot_event(LibrarySlotEvent::List(RowLocalInput::ContextClick(at)));
    assert!(
        matches!(
            message,
            Some(Msg::Shell(ShellRequest::BrowserContextMenu { ref item }))
                if item.name == "Search Result Alpha"
        ),
        "a right click on a result row opens its context menu: {message:?}"
    );
}
