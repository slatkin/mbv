//! The list controller every Library destination shares (`add-gui-library-panel`
//! row 7C.1, the port of `lib/library/pill-list.ts`, issue #1009): the
//! interaction state the window content owns and the core never reads — the
//! cursor and the `/` filter's text — plus the actions over it. The scroll
//! offset and the measured viewport live in the list's scroll store ([`super::scroll`], design D9), which the caller names. The
//! core owns the pill selection and the selected row (the frame's `pills` and
//! `selected`), so nothing here mirrors them: a press sends the select
//! command, and the frame carries the choice back. The Hero overlay stays in
//! [`super::state`]'s shared store.
//!
//! Each destination's module keeps its own doc, its own command shapes, its
//! own names, and its own extras — Emby's letter rail and play-next, Home's
//! select-on-settle — over this one controller, so a filter change, a cursor
//! move, a measure, and a scroll resolve the same way on every one of them and
//! the destinations cannot drift. A destination names its UI
//! signal, its scroll store, its painted rows, and its cursor movement in a
//! [`ListControl`]; the command shapes stay in its module.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::window::library::list::{
    GeometryRow, ListMove, clamp_scroll_top, list_window, scroll_top_to_show_cursor,
};
use crate::window::library::scroll::ListScroll;

use super::state::{LibraryState, LibraryUiState};

/// A destination list's interaction state that the shared controller reads
/// and writes: the cursor and the `/` filter's text (design D12).
pub trait ListUi: Clone + 'static {
    /// The cursor's row id, or `None` while no row carries the cursor.
    fn cursor(&self) -> Option<&str>;
    /// Set the cursor's row id.
    fn set_cursor(&mut self, cursor: Option<String>);
    /// Set the `/` filter's text.
    fn set_filter(&mut self, filter: String);
}

/// A pill-list destination's interaction state (design D12): what the window
/// content owns and the core never reads.
#[derive(Debug, Clone, PartialEq)]
pub struct PillListUiState<Pill> {
    /// The cursor's row id, or `None` while no row carries the cursor
    /// (design D12). The cursor moves over the item rows only.
    pub cursor: Option<String>,
    /// The pill selection a press asked for, and the pill row turns it into
    /// the set-pill command.
    pub pill_select: Option<Pill>,
    /// The `/` filter's text: the list paints only the rows whose title
    /// contains it, without case (design D7's filter rule).
    pub filter: String,
}

impl<Pill> PillListUiState<Pill> {
    /// The state a freshly mounted pill list starts with.
    #[must_use]
    pub fn initial() -> Self {
        Self {
            cursor: None,
            pill_select: None,
            filter: String::new(),
        }
    }
}

impl<Pill: Clone + 'static> ListUi for PillListUiState<Pill> {
    fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    fn set_cursor(&mut self, cursor: Option<String>) {
        self.cursor = cursor;
    }

    fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }
}

/// A destination's cursor movement over its own rows: the rows, the cursor,
/// the movement, and the rows in view give the cursor's new row id.
pub(super) type CursorMove<Row> = fn(&[Row], Option<&str>, ListMove, usize) -> Option<String>;

/// One destination list under the shared controller: its interaction state,
/// its scroll store, its painted rows (read fresh, because a filter change
/// narrows them), and its cursor movement.
pub(super) struct ListControl<Ui: 'static, Row, Rows> {
    /// The destination's interaction state.
    pub ui: Signal<Ui>,
    /// The destination's scroll store.
    pub scroll: Signal<ListScroll>,
    /// The destination's painted rows under the current interaction state.
    pub rows: Rows,
    /// The destination's cursor movement.
    pub move_over: CursorMove<Row>,
}

impl<Ui, Row, Rows> ListControl<Ui, Row, Rows>
where
    Ui: ListUi,
    Row: GeometryRow,
    Rows: Fn() -> Arc<[Row]>,
{
    /// Scroll the list so the cursor's row paints fully, in the scroll store
    /// (design D12, D9).
    fn follow_cursor(&self, rows: &[Row], cursor: Option<&str>) {
        let held = *self.scroll.peek();
        let shown = ListScroll {
            scroll_top: scroll_top_to_show_cursor(
                rows,
                cursor,
                held.scroll_top,
                held.viewport_height,
            ),
            viewport_height: held.viewport_height,
        };
        if shown != held {
            let mut scroll = self.scroll;
            scroll.set(shown);
        }
    }

    /// Settle the cursor once the painted rows may have narrowed (design
    /// D12): the cursor keeps its row while that row still paints, and falls
    /// to the first painted row otherwise. The scroll store then shows the
    /// cursor — the Music filter's own resolve (design D7).
    pub fn settle_cursor(&self) {
        let held = self.ui.peek().cursor().map(str::to_owned);
        let rows = (self.rows)();
        let kept = held
            .as_deref()
            .is_some_and(|cursor| rows.iter().any(|row| row.id() == cursor));
        let cursor = if kept {
            held.clone()
        } else {
            (self.move_over)(&rows, None, ListMove::Home, 1)
        };
        if cursor != held {
            let mut ui = self.ui;
            ui.with_mut(|current| current.set_cursor(cursor.clone()));
        }
        self.follow_cursor(&rows, cursor.as_deref());
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule): the cursor follows the rows.
    pub fn set_filter(&self, filter: String) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.set_filter(filter));
        self.settle_cursor();
    }

    /// Close the filter with Esc: the text goes with it, so a closed filter
    /// narrows nothing (design D3, step 2). The cursor follows the widened
    /// rows. `panel` is the panel's shared store, which holds the field's
    /// open flag.
    pub fn close_filter(&self, panel: Signal<LibraryUiState>) {
        let mut panel = panel;
        panel.with_mut(|ui| ui.filter_open = false);
        self.set_filter(String::new());
    }

    /// Move the list's cursor one movement along (design D12): Up and Down
    /// move a row, Page Up and Page Down move the rows in view, and Home and
    /// End take the list's ends. The scroll offset follows, so the cursor row
    /// stays fully visible. Returns whether the cursor moved.
    pub fn move_cursor(&self, mv: ListMove) -> bool {
        let held = self.ui.peek().cursor().map(str::to_owned);
        let rows = (self.rows)();
        let scroll = *self.scroll.peek();
        let view = list_window(&rows, scroll.scroll_top, scroll.viewport_height);
        let cursor = (self.move_over)(&rows, held.as_deref(), mv, view.end - view.start);
        if cursor == held {
            return false;
        }
        let mut ui = self.ui;
        ui.with_mut(|current| current.set_cursor(cursor.clone()));
        self.follow_cursor(&rows, cursor.as_deref());
        true
    }

    /// Report the list's measured viewport height (design D12): the rows in
    /// view follow it, and the scroll offset stays inside the narrower range.
    pub fn measure(&self, viewport_height: f32) {
        let rows = (self.rows)();
        let held = *self.scroll.peek();
        let shown = ListScroll {
            viewport_height,
            scroll_top: clamp_scroll_top(&rows, held.scroll_top, viewport_height),
        };
        if shown != held {
            let mut scroll = self.scroll;
            scroll.set(shown);
        }
    }

    /// Report the list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_to(&self, scroll_top: f32) {
        let rows = (self.rows)();
        let held = *self.scroll.peek();
        let shown = ListScroll {
            scroll_top: clamp_scroll_top(&rows, scroll_top, held.viewport_height),
            viewport_height: held.viewport_height,
        };
        if shown != held {
            let mut scroll = self.scroll;
            scroll.set(shown);
        }
    }
}

impl LibraryState {
    /// Accept a destination list's filter with Enter: the field closes and
    /// the list keeps the text (design D3, step 2).
    pub fn accept_destination_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
    }
}
