//! The Library panel's key routing (`add-gui-library-panel` design D3, the
//! port of `lib/library/keys.ts`): the router the window content installs in
//! [`crate::window::input`], where it runs before the forwarder and decides
//! whether the core ever sees a keydown. The order is the design's: an open
//! core overlay or an armed prefix keeps the keyboard with the core, an open
//! text field of the panel types, Esc closes what the window opened, `1`–`7`
//! open a destination, and the Library keys run while the pointer rests over
//! the Library column. Every other key goes to the core.

use crate::bridge::frame::library::{GuiDestinationList, GuiSearchFrame};
use crate::bridge::keys::{DomKey, KeyTarget};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::input::KeyOutcome;
use crate::window::pointer_target::{PointerList, PointerTarget, PointerZone};
use crate::window::sections::Sections;
use dioxus_native::prelude::*;

use super::keys_shared::{bare, core_target, cursor_move, is_list_and_hero_key};
use super::state::LibraryState;
use crate::window::arrangement::GuiArrangement;
use crate::window::settings::keys::route_settings_key;
use crate::window::settings::state::SettingsState;

/// The place of the destination tab a digit opens (design D3, step 3): `1`
/// opens the first tab after Home, so the digit is the tab's place in the
/// carried list rather than a tab position.
fn destination_place(key: &DomKey) -> Option<usize> {
    if !bare(key) || key.key.len() != 1 {
        return None;
    }
    let place: usize = key.key.parse().ok()?;
    (1..=7).contains(&place).then_some(place)
}

/// Close the active destination's filter with Esc (design D3, step 2): the
/// open field is the one list's own, so its close and accept run the
/// destination's own resolve, in the same order `routePanelKey` hands keys
/// over.
fn close_active_filter(state: &LibraryState) {
    let list = state.sections.library_list.peek();
    match list.as_ref().map(|list| &list.destination) {
        Some(GuiDestinationList::Home(_)) => state.close_home_filter(),
        Some(GuiDestinationList::Emby(_)) => state.close_emby_filter(),
        Some(GuiDestinationList::Shows(_)) => state.close_shows_filter(),
        Some(GuiDestinationList::Audiobooks(_)) => state.close_audiobooks_filter(),
        Some(GuiDestinationList::Podcasts(_)) => state.close_podcasts_filter(),
        Some(GuiDestinationList::Feeds(_)) => state.close_feeds_filter(),
        _ => state.close_library_filter(),
    }
}

/// Accept the active destination's filter with Enter (design D3, step 2):
/// the field closes and the list keeps its text.
fn accept_active_filter(state: &LibraryState) {
    let list = state.sections.library_list.peek();
    match list.as_ref().map(|list| &list.destination) {
        Some(
            GuiDestinationList::Home(_)
            | GuiDestinationList::Emby(_)
            | GuiDestinationList::Shows(_)
            | GuiDestinationList::Audiobooks(_)
            | GuiDestinationList::Podcasts(_)
            | GuiDestinationList::Feeds(_),
        ) => state.accept_destination_filter(),
        _ => state.accept_library_filter(),
    }
}

/// Route a key to the Home keys (design D12): the Home cursor owns the
/// movement keys, Enter plays the row it rests on, and `n` and `a` enqueue from
/// it. There is no filter and no pill row on Home, so `/` and Space are not
/// taken: they pass to the core's router. The tree's Right and Left have no
/// node to open, so they change nothing but still never reach the core.
fn route_home_panel_key(state: &LibraryState, key: &DomKey, target: KeyTarget) -> KeyOutcome {
    if key.key == "/" && bare(key) {
        state.open_library_filter();
        return KeyOutcome::Handled;
    }
    if key.key == " " {
        // No pill row on Home (row 6.2): Space keeps its core effect, as it
        // does under the Queue.
        return KeyOutcome::Core(target);
    }
    if !bare(key) {
        return KeyOutcome::Core(target);
    }
    if let Some(mv) = cursor_move(&key.key) {
        state.move_home_list_cursor(mv);
        return KeyOutcome::Handled;
    }
    match key.key.as_str() {
        "Enter" => {
            if let Some(row) = state.home_cursor_row_id() {
                state.play_home_row(row);
            }
            KeyOutcome::Handled
        }
        "n" => {
            if let Some(row) = state.home_cursor_row_id() {
                state.play_home_row_next(row);
            }
            KeyOutcome::Handled
        }
        "a" => {
            if let Some(row) = state.home_cursor_row_id() {
                state.queue_home_row(row);
            }
            KeyOutcome::Handled
        }
        _ if is_list_and_hero_key(&key.key) => {
            // Right and Left have no tree node to open on Home; the other
            // keys are answered above. They still never reach the core
            // (design D3, step 4).
            KeyOutcome::Handled
        }
        _ => KeyOutcome::Core(target),
    }
}

/// Route a key while the filter field holds the keyboard (design D3, step 2).
fn route_filter_key(state: &LibraryState, key: &DomKey) -> KeyOutcome {
    match key.key.as_str() {
        "Escape" => {
            close_active_filter(state);
            KeyOutcome::Handled
        }
        "Enter" => {
            accept_active_filter(state);
            KeyOutcome::Handled
        }
        // The field takes the rest: the browser types the key, and the core
        // gets none.
        _ => KeyOutcome::Text,
    }
}

/// The item the pointer's control names (design D2): the item of the row the
/// pointer rests on — the Hero's tracklist track while the tracklist carries
/// it, and the Music list's cursor row anywhere else. With no item the key that
/// acts on it sends no command.
#[must_use]
fn hovered_item_id(state: &LibraryState, pointer: &PointerTarget) -> Option<String> {
    if pointer.list == Some(PointerList::Tracklist) {
        return state.tracklist_item_id();
    }
    state.list_cursor_item_id()
}

/// Play the item the pointer's control names (design D2's Enter): the list row,
/// or the Hero's tracklist track while the pointer rests on the tracklist. With
/// no item the key sends no command. Enter never picks: a mouse press picks the
/// item for the Hero, and Enter plays.
fn play_hovered_item(state: &LibraryState, pointer: &PointerTarget) {
    if let Some(item_id) = hovered_item_id(state, pointer) {
        state.link.library(GuiLibraryCommand::Play { item_id });
    }
}

/// Route a movement or open key to the Music list (design D2, D7): the arrows,
/// Page Up, Page Down, Home, and End move the list, Right opens the cursor's
/// node, Left closes it, and Enter plays the item under it.
fn route_list_key(state: &LibraryState, key: &DomKey, pointer: &PointerTarget) -> bool {
    if let Some(mv) = cursor_move(&key.key) {
        state.move_library_cursor(mv);
        return true;
    }
    match key.key.as_str() {
        "ArrowRight" => {
            state.set_library_cursor_open(true);
            true
        }
        "ArrowLeft" => {
            state.set_library_cursor_open(false);
            true
        }
        "Enter" => {
            play_hovered_item(state, pointer);
            true
        }
        _ => false,
    }
}

/// Route a movement or Enter key to the Hero's tracklist (design D2): the
/// movement keys move the tracklist's cursor, and Enter plays the track it
/// rests on.
fn route_tracklist_key(state: &LibraryState, key: &DomKey, pointer: &PointerTarget) -> bool {
    if let Some(mv) = cursor_move(&key.key) {
        state.move_library_track_cursor(mv);
        return true;
    }
    if key.key == "Enter" {
        play_hovered_item(state, pointer);
        return true;
    }
    false
}

/// Route `n` and `a` to the item the pointer's control names (design D2, D8):
/// play next and add to queue. The Library takes both keys while the pointer
/// rests over the Library column, so the core's `a` chord never runs from here;
/// with no item the key sends no command.
fn route_item_key(state: &LibraryState, key: &DomKey, pointer: &PointerTarget) {
    let Some(item_id) = hovered_item_id(state, pointer) else {
        return;
    };
    match key.key.as_str() {
        "n" => state.link.library(GuiLibraryCommand::PlayNext { item_id }),
        _ => state
            .link
            .library(GuiLibraryCommand::AddToQueue { item_id }),
    }
}

/// Close the window's open menu or overlay with Esc (design D2, step 1): the
/// right-click item menu and the picked item's menu first, then the Narrow
/// arrangement's Hero overlay, then the carried Search section — wherever the
/// pointer is. Whether one paints decides the answer, so an Esc with nothing
/// open may not leave the core's Esc unclaimed. The carried Search section's
/// close is the one close the open results have (`search.ts`, row 6.8).
fn route_escape_key(
    state: &LibraryState,
    ui: &super::state::LibraryUiState,
    search: Option<&GuiSearchFrame>,
    arrangement: GuiArrangement,
    target: KeyTarget,
) -> KeyOutcome {
    if ui.row_menu.is_some() {
        state.close_library_row_menu();
        return KeyOutcome::Handled;
    }
    if ui.item_menu_open {
        state.close_library_item_menu();
        return KeyOutcome::Handled;
    }
    if state.hero_overlay_open(ui, arrangement) {
        state.close_library_hero();
        return KeyOutcome::Handled;
    }
    // The carried Search section (design D12, row 12.1): the dropdown paints
    // while the frame carries it, so Esc closes it even after a press took
    // the keyboard elsewhere — it is the only close the open results have.
    if search.is_some() {
        state.close_library_search();
        return KeyOutcome::Handled;
    }
    KeyOutcome::Core(target)
}

/// Route a key while the Search field holds the keyboard (design D3, step 2,
/// row 12.1): Esc closes the field and the results, the arrows move the
/// highlighted result, Enter activates it, and every other key is the field's
/// typing — the `Text` answer lets the focused input take the key and sends
/// nothing to the core. With no results held, the arrows and Enter still
/// never reach the core: they would move the caret and submit nothing, and
/// the core gets none.
fn route_search_key(
    state: &LibraryState,
    key: &DomKey,
    search: Option<&GuiSearchFrame>,
) -> KeyOutcome {
    match key.key.as_str() {
        "Escape" => {
            state.close_library_search();
            KeyOutcome::Handled
        }
        "ArrowDown" => {
            state.move_search_highlight(1, search);
            KeyOutcome::Handled
        }
        "ArrowUp" => {
            state.move_search_highlight(-1, search);
            KeyOutcome::Handled
        }
        "Enter" => {
            state.activate_search_highlight(search);
            KeyOutcome::Handled
        }
        // The field takes the rest: the browser types the key, and the core
        // gets none.
        _ => KeyOutcome::Text,
    }
}

/// Route a key to the active destination's keys (design D2, step 3): the
/// control the pointer marks acts on it. On Home the Home keys route (design
/// D12), on a plain Emby library the Emby keys (row 8.1), and the Shows,
/// Audiobooks, Podcasts, and Feeds keys route with their ports (rows 6.5 to
/// 6.7). The Settings placeholder remains (row 8.1); a key under that
/// destination goes to the core's router.
fn route_panel_key(
    state: &LibraryState,
    key: &DomKey,
    sections: &Sections,
    pointer: &PointerTarget,
) -> KeyOutcome {
    let list = sections.library_list.peek();
    match list.as_ref().map(|list| &list.destination) {
        Some(GuiDestinationList::Home(_)) => route_home_panel_key(state, key, core_target(pointer)),
        Some(GuiDestinationList::Emby(_)) => {
            super::emby_keys::route_emby_panel_key(state, key, pointer)
        }
        Some(GuiDestinationList::Shows(_)) => {
            super::shows_keys::route_shows_panel_key(state, key, pointer)
        }
        Some(GuiDestinationList::Audiobooks(_)) => {
            super::audiobooks_keys::route_audiobooks_panel_key(state, key, pointer)
        }
        Some(GuiDestinationList::Podcasts(_)) => {
            super::podcasts_keys::route_podcasts_panel_key(state, key, pointer)
        }
        Some(GuiDestinationList::Feeds(_)) => {
            super::feeds_keys::route_feeds_panel_key(state, key, pointer)
        }
        Some(GuiDestinationList::Music(_)) => route_music_panel_key(state, key, pointer),
        // No carried Library list paints no Library keys: the core takes
        // every one of them (design D2, D10).
        None => KeyOutcome::Core(core_target(pointer)),
    }
}

/// The Music destination's own keys (design D2, D12), over the carried Music
/// content.
fn route_music_panel_key(
    state: &LibraryState,
    key: &DomKey,
    pointer: &PointerTarget,
) -> KeyOutcome {
    if key.key == "/" && bare(key) {
        state.open_library_filter();
        return KeyOutcome::Handled;
    }
    // Space stays the core's play and pause (design D2, step 4): the pill row
    // holds no keyboard focus, so the window never takes it.
    if key.key == " " {
        return KeyOutcome::Core(core_target(pointer));
    }
    if !bare(key) {
        return KeyOutcome::Core(core_target(pointer));
    }
    if pointer.list == Some(PointerList::Tracklist) {
        if route_tracklist_key(state, key, pointer) {
            return KeyOutcome::Handled;
        }
    } else if route_list_key(state, key, pointer) {
        return KeyOutcome::Handled;
    }
    if key.key == "n" || key.key == "a" {
        route_item_key(state, key, pointer);
        return KeyOutcome::Handled;
    }
    if is_list_and_hero_key(&key.key) {
        return KeyOutcome::Handled;
    }
    // Nothing else of the Library's answers this key: the core's router does.
    KeyOutcome::Core(core_target(pointer))
}

/// Route one keydown (design D2): the window does its part, and the answer is
/// what the input forwarder must do with the key. The section facts the routing
/// reads come from the window's own section stores (`gui-change-driven-sections`
/// design D6), read through `peek` — a keydown copies none of their values. The
/// `arrangement` is the window's own value, the same one the Library panel
/// paints (design D4): the keys close the Narrow Hero overlay on the fact the
/// panel paints, never on a report that may not have landed yet.
///
/// `pointer` is the pointer target the launch copy resolved (design D1 of
/// `gui-pointer-target-interaction`), read at key time: every core answer names
/// the panel the pointer is over, and the Library keys run only over the
/// Library column, so the core needs no stored Panel focus of its own (design
/// D2).
#[must_use]
pub fn route_library_key(
    key: &DomKey,
    sections: Option<&Sections>,
    state: &LibraryState,
    settings: Option<&SettingsState>,
    arrangement: GuiArrangement,
    pointer: &PointerTarget,
) -> KeyOutcome {
    let target = core_target(pointer);
    let Some(sections) = sections else {
        return KeyOutcome::Core(target);
    };
    // (1) An open core overlay or an armed prefix keeps the keyboard with the
    // core: both are section facts, and the core's own router answers them.
    if sections.overlay.peek().is_some() || sections.status.peek().prefix_armed {
        return KeyOutcome::Core(target);
    }
    // The Queue filter's text entry is the core's, so while it holds the
    // keyboard every key is the core's to type (`text_entry_focused` in
    // `src/app/input/gui.rs`). It is the Queue's wherever the pointer is: the
    // keys the filter does not consume reach the Queue leaf (design D2).
    if sections.summary.peek().filter_active {
        return KeyOutcome::Core(KeyTarget::Queue);
    }
    // The Settings screen takes every key the overlay step leaves while its
    // frame section is present (add-gui-settings-screen design D11): the
    // window paints the screen in place of the two panels then, so no Library
    // step runs. The Settings steps route ahead of the Library ones (row
    // 8.1); a harness that carries no Settings store answers the core.
    if let Some(frame) = sections.settings.peek().as_ref() {
        return match settings {
            Some(settings) => route_settings_key(key, frame, settings, target),
            None => KeyOutcome::Core(target),
        };
    }
    // The attach states carry no slice, and no Library panel means no Library
    // keys: the core takes every one of them. The panel's presence is one fact
    // for the window and the router (design D2, D10): sections that carry no
    // Library list take no Library key, even while the window still holds the
    // content of an earlier frame. The content names the active destination's
    // section (design D12).
    if !LibraryState::library_shown(sections.library_list.peek().as_deref()) {
        return KeyOutcome::Core(target);
    }
    // The `Ctrl+K` chord (design D3, step 3, row 12.1): the Search field takes
    // the keyboard wherever the Library panel shows, before the open-field
    // checks — even while the filter holds it, where the DOM focus move closes
    // the filter the way any other focus move does. The chord arms the field
    // only while a top bar mounts it: in Mini the panel paints no top bar, and
    // arming a field that holds no element leaves the keyboard dead on its
    // fact, so the chord keeps its core effect there instead.
    if key.modifiers.ctrl_key
        && !key.meta_key
        && !key.modifiers.alt_key
        && !key.modifiers.shift_key
        && matches!(key.key.as_str(), "k" | "K")
        && state.search_field_mounted()
    {
        state.focus_search_field();
        return KeyOutcome::Handled;
    }
    // (2) An open text field of the panel takes the keys. The Search field
    // routes first: its hold on the keyboard is DOM focus, which no panel key
    // runs while it types (the key-leak rule), and the field's typing is the
    // `Text` answer the forwarder lets through (design D3, step 2, row 12.1).
    // Both facts must hold: a stale armed fact an unmounted bar retracted must
    // never route a key under it (the same key-leak gate).
    if state.search_field_mounted() && state.search_field_focused() {
        return route_search_key(state, key, sections.search.peek().as_ref());
    }
    if state.ui.peek().filter_open {
        return route_filter_key(state, key);
    }
    // (3) Esc closes what the window opened, wherever the pointer is (design
    // D2, step 1). With nothing open it stays the core's key.
    if key.key == "Escape" && bare(key) {
        let ui = state.ui.peek().clone();
        return route_escape_key(
            state,
            &ui,
            sections.search.peek().as_ref(),
            arrangement,
            target,
        );
    }
    // (4) A digit opens the destination tab at its place (design D2, step 2).
    if let Some(place) = destination_place(key) {
        if let Some(tab) = sections
            .library_list
            .peek()
            .as_ref()
            .and_then(|list| list.tabs.get(place.wrapping_sub(1)))
        {
            state.open_destination(tab.position);
        }
        return KeyOutcome::Handled;
    }
    // (5) The Library keys run while the pointer rests over the Library column
    // (design D2, step 3). The carried Search section rides with them: its Esc
    // close needs it.
    if pointer.zone == Some(PointerZone::Library) {
        return route_panel_key(state, key, sections, pointer);
    }
    // (6) Every other key goes to the core's keyboard router.
    KeyOutcome::Core(target)
}

#[cfg(test)]
mod tests;
