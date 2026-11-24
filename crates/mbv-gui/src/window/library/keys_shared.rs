//! The movement vocabulary and the list routing the Library's key routers share
//! (`add-gui-library-panel` design D3, row 7C.2, the port of
//! `lib/library/keys-shared.ts`): the keys the Library keeps from the core's
//! router, the list movement each movement key names, and the one key handler the
//! Feeds, Podcasts, Emby, Audiobooks, and Shows destinations run over their own
//! facts — the frame, the rows, and the item commands.
//! [`super::keys`] routes the vocabulary for Music and Home;
//! [`route_panel_list_key`] serves the other five destinations, which copy
//! nothing but their own extras — the Audiobooks chapter Workspace, and the
//! Shows tree's open, close, and toggle hooks and episode Workspace. The Hero's
//! own controls — the tracklist and the Workspaces — are
//! the lists the pointer marks, so the key under the pointer is the control the
//! key acts on (design D2 of `gui-pointer-target-interaction`).
//!
//! The keys act on the row the pointer marks, and their movement keys scroll
//! the destination's list (design D2, D6). Every destination routes that way,
//! so no list and no Workspace keeps a keyboard cursor.

use crate::bridge::keys::{DomKey, KeyTarget};
use crate::window::input::KeyOutcome;
use crate::window::pointer_target::{PointerList, PointerTarget, PointerZone};

use super::list::ListMove;
use super::state::LibraryState;

/// The panel a key's core answer names (design D2 of
/// `gui-pointer-target-interaction`): the Queue list box when the pointer is
/// over it, and no Queue target otherwise — the Library column, neither panel,
/// or outside the window.
#[must_use]
pub fn core_target(pointer: &PointerTarget) -> KeyTarget {
    match pointer.zone {
        Some(PointerZone::Queue) => KeyTarget::Queue,
        Some(PointerZone::Library) | None => KeyTarget::None,
    }
}

/// Whether the pointer rests on a Hero control rather than the destination's
/// list (design D2): the Hero's tracklist and its Workspaces carry list
/// markers of their own, so the pointer on one of them names the Hero's keys.
#[must_use]
pub fn pointer_on_hero(pointer: &PointerTarget) -> bool {
    matches!(
        pointer.list,
        Some(PointerList::Tracklist | PointerList::Workspace)
    )
}

/// The keys the Library keeps from the core's router (design D3, step 4, D2 of
/// `gui-pointer-target-interaction`): the movements, Right and Left's open and
/// close, and Enter. Each list acts on the ones it owns — the destination's
/// list under the pointer, the Hero's tracklist or Workspace, or the series
/// list — and the others change nothing but still never reach the core. `a` is
/// the Library's too, and each destination's item key answers it. `n` is not,
/// so it falls through to the core's router.
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

/// The list movement each movement key names (design D3, D7).
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

/// The destination facts the shared panel key handler routes over (row 7C.2):
/// the carried frame, the row the pointer marks, the item the play and queue
/// commands name, and the Hero a destination paints. Each destination's keys
/// file is this one table plus its extras.
pub trait PanelKeys {
    /// The carried content slice, or `None` while another destination holds
    /// the window: a key routed then answers [`KeyOutcome::Core`].
    type Frame;
    /// One painted row the keys may act on.
    type Item: Clone;

    /// The carried content slice, or `None` while another destination holds
    /// the window: a key routed then answers `core`.
    fn frame(&self) -> Option<Self::Frame>;

    /// Whether the frame paints a Hero pane.
    fn has_hero(&self, frame: &Self::Frame) -> bool;

    /// The item the frame's Hero paints: the core's selected one.
    fn selected_item_id(&self, frame: &Self::Frame) -> Option<String>;

    /// The row a list key acts on (design D1, D2): the row the pointer marks
    /// in this destination's list, resolved against the painted rows, so a
    /// stale or foreign id names no row. `None` while the pointer marks no row
    /// of it — another list, a Hero control, or no row at all.
    fn row(&self, pointer: &PointerTarget) -> Option<Self::Item>;

    /// The item id the play and queue commands name.
    fn item_id(&self, item: &Self::Item) -> Option<String>;

    /// Scroll the list one movement key along (design D6). `false` when the key
    /// is not this list's, because the pointer rests on another control: the
    /// router then sends it to the core.
    fn move_list(&self, mv: ListMove, pointer: &PointerTarget) -> bool;

    /// Play the item Enter or a double-click names.
    fn play_item(&self, item_id: &str);

    /// Right on `row`. A flat list has nothing to open, so the key is consumed
    /// and changes nothing; the Shows tree opens the node under the pointer.
    fn open_row(&self, row: &Self::Item) {
        let _ = row;
    }

    /// Left on `row`: the counterpart of [`PanelKeys::open_row`], a no-op on a
    /// flat list.
    fn close_row(&self, row: &Self::Item) {
        let _ = row;
    }

    /// Enter on `row`: play the item it names. The Shows tree overrides this,
    /// because Enter on a series or season node toggles it instead.
    fn enter_row(&self, row: &Self::Item) {
        if let Some(item_id) = self.item_id(row) {
            self.play_item(&item_id);
        }
    }

    /// Add the item `a` names to the queue's tail.
    fn queue_item(&self, item_id: &str);

    /// The Hero's own router, on a destination whose Hero carries a control —
    /// the Audiobooks chapter Workspace and the Shows episode Workspace. `None`
    /// routes the Hero's default: the movements and Enter change nothing but
    /// are consumed, and `a` acts on the selected item. A `Core` answer
    /// names `target`: the panel the key's pointer resolved (design D2 of
    /// `gui-pointer-target-interaction`).
    fn route_hero(
        &self,
        key: &DomKey,
        frame: &Self::Frame,
        pointer: &PointerTarget,
        target: KeyTarget,
    ) -> Option<KeyOutcome> {
        let _ = (key, frame, pointer, target);
        None
    }
}

/// Route a key to a flat destination's keys (design D3, D12, row 7C.2): `/`
/// opens the list's filter, Space stays the core's play and pause, and the
/// control the pointer marks takes the key. On a Hero control — the tracklist
/// or a Workspace — the destination's own Hero router runs, or the Hero's
/// default: the movements and Enter change nothing but are consumed, and `a`
/// acts on the item the core selected. Anywhere else Enter plays the row the
/// pointer marks, Right and Left act on that row through the destination's
/// hooks, and `a` queues it. Every one of them is consumed, so no Library key
/// reaches the core's playback chords from the list. `n` is not a Library key,
/// so it goes to the core.
pub fn route_panel_list_key<K: PanelKeys>(
    key: &DomKey,
    state: &LibraryState,
    keys: &K,
    pointer: &PointerTarget,
) -> KeyOutcome {
    let target = core_target(pointer);
    let Some(frame) = keys.frame() else {
        return KeyOutcome::Core(target);
    };
    // `/` opens the list's filter (design D3, D7): the list narrows to the
    // titles that contain the text.
    if key.key == "/" && bare(key) {
        state.open_library_filter();
        return KeyOutcome::Handled;
    }
    // Space stays the core's play and pause (design D2, step 4): the pill rows
    // hold no keyboard focus, so the window never takes it.
    if key.key == " " {
        return KeyOutcome::Core(target);
    }
    if !bare(key) {
        return KeyOutcome::Core(target);
    }
    if pointer_on_hero(pointer) && keys.has_hero(&frame) {
        let outcome = keys
            .route_hero(key, &frame, pointer, target)
            .unwrap_or_else(|| route_hero_key(key, &frame, keys, target));
        return or_remaining(outcome, key, keys, pointer, target);
    }
    route_list_key(key, keys, pointer, target)
}

/// The key's answer, or the remaining Library keys' answer when the control
/// left it unclaimed: a control that does not act on a key never passes it to
/// the core's playback chords (design D3, step 4).
fn or_remaining<K: PanelKeys>(
    outcome: KeyOutcome,
    key: &DomKey,
    keys: &K,
    pointer: &PointerTarget,
    target: KeyTarget,
) -> KeyOutcome {
    match outcome {
        KeyOutcome::Core(_) => route_remaining_key(key, keys, pointer, target),
        taken => taken,
    }
}

/// The Library's own keys no control claims (design D3, step 4): `a` acts on
/// the row the pointer marks, as the Music router's item key acts, and the
/// movements and Enter change nothing at all. Every one of them is consumed, so
/// a playback chord never runs.
fn route_remaining_key<K: PanelKeys>(
    key: &DomKey,
    keys: &K,
    pointer: &PointerTarget,
    target: KeyTarget,
) -> KeyOutcome {
    if key.key == "a" {
        let item = keys.row(pointer).and_then(|row| keys.item_id(&row));
        route_item_key(item.as_deref(), keys);
        return KeyOutcome::Handled;
    }
    if is_list_and_hero_key(&key.key) {
        KeyOutcome::Handled
    } else {
        KeyOutcome::Core(target)
    }
}

/// Route a key to the list (design D12, D2 of `gui-pointer-target-interaction`):
/// the arrows, Page Up, Page Down, Home, and End move the list on, Right, Left,
/// and Enter act on the row the pointer marks through the destination's hooks
/// (a tree opens, closes, or toggles a node; a flat list plays the item), and
/// `a` queues it. With no row the keys are consumed and do nothing, and a
/// movement key over another control is the core's.
fn route_list_key<K: PanelKeys>(
    key: &DomKey,
    keys: &K,
    pointer: &PointerTarget,
    target: KeyTarget,
) -> KeyOutcome {
    if let Some(mv) = cursor_move(&key.key) {
        if !keys.move_list(mv, pointer) {
            return KeyOutcome::Core(target);
        }
        return KeyOutcome::Handled;
    }
    let row = keys.row(pointer);
    match key.key.as_str() {
        "ArrowRight" => {
            if let Some(row) = row.as_ref() {
                keys.open_row(row);
            }
            return KeyOutcome::Handled;
        }
        "ArrowLeft" => {
            if let Some(row) = row.as_ref() {
                keys.close_row(row);
            }
            return KeyOutcome::Handled;
        }
        "Enter" => {
            if let Some(row) = row.as_ref() {
                keys.enter_row(row);
            }
            return KeyOutcome::Handled;
        }
        _ => {}
    }
    if key.key == "a" {
        let item = row.as_ref().and_then(|row| keys.item_id(row));
        route_item_key(item.as_deref(), keys);
        return KeyOutcome::Handled;
    }
    if is_list_and_hero_key(&key.key) {
        KeyOutcome::Handled
    } else {
        KeyOutcome::Core(target)
    }
}

/// Route a key to the Hero (design D12): it carries no cursor of its own, so
/// the movements and Enter change nothing but are consumed, and `a` acts on the
/// item the core selected — the one the Hero paints. A destination whose Hero
/// holds a control names its own router on [`PanelKeys::route_hero`].
fn route_hero_key<K: PanelKeys>(
    key: &DomKey,
    frame: &K::Frame,
    keys: &K,
    target: KeyTarget,
) -> KeyOutcome {
    if key.key == "a" {
        route_item_key(keys.selected_item_id(frame).as_deref(), keys);
        return KeyOutcome::Handled;
    }
    if is_list_and_hero_key(&key.key) {
        KeyOutcome::Handled
    } else {
        KeyOutcome::Core(target)
    }
}

/// Route `a` to one item (design D12): the row the pointer marks, or the one
/// the Hero paints. No item sends nothing.
fn route_item_key<K: PanelKeys>(item_id: Option<&str>, keys: &K) {
    let Some(item_id) = item_id else {
        return;
    };
    keys.queue_item(item_id);
}

#[cfg(test)]
pub(crate) mod tests;
