//! The pill-list controller the Feeds, Podcasts, and Emby destinations share
//! (`add-gui-library-panel` row 7C.1, the port of `lib/library/pill-list.ts`):
//! the interaction state the window content owns and the core never reads —
//! the cursor, the pill row's focus, and the `/` filter's text — plus the
//! actions over it. The scroll offset and the measured viewport live in the
//! list's scroll store ([`super::scroll`], design D9), which the caller names.
//! The core owns the pill selection and the selected row (the frame's `pills`
//! and `selected`), so nothing here mirrors them: a press sends the select
//! command, and the frame carries the choice back. The panel's pane focus and
//! the Hero overlay stay in [`super::state`]'s shared store.
//!
//! Each destination's module keeps its own doc, its own command shapes, its
//! own names, and its own extras — Emby's letter rail and play-next — over
//! this one controller, so a filter change, a cursor move, a measure, a
//! scroll, and a pill walk resolve the same way on every one of them and the
//! destinations cannot drift. The operations run over the destination's
//! signals and painted rows, which the destination's module passes in; the
//! command shapes stay there too.

use dioxus_native::prelude::*;

use crate::window::library::list::{
    GeometryRow, ListMove, clamp_scroll_top, list_window, scroll_top_to_show_cursor,
    step_pill_focus,
};
use crate::window::library::scroll::ListScroll;

/// A pill-list destination's interaction state (design D12): what the window
/// content owns and the core never reads.
#[derive(Debug, Clone, PartialEq)]
pub struct PillListUiState<Pill, Focus = Pill> {
    /// The cursor's row id, or `None` while no row carries the cursor
    /// (design D12). The cursor moves over the item rows only.
    pub cursor: Option<String>,
    /// The pill the pill row's Left and Right move the focus to, or `None`
    /// while no pill has it (design D6, D12): the pill itself, or its stable
    /// key — the destination picks.
    pub focused_pill: Option<Focus>,
    /// The pill selection the keys asked for: Space and Enter select the
    /// focused pill, and the pill row turns it into the set-pill command.
    pub pill_select: Option<Pill>,
    /// The `/` filter's text: the list paints only the rows whose title
    /// contains it, without case (design D7's filter rule).
    pub filter: String,
}

impl<Pill, Focus> PillListUiState<Pill, Focus> {
    /// The state a freshly mounted pill list starts with.
    #[must_use]
    pub fn initial() -> Self {
        Self {
            cursor: None,
            focused_pill: None,
            pill_select: None,
            filter: String::new(),
        }
    }
}

/// Scroll the list so the cursor's row paints fully, in the scroll store
/// (design D12, D9).
pub fn follow_cursor<Row: GeometryRow>(
    scroll: &mut Signal<ListScroll>,
    rows: &[Row],
    cursor: Option<&str>,
) {
    let held = *scroll.peek();
    let shown = ListScroll {
        scroll_top: scroll_top_to_show_cursor(rows, cursor, held.scroll_top, held.viewport_height),
        viewport_height: held.viewport_height,
    };
    if shown != held {
        scroll.set(shown);
    }
}

/// Settle the cursor once the painted rows may have narrowed (design D12): the
/// cursor keeps its row while that row still paints, and falls to the first
/// painted row otherwise. The scroll store then shows the cursor — the Music
/// filter's own resolve (design D7). `move_from_start` is the destination's
/// cursor movement over its own rows, called with no cursor and the Home
/// movement.
pub fn settle_cursor<Row: GeometryRow, Pill: Clone + 'static, Focus: Clone + 'static>(
    ui: &mut Signal<PillListUiState<Pill, Focus>>,
    scroll: &mut Signal<ListScroll>,
    rows: &[Row],
    move_from_start: impl Fn(&[Row]) -> Option<String>,
) {
    let state = ui.peek().clone();
    let kept = state
        .cursor
        .as_deref()
        .is_some_and(|cursor| rows.iter().any(|row| row.id() == cursor));
    let cursor = if kept {
        state.cursor.clone()
    } else {
        move_from_start(rows)
    };
    if cursor != state.cursor {
        ui.with_mut(|current| current.cursor.clone_from(&cursor));
    }
    follow_cursor(scroll, rows, cursor.as_deref());
}

/// Move the list's cursor one movement along (design D12): Up and Down move a
/// row, Page Up and Page Down move the rows in view, and Home and End take the
/// list's ends. The scroll offset follows, so the cursor row stays fully
/// visible. `move_over` is the destination's cursor movement over its own
/// rows.
pub fn move_cursor<Row: GeometryRow, Pill: Clone + 'static, Focus: Clone + 'static>(
    ui: &mut Signal<PillListUiState<Pill, Focus>>,
    scroll: &mut Signal<ListScroll>,
    rows: &[Row],
    move_over: impl Fn(&[Row], Option<&str>, ListMove, usize) -> Option<String>,
    mv: ListMove,
) {
    let current = ui.peek().clone();
    let held = *scroll.peek();
    let view = list_window(rows, held.scroll_top, held.viewport_height);
    let cursor = move_over(rows, current.cursor.as_deref(), mv, view.end - view.start);
    if cursor == current.cursor {
        return;
    }
    ui.with_mut(|state| state.cursor.clone_from(&cursor));
    follow_cursor(scroll, rows, cursor.as_deref());
}

/// Report the list's measured viewport height (design D12): the rows in view
/// follow it, and the scroll offset stays inside the narrower range.
pub fn measure<Row: GeometryRow>(
    scroll: &mut Signal<ListScroll>,
    rows: &[Row],
    viewport_height: f32,
) {
    let held = *scroll.peek();
    let shown = ListScroll {
        viewport_height,
        scroll_top: clamp_scroll_top(rows, held.scroll_top, viewport_height),
    };
    if shown != held {
        scroll.set(shown);
    }
}

/// Report the list's native scroll offset (design D12): a user scroll moves
/// the rows in view, and the offset stays inside the list's range.
pub fn scroll<Row: GeometryRow>(scroll: &mut Signal<ListScroll>, rows: &[Row], scroll_top: f32) {
    let held = *scroll.peek();
    let shown = ListScroll {
        scroll_top: clamp_scroll_top(rows, scroll_top.max(0.0), held.viewport_height),
        viewport_height: held.viewport_height,
    };
    if shown != held {
        scroll.set(shown);
    }
}

/// The pill the pill row's focus holds, resolved against the frame's carried
/// pills so a stale focus names no carried pill: the pill the keys' Space and
/// Enter select (design D6, D12). `key_of` names a pill's stable focus key,
/// `focus_key` the stored focus's key — every carried frame is a fresh
/// deserialization, so the resolution runs over the keys, never an object
/// identity (row 10.1 review P2).
pub fn focused_pill<'a, Pill, Focus>(
    focused: Option<&Focus>,
    pills: &'a [Pill],
    key_of: impl Fn(&Pill) -> String,
    focus_key: impl Fn(&Focus) -> String,
) -> Option<&'a Pill> {
    let key = focus_key(focused?);
    pills.iter().find(|pill| key_of(pill) == key)
}

/// Move the focused pill one pill along the row (design D12): Right and Left
/// walk the pills, and the first of them with no pill focused takes the end it
/// walks from. `focus_of` is the focus value a pill leaves in the state.
pub fn move_pill_focus<Pill: Clone + 'static, Focus: Clone + 'static>(
    ui: &mut Signal<PillListUiState<Pill, Focus>>,
    pills: &[Pill],
    key_of: impl Fn(&Pill) -> String,
    focus_of: impl Fn(&Pill) -> Focus,
    focus_key: impl Fn(&Focus) -> String,
    step: i32,
) {
    let focused = ui.peek().focused_pill.clone();
    let focused_key = focused.as_ref().map(focus_key);
    let moved = step_pill_focus(pills, focused_key.as_deref(), key_of, step);
    if let Some(pill) = moved {
        ui.with_mut(|state| state.focused_pill = Some(focus_of(&pill)));
    }
}
