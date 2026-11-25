//! The Library panel's key routing (`add-gui-library-panel` design D3, the
//! port of `lib/library/keys.ts`): the router the window content installs in
//! [`crate::window::input`], where it runs before the forwarder and decides
//! whether the core ever sees a keydown. The order is the design's: an open
//! core overlay or an armed prefix keeps the keyboard with the core, an open
//! text field of the panel types, Esc closes what the window opened, `1`–`7`
//! open a destination, and the Library keys run while the pointer rests over
//! the Library column. Every other key goes to the core.

use crate::bridge::frame::library::{GuiDestinationList, GuiHomeRowIdFrame, GuiSearchFrame};
use crate::bridge::keys::{DomKey, KeyTarget};
use crate::window::input::KeyOutcome;
use crate::window::pointer_target::{PointerList, PointerTarget, PointerZone};
use crate::window::sections::Sections;
use dioxus_native::prelude::*;

use super::keys_shared::{bare, core_target, cursor_move};
use super::lettered::{AudiobooksList, EmbyList, FeedsList, PodcastsList};
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
        Some(GuiDestinationList::Emby(_)) => state.close_lettered_filter::<EmbyList>(),
        Some(GuiDestinationList::Shows(_)) => state.close_shows_filter(),
        Some(GuiDestinationList::Audiobooks(_)) => state.close_lettered_filter::<AudiobooksList>(),
        Some(GuiDestinationList::Podcasts(_)) => state.close_lettered_filter::<PodcastsList>(),
        Some(GuiDestinationList::Feeds(_)) => state.close_lettered_filter::<FeedsList>(),
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

/// Whether a key is one the open collection dropdown's filter types (design
/// D7): one printable character, or Backspace. A chord key and every control
/// key stay the Library's and the core's, so the dropdown binds no key beyond
/// the filter's own.
fn types_into_filter(key: &DomKey) -> bool {
    key.key == "Backspace"
        || (!key.modifiers.ctrl_key
            && !key.meta_key
            && !key.modifiers.alt_key
            && key.key.chars().count() == 1)
}

/// Whether the open popover mounts a filter field: the collection dropdown
/// does, and a destination that carries no collection selector mounts none.
fn collections_filter_mounted(state: &LibraryState) -> bool {
    let list = state.sections.library_list.peek();
    list.as_ref()
        .is_some_and(|list| list.destination.collections().is_some())
}

/// Route a key while the collection dropdown is open (gui-collection-dropdown
/// design D7): Esc closes it and clears its filter text, and the filter takes
/// the typed keys and Backspace wherever the pointer is. Every other key
/// passes to the rest of the routing, so the dropdown binds nothing else. A
/// destination that mounts no filter field takes no typed key.
fn route_collections_open_key(state: &LibraryState, key: &DomKey) -> Option<KeyOutcome> {
    if key.key == "Escape" && bare(key) {
        state.close_collections_dropdown();
        return Some(KeyOutcome::Handled);
    }
    if !collections_filter_mounted(state) {
        return None;
    }
    types_into_filter(key).then_some(KeyOutcome::Text)
}

/// Route a key while the Shows view dropdown is open (gui-collection-dropdown
/// design D8, gtds design D15): Esc closes it. The popover mounts no filter
/// field, so every other key passes to the rest of the routing and stays the
/// panel's and the core's.
fn route_shows_view_open_key(state: &LibraryState, key: &DomKey) -> Option<KeyOutcome> {
    if key.key == "Escape" && bare(key) {
        state.close_shows_view_dropdown();
        return Some(KeyOutcome::Handled);
    }
    None
}

/// Route a key while the Shows Hero's season dropdown is open: Esc closes it
/// and clears its filter text, and its filter field, which always mounts with
/// the popover, takes the typed keys and Backspace wherever the pointer is.
fn route_seasons_open_key(state: &LibraryState, key: &DomKey) -> Option<KeyOutcome> {
    if key.key == "Escape" && bare(key) {
        state.close_shows_seasons_dropdown();
        return Some(KeyOutcome::Handled);
    }
    types_into_filter(key).then_some(KeyOutcome::Text)
}

/// Route a key while an open text field of the panel holds the keyboard (design
/// D3, step 2; gui-collection-dropdown design D7, D8): the list's `/` filter
/// takes every key, the collection dropdown's own filter takes the typed keys,
/// Backspace, and Esc, and the Shows view popover takes Esc — everywhere the
/// pointer is. `None` while no field or popover of the panel is open, so the
/// rest of the routing answers the key.
fn route_open_field_key(state: &LibraryState, key: &DomKey) -> Option<KeyOutcome> {
    // The facts are copied out of the store: a route may write it, and a held
    // read would meet that write.
    let (filter_open, collections_open, shows_view_open, seasons_open) = {
        let ui = state.ui.peek();
        (
            ui.filter_open,
            ui.collections.is_some(),
            ui.shows_view_open.is_open(),
            ui.shows_seasons.is_some(),
        )
    };
    if filter_open {
        return Some(route_filter_key(state, key));
    }
    if collections_open {
        return route_collections_open_key(state, key);
    }
    if shows_view_open {
        return route_shows_view_open_key(state, key);
    }
    if seasons_open {
        return route_seasons_open_key(state, key);
    }
    None
}

/// Route a key to the Home keys (design D12, D2 of
/// `gui-pointer-target-interaction`): `/` opens the list filter, the movement
/// keys scroll the list under the pointer, and Enter and `a` act on the hovered
/// row. With no hovered row they are consumed and do nothing. There is no
/// destination filter and no collection dropdown on Home, so Space passes to
/// the core's router.
fn route_home_panel_key(state: &LibraryState, key: &DomKey, pointer: &PointerTarget) -> KeyOutcome {
    let target = core_target(pointer);
    if key.key == "/" && bare(key) {
        state.open_library_filter();
        return KeyOutcome::Handled;
    }
    if key.key == " " {
        // No collection dropdown on Home (row 6.2): Space keeps its core
        // effect, as it does under the Queue.
        return KeyOutcome::Core(target);
    }
    if !bare(key) {
        return KeyOutcome::Core(target);
    }
    if let Some(mv) = cursor_move(&key.key) {
        // The scroll keys scroll the destination's list under the pointer
        // (design D2, step 3, D6). Anywhere else they are the core's keys.
        if pointer.list != Some(PointerList::Home) {
            return KeyOutcome::Core(target);
        }
        state.step_home_scroll(mv);
        return KeyOutcome::Handled;
    }
    match key.key.as_str() {
        "Enter" => {
            if let Some(row) = hovered_home_row(state, pointer) {
                state.play_home_row(row);
            }
            KeyOutcome::Handled
        }
        "a" => {
            if let Some(row) = hovered_home_row(state, pointer) {
                state.queue_home_row(row);
            }
            KeyOutcome::Handled
        }
        _ => KeyOutcome::Core(target),
    }
}

/// The Home row the pointer rests on (design D1, D2): the row its marker names,
/// resolved against the painted rows. `None` while the pointer is over no Home
/// row — over another list, over a heading, or over no row at all.
#[must_use]
fn hovered_home_row(state: &LibraryState, pointer: &PointerTarget) -> Option<GuiHomeRowIdFrame> {
    if pointer.list != Some(PointerList::Home) {
        return None;
    }
    state.home_row_id(pointer.row.as_deref()?)
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

/// The item the pointer's control names (design D2): the row the pointer rests
/// on, resolved against the list that paints it — the Hero's tracklist track
/// while the tracklist carries the marker, and the Music list's row anywhere
/// else. The marker carries the row's id, so each list's own row model resolves
/// it. With no row the key that acts on it sends no command.
#[must_use]
fn hovered_item_id(state: &LibraryState, pointer: &PointerTarget) -> Option<String> {
    let row = pointer.row.as_deref()?;
    match pointer.list? {
        PointerList::Music => state.music_row_item_id(row),
        PointerList::Tracklist => Some(row.to_owned()),
        PointerList::Home
        | PointerList::Shows
        | PointerList::Emby
        | PointerList::Audiobooks
        | PointerList::Podcasts
        | PointerList::Feeds
        | PointerList::Workspace => None,
    }
}

/// Play the item the pointer's control names (design D2's Enter): the list row,
/// or the Hero's tracklist track while the pointer rests on the tracklist. With
/// no item the key sends no command. Enter never picks: a mouse press picks the
/// item for the Hero, and Enter plays.
fn play_hovered_item(state: &LibraryState, pointer: &PointerTarget) {
    if let Some(item_id) = hovered_item_id(state, pointer) {
        state.play_library_item(&item_id);
    }
}

/// Route `a` to the item the pointer's control names (design D2, D8): add to
/// queue. The Library takes the key while the pointer rests over the Library
/// column, so the core's `a` chord never runs from here; with no item the key
/// sends no command.
fn route_item_key(state: &LibraryState, pointer: &PointerTarget) {
    let Some(item_id) = hovered_item_id(state, pointer) else {
        return;
    };
    state.queue_library_item(&item_id);
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
            // The collapsed control's panel closes with the search (user
            // request 2026-11-19): the guard inside makes it a no-op while
            // the field paints inline.
            state.close_search_panel();
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
        Some(GuiDestinationList::Home(_)) => route_home_panel_key(state, key, pointer),
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

/// The Music destination's own keys (design D2, D12): `/` opens the filter, the
/// movement keys scroll the list under the pointer, and Enter, `n`, and `a` act
/// on the hovered row of the list or the Hero's tracklist.
fn route_music_panel_key(
    state: &LibraryState,
    key: &DomKey,
    pointer: &PointerTarget,
) -> KeyOutcome {
    if key.key == "/" && bare(key) {
        state.open_library_filter();
        return KeyOutcome::Handled;
    }
    // Space stays the core's play and pause (design D2, step 4): the
    // collection dropdown's trigger holds no keyboard focus, so the window
    // never takes it.
    if key.key == " " {
        return KeyOutcome::Core(core_target(pointer));
    }
    if !bare(key) {
        return KeyOutcome::Core(core_target(pointer));
    }
    if let Some(mv) = cursor_move(&key.key) {
        // The scroll keys scroll the destination's list under the pointer
        // (design D2, step 3, D6). Anywhere else they are the core's keys.
        if pointer.list != Some(PointerList::Music) {
            return KeyOutcome::Core(core_target(pointer));
        }
        state.step_library_scroll(mv);
        return KeyOutcome::Handled;
    }
    match key.key.as_str() {
        "Enter" => {
            play_hovered_item(state, pointer);
            KeyOutcome::Handled
        }
        "a" => {
            route_item_key(state, pointer);
            KeyOutcome::Handled
        }
        // Nothing else of the Library's answers this key: the core's router
        // does (design D2, step 4).
        _ => KeyOutcome::Core(core_target(pointer)),
    }
}

/// A key typed in the Playlists screen's rename field: Esc cancels, Enter
/// sends the rename, and every other key types.
fn rename_field_key(key: &DomKey, settings: &SettingsState) -> KeyOutcome {
    if key.key == "Escape" {
        settings.cancel_settings_form();
        return KeyOutcome::Handled;
    }
    if key.key == "Enter" {
        settings.submit_rename_draft();
        return KeyOutcome::Handled;
    }
    KeyOutcome::Text
}

/// Route the `Ctrl+K` chord and the Search field's keyboard hold (design D3,
/// step 2 and 3, row 12.1): the Search field takes the keyboard wherever its
/// element paints, and the top bar's Search control paints at the window's top
/// edge in every screen but Settings — above the Queue, above the Library
/// panel, and above the Playlists screen. So this step runs before the
/// Playlists screen and the Library-presence gates of [`route_window_gate`]: a
/// focused field must type there too, not hand its keystrokes to the core's
/// keybinds. Only one element holds DOM focus, so the order against the field
/// the Panel routes is safe.
///
/// The chord arms the field only while a mounted element paints it: the control
/// mounts it while it paints the field — inline, or inside its collapsed panel
/// — so a collapsed-and-closed control holds no field, and the chord opens the
/// panel instead: the field the panel paints mounts and takes the keyboard the
/// next frame (user request 2026-11-19). It runs before the open-field checks —
/// even while the filter holds the keyboard, where the DOM focus move closes the
/// filter the way any other focus move does.
///
/// `None` while neither the chord nor a focused field answers the key, so the
/// rest of the routing does.
fn route_search_hold_key(
    key: &DomKey,
    sections: &Sections,
    state: &LibraryState,
) -> Option<KeyOutcome> {
    if key.modifiers.ctrl_key
        && !key.meta_key
        && !key.modifiers.alt_key
        && !key.modifiers.shift_key
        && matches!(key.key.as_str(), "k" | "K")
    {
        // The chord opens the collapsed panel first (user request 2026-11-19):
        // the field the panel paints mounts and takes the keyboard the next
        // frame. The call is unconditional — an open while the field paints
        // inline, or while the panel is already open, is `open_search_panel`'s
        // own no-op.
        state.open_search_panel();
        if state.search_field_mounted() {
            state.focus_search_field();
            return Some(KeyOutcome::Handled);
        }
    }
    // (2) The focused Search field takes the keys: its hold on the keyboard is
    // DOM focus, which no panel key runs while it types (the key-leak rule),
    // and the field's typing is the `Text` answer the forwarder lets through
    // (design D3, step 2, row 12.1). Both facts must hold: a stale armed fact
    // an unmounted control retracted must never route a key under it (the same
    // key-leak gate).
    if state.search_field_mounted() && state.search_field_focused() {
        return Some(route_search_key(
            state,
            key,
            sections.search.peek().as_deref(),
        ));
    }
    None
}

/// Route the keys no Library step may answer, in the design's order (design D2,
/// step 1): an open core overlay or an armed prefix, the Queue filter's text
/// entry, the Settings screen, the Search field's keyboard hold, the Playlists
/// screen, and a window that carries no Library panel. `None` when none of them
/// answers the key, so the Library keys run.
///
/// The section facts are the window's own section stores
/// (`gui-change-driven-sections` design D6), read through `peek` — a keydown
/// copies none of their values.
fn route_window_gate(
    key: &DomKey,
    sections: &Sections,
    state: &LibraryState,
    settings: Option<&SettingsState>,
    target: KeyTarget,
) -> Option<KeyOutcome> {
    // (1) An open core overlay or an armed prefix keeps the keyboard with the
    // core: both are section facts, and the core's own router answers them.
    if sections.overlay.peek().is_some() || sections.status.peek().prefix_armed {
        return Some(KeyOutcome::Core(target));
    }
    // The Queue filter's text entry is the core's, so while it holds the
    // keyboard every key is the core's to type (`text_entry_focused` in
    // `src/app/input/gui.rs`). It is the Queue's wherever the pointer is: the
    // keys the filter does not consume reach the Queue leaf (design D2).
    if sections.summary.peek().filter_active {
        return Some(KeyOutcome::Core(KeyTarget::Queue));
    }
    // The Settings screen takes every key the overlay step leaves while its
    // frame section is present (add-gui-settings-screen design D11): the
    // window paints the screen in place of the two panels then, so no Library
    // step runs. The Settings steps route ahead of the Library ones (row
    // 8.1); a harness that carries no Settings store answers the core.
    if let Some(frame) = sections.settings.peek().as_ref() {
        return Some(match settings {
            Some(settings) => route_settings_key(key, frame, settings, target),
            None => KeyOutcome::Core(target),
        });
    }
    if let Some(outcome) = route_search_hold_key(key, sections, state) {
        return Some(outcome);
    }
    // The Playlists screen takes the rename field's keys while it holds the
    // keyboard, and the core takes every other key: the Library panel is not
    // painted, so no Library step runs.
    if sections.playlists.peek().is_some() {
        if let Some(settings) = settings
            && settings.ui.peek().text_field.is_some()
        {
            return Some(rename_field_key(key, settings));
        }
        return Some(KeyOutcome::Core(target));
    }
    // The attach states carry no slice, and no Library panel means no Library
    // keys: the core takes every one of them. The panel's presence is one fact
    // for the window and the router (design D2, D10): sections that carry no
    // Library list take no Library key, even while the window still holds the
    // content of an earlier frame. The content names the active destination's
    // section (design D12).
    if !LibraryState::library_shown(sections.library_list.peek().as_deref()) {
        return Some(KeyOutcome::Core(target));
    }
    None
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
    // (1) The gates the window itself answers, in the design's order.
    if let Some(outcome) = route_window_gate(key, sections, state, settings, target) {
        return outcome;
    }
    // (2) An open text field of the panel takes the keys (design D3, step 2).
    if let Some(outcome) = route_open_field_key(state, key) {
        return outcome;
    }
    // (3) Esc closes what the window opened, wherever the pointer is (design
    // D2, step 1). With nothing open it stays the core's key.
    if key.key == "Escape" && bare(key) {
        let ui = state.ui.peek().clone();
        return route_escape_key(
            state,
            &ui,
            sections.search.peek().as_deref(),
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
