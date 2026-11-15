//! The movement vocabulary and the pane routing the Library's key routers share
//! (`add-gui-library-panel` design D3, row 7C.2, the port of
//! `lib/library/keys-shared.ts`): the keys the Library keeps from the core's
//! router, the list movement each cursor key names, and the one key handler the
//! Feeds, Podcasts, Emby, and Audiobooks destinations run over their own facts —
//! the frame, the rows, the pill row, and the item commands. [`super::keys`]
//! routes the vocabulary for Music and Home and `shows-keys` for Shows, so one
//! table serves both and they cannot drift; [`route_panel_list_key`] serves the
//! four flat destinations, which copy nothing but their own extras — Emby's
//! play-next, the Audiobooks chapter Workspace.

use crate::bridge::keys::DomKey;
use crate::window::input::KeyOutcome;
use dioxus_native::prelude::*;

use super::list::ListMove;
use super::state::{LibraryPane, LibraryState};

/// The keys the Library keeps from the core's router (design D3, step 4): the
/// movements, Right and Left's open and close, and Enter. Each pane acts on the
/// ones it owns — the list's cursor, the Hero's tracklist or Workspace, or the
/// series list — and the others change nothing but still never reach the core.
/// `n` and `a` are the Library's too, and each destination's item key answers
/// them for every pane.
#[must_use]
pub fn is_list_and_hero_key(key: &str) -> bool {
    matches!(
        key,
        "Enter"
            | "ArrowUp"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "PageUp"
            | "PageDown"
            | "Home"
            | "End"
    )
}

/// The list movement each cursor key names (design D3, D7).
#[must_use]
pub fn cursor_move(key: &str) -> Option<ListMove> {
    match key {
        "ArrowUp" => Some(ListMove::Up),
        "ArrowDown" => Some(ListMove::Down),
        "PageUp" => Some(ListMove::PageUp),
        "PageDown" => Some(ListMove::PageDown),
        "Home" => Some(ListMove::Home),
        "End" => Some(ListMove::End),
        _ => None,
    }
}

/// Whether no modifier is held.
#[must_use]
pub fn bare(key: &DomKey) -> bool {
    !key.modifiers.ctrl_key && !key.meta_key && !key.modifiers.alt_key && !key.modifiers.shift_key
}

/// The pill-row facts a destination's keys read (design D6, D12): the pill
/// walk and the pill select the keys ask for. A destination with no pill row
/// leaves [`PanelKeys::pills`] unset on its [`PanelKeys`].
pub trait PillKeys {
    /// The carried content slice the pill row paints over.
    type Frame;
    /// The pill the row paints.
    type Pill: Clone;

    /// Whether the frame paints the pill row: the Feeds and Podcasts rows
    /// always, the Emby one in feed-group mode only.
    fn available(&self, frame: &Self::Frame) -> bool;

    /// The pill the pill row's focus holds, or `None` while no pill has it.
    fn focused(&self, frame: &Self::Frame) -> Option<Self::Pill>;

    /// Ask for a pill to be selected: Space and Enter request it.
    fn request(&self, pill: Self::Pill);

    /// Move the focused pill one pill along the row.
    fn move_focus(&self, step: i32);
}

/// The destination facts the shared panel key handler routes over (row 7C.2):
/// the carried frame, the list's cursor row, the item the play and queue
/// commands name, and the Hero a destination paints. Each destination's keys
/// file is this one table plus its extras.
pub trait PanelKeys {
    /// The carried content slice, or `None` while another destination holds
    /// the window: a key routed then answers [`KeyOutcome::Core`].
    type Frame;
    /// The row the list's cursor rests on.
    type Item: Clone;
    /// The pill the row paints, `()` on a destination that paints none.
    type Pill: Clone;

    /// The carried content slice, or `None` while another destination holds
    /// the window: a key routed then answers `core`.
    fn frame(&self) -> Option<Self::Frame>;

    /// The pill row's facts; `None` on a destination that paints none.
    fn pills(&self) -> Option<&dyn PillKeys<Frame = Self::Frame, Pill = Self::Pill>> {
        None
    }

    /// Whether the frame paints a Hero pane.
    fn has_hero(&self, frame: &Self::Frame) -> bool;

    /// The item the frame's Hero paints: the core's selected one.
    fn selected_item_id(&self, frame: &Self::Frame) -> Option<String>;

    /// The row the list's cursor rests on, or `None` while no painted row
    /// carries it.
    fn cursor_row(&self) -> Option<Self::Item>;

    /// The item id the play and queue commands name.
    fn item_id(&self, item: &Self::Item) -> Option<String>;

    /// Move the list's cursor one movement along.
    fn move_list_cursor(&self, mv: ListMove);

    /// Play the item Enter or a double-click names.
    fn play_item(&self, item_id: &str);

    /// Add the item `a` names to the queue's tail.
    fn queue_item(&self, item_id: &str);

    /// Play the item `n` names next; `false` on a destination with no
    /// play-next, where `n` is consumed and does nothing.
    fn play_next_item(&self, item_id: &str) -> bool {
        let _ = item_id;
        false
    }

    /// The Hero pane's own router, on a destination whose Hero carries a
    /// control — the Audiobooks chapter Workspace. `None` routes the Hero's
    /// default: the movements and Enter change nothing but are consumed, and
    /// `n` and `a` act on the selected item.
    fn route_hero(&self, key: &DomKey, frame: &Self::Frame) -> Option<KeyOutcome> {
        let _ = (key, frame);
        None
    }
}

/// Route a key to a flat destination's keys (design D3, D12, row 7C.2): `/`
/// opens the list's filter, Space selects the focused pill and with no pill
/// focused stays the core's play and pause, the view pills walk and select,
/// the list's cursor moves and Enter plays the item it rests on, and the Hero
/// consumes the movements it has no cursor for — or routes them to the
/// destination's own Workspace. `a` queues the item the focused pane names,
/// and `n` plays it next where the destination has a play-next and is consumed
/// and does nothing where it has none. Every one of them is consumed, so no
/// Library key reaches the core's playback chords from any pane.
pub fn route_panel_list_key<K: PanelKeys>(
    key: &DomKey,
    state: &LibraryState,
    keys: &K,
) -> KeyOutcome {
    let ui = state.ui.peek().clone();
    let Some(frame) = keys.frame() else {
        return KeyOutcome::Core;
    };
    // `/` opens the list's filter (design D3, D7): the list narrows to the
    // titles that contain the text.
    if key.key == "/" && bare(key) {
        state.open_library_filter();
        return KeyOutcome::Handled;
    }
    // Space selects the focused pill (design D6). With no pill focused — and
    // on a destination with no pill row — it is the core's: Space plays and
    // pauses.
    if key.key == " " && !key.modifiers.ctrl_key && !key.meta_key {
        let pill = keys.pills().and_then(|pills| pills.focused(&frame));
        let Some(pill) = pill else {
            return KeyOutcome::Core;
        };
        if let Some(pills) = keys.pills() {
            pills.request(pill);
        }
        return KeyOutcome::Handled;
    }
    if !bare(key) {
        return KeyOutcome::Core;
    }
    if ui.pane == Some(LibraryPane::Pills)
        && keys.pills().is_some_and(|pills| pills.available(&frame))
    {
        let outcome = route_pill_key(key, &frame, keys);
        return or_remaining(outcome, key, keys);
    }
    if ui.pane == Some(LibraryPane::List) {
        return route_list_key(key, keys);
    }
    if ui.pane == Some(LibraryPane::Hero) && keys.has_hero(&frame) {
        let outcome = keys
            .route_hero(key, &frame)
            .unwrap_or_else(|| route_hero_key(key, &frame, keys));
        return or_remaining(outcome, key, keys);
    }
    // The Hero pane a frame without a Hero does not paint, and any pane that
    // left a key unclaimed, still keep the Library's keys from the core: the
    // movements and Enter change nothing, and `n` and `a` act on the list's
    // cursor row.
    route_remaining_key(key, keys)
}

/// The key's answer, or the remaining Library keys' answer when the pane left
/// it unclaimed: a pane that does not act on a key never passes it to the
/// core's playback chords (design D3, step 4).
fn or_remaining<K: PanelKeys>(outcome: KeyOutcome, key: &DomKey, keys: &K) -> KeyOutcome {
    match outcome {
        KeyOutcome::Core => route_remaining_key(key, keys),
        taken => taken,
    }
}

/// The Library's own keys no pane claims (design D3, step 4): `n` and `a` act
/// on the list's cursor row, as the Music router's item key acts from every
/// pane, and the movements and Enter change nothing at all. Every one of them
/// is consumed, so a playback chord never runs.
fn route_remaining_key<K: PanelKeys>(key: &DomKey, keys: &K) -> KeyOutcome {
    if key.key == "n" || key.key == "a" {
        let item = keys.cursor_row().and_then(|row| keys.item_id(&row));
        route_item_key(key, item.as_deref(), keys);
        return KeyOutcome::Handled;
    }
    if is_list_and_hero_key(&key.key) {
        KeyOutcome::Handled
    } else {
        KeyOutcome::Core
    }
}

/// Route a key to the pill row (design D12): Right and Left walk the pills,
/// the first of them with no pill focused taking the end it walks from, and
/// Enter selects the focused pill as Space does.
fn route_pill_key<K: PanelKeys>(key: &DomKey, frame: &K::Frame, keys: &K) -> KeyOutcome {
    let pills = keys.pills();
    let Some(pills) = pills else {
        return KeyOutcome::Core;
    };
    match key.key.as_str() {
        "ArrowRight" => {
            pills.move_focus(1);
            KeyOutcome::Handled
        }
        "ArrowLeft" => {
            pills.move_focus(-1);
            KeyOutcome::Handled
        }
        "Enter" => {
            if let Some(pill) = pills.focused(frame) {
                pills.request(pill);
            }
            KeyOutcome::Handled
        }
        _ => KeyOutcome::Core,
    }
}

/// Route a key to the list's cursor (design D12): the arrows, Page Up, Page
/// Down, Home, and End move the cursor, Enter plays the item it rests on, and
/// `n` and `a` play it next or queue it.
fn route_list_key<K: PanelKeys>(key: &DomKey, keys: &K) -> KeyOutcome {
    if let Some(mv) = cursor_move(&key.key) {
        keys.move_list_cursor(mv);
        return KeyOutcome::Handled;
    }
    if key.key == "Enter" {
        if let Some(row) = keys.cursor_row()
            && let Some(item_id) = keys.item_id(&row)
        {
            keys.play_item(&item_id);
        }
        return KeyOutcome::Handled;
    }
    if key.key == "n" || key.key == "a" {
        let item = keys.cursor_row().and_then(|row| keys.item_id(&row));
        route_item_key(key, item.as_deref(), keys);
        return KeyOutcome::Handled;
    }
    if is_list_and_hero_key(&key.key) {
        KeyOutcome::Handled
    } else {
        KeyOutcome::Core
    }
}

/// Route a key to the Hero (design D12): it carries no cursor of its own, so
/// the movements and Enter change nothing but are consumed, and `n` and `a`
/// act on the item the core selected — the one the Hero paints. A destination
/// whose Hero holds a control names its own router on [`PanelKeys::route_hero`].
fn route_hero_key<K: PanelKeys>(key: &DomKey, frame: &K::Frame, keys: &K) -> KeyOutcome {
    if key.key == "n" || key.key == "a" {
        route_item_key(key, keys.selected_item_id(frame).as_deref(), keys);
        return KeyOutcome::Handled;
    }
    if is_list_and_hero_key(&key.key) {
        KeyOutcome::Handled
    } else {
        KeyOutcome::Core
    }
}

/// Route `n` and `a` to one item (design D12): the list's cursor item, or the
/// one the Hero paints. `n` plays next where the destination has a play-next
/// and sends nothing where it has none; `a` sends the queue command. No item
/// sends nothing.
fn route_item_key<K: PanelKeys>(key: &DomKey, item_id: Option<&str>, keys: &K) {
    let Some(item_id) = item_id else {
        return;
    };
    if key.key == "n" {
        keys.play_next_item(item_id);
        return;
    }
    keys.queue_item(item_id);
}
