//! The Library panel's key routing (`add-gui-library-panel` design D3, the
//! port of `lib/library/keys.ts`): the router the window content installs in
//! [`crate::window::input`], where it runs before the forwarder and decides
//! whether the core ever sees a keydown. The order is the design's: an open
//! core overlay or an armed prefix keeps the keyboard with the core, an open
//! text field of the panel types, the window keys (Tab across the panes, `1`–`7`
//! for the destination tabs) and the Library keys run in the front end while the
//! Library panel shows, and every other key goes to the core.

use crate::bridge::frame::focus::GuiPanel;
use crate::bridge::frame::library::{
    GuiDestinationList, GuiMusicList, GuiMusicPillFrame, GuiSearchFrame,
};
use crate::bridge::keys::DomKey;
use crate::bridge::library::GuiLibraryCommand;
use crate::window::input::KeyOutcome;
use crate::window::sections::Sections;
use dioxus_native::prelude::*;

use super::keys_shared::{bare, cursor_move, is_list_and_hero_key};
use super::list::step_pill_focus;
use super::state::{LibraryPane, LibraryState};
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

/// Move the pane focus (design D3, step 3, D12): Tab and Shift+Tab walk the
/// panes the active destination paints — the pill row, the list, and the Hero on
/// Music; the list and the Hero on Home; the mode pills, the list, and the Hero
/// on Shows, without the Hero the Upcoming mode does not paint. The ends of that
/// walk cross the panel boundary (design D4): Tab past the last pane and
/// Shift+Tab before the first give the Queue panel Panel focus, and Queue Panel
/// focus gives the Library panel's near end the keyboard, because Panel focus
/// lives in the core.
fn take_pane_focus(state: &LibraryState, panel: GuiPanel, step: i32) {
    let walk = state.active_library_panes();
    if panel == GuiPanel::Queue {
        state.link.library(GuiLibraryCommand::TakeFocus);
        let pane = if step > 0 {
            walk[0]
        } else {
            walk[walk.len() - 1]
        };
        state.focus_library_pane(pane);
        return;
    }
    let ui = state.ui.peek().clone();
    let back = step < 0;
    // The pane's place in the walk (`keys.ts`'s `at`): a pane the walk does
    // not paint has none, so Tab walks from one before the walk's first pane
    // and Shift+Tab from one past its last — neither is a boundary crossing.
    let at = match ui
        .pane
        .and_then(|pane| walk.iter().position(|walked| *walked == pane))
    {
        Some(at) => at,
        None if back => walk.len(),
        None => usize::MAX,
    };
    let boundary = if back {
        at == 0
    } else {
        at == walk.len().saturating_sub(1)
    };
    if boundary {
        state.link.library(GuiLibraryCommand::TakeQueueFocus);
        return;
    }
    state.move_library_pane(step);
}

/// The pills of the active destination's carried Music content.
fn pills(music: &GuiMusicList) -> &[GuiMusicPillFrame] {
    music.pills.as_slice()
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
        Some(GuiDestinationList::Home(_)) => state.accept_home_filter(),
        Some(GuiDestinationList::Emby(_)) => state.accept_emby_filter(),
        Some(GuiDestinationList::Shows(_)) => state.accept_shows_filter(),
        Some(GuiDestinationList::Audiobooks(_)) => state.accept_audiobooks_filter(),
        Some(GuiDestinationList::Podcasts(_)) => state.accept_podcasts_filter(),
        Some(GuiDestinationList::Feeds(_)) => state.accept_feeds_filter(),
        _ => state.accept_library_filter(),
    }
}

/// Route a key to the Home keys (design D12): the Home cursor owns the
/// movement keys wherever the pane focus sits, Enter plays the row it rests
/// on, and `n` and `a` enqueue from it. There is no filter and no pill row on
/// Home, so `/` and Space are not taken: they pass to the core's router. The
/// tree's Right and Left have no node to open, so they change nothing but
/// still never reach the core.
fn route_home_panel_key(state: &LibraryState, key: &DomKey) -> KeyOutcome {
    if key.key == "/" && bare(key) {
        state.open_library_filter();
        return KeyOutcome::Handled;
    }
    if key.key == " " {
        // No pill row on Home (row 6.2): Space keeps its core effect, as it
        // does under Queue focus.
        return KeyOutcome::Core;
    }
    if !bare(key) {
        return KeyOutcome::Core;
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
        _ => KeyOutcome::Core,
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

/// Play the item the focused pane's cursor names (design D3's Enter): the list
/// row, or the Hero's tracklist track while that pane has the pane focus. With
/// no item under the cursor the key sends no command. Enter never picks: a mouse
/// press picks the item for the Hero, and Enter plays.
fn play_focused_item(state: &LibraryState) {
    if let Some(item_id) = state.focused_item_id() {
        state.link.library(GuiLibraryCommand::Play { item_id });
    }
}

/// Route a movement or open key to the list's cursor (design D3, D7): the
/// arrows, Page Up, Page Down, Home, and End move the cursor, Right opens the
/// cursor's node, Left closes it, and Enter plays the item under it.
fn route_list_key(state: &LibraryState, key: &DomKey) -> bool {
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
            play_focused_item(state);
            true
        }
        _ => false,
    }
}

/// Route a movement or Enter key to the Hero's tracklist cursor (design D3):
/// the movement keys move the tracklist's cursor, and Enter plays the track it
/// rests on.
fn route_tracklist_key(state: &LibraryState, key: &DomKey) -> bool {
    if let Some(mv) = cursor_move(&key.key) {
        state.move_library_track_cursor(mv);
        return true;
    }
    if key.key == "Enter" {
        play_focused_item(state);
        return true;
    }
    false
}

/// Route `n` and `a` to the item the focused pane's cursor names (design D3,
/// step 4, D8): play next and add to queue. The Library takes both keys while
/// it has Library Panel focus, so the core's `a` chord never runs from here;
/// with no item under the cursor the key sends no command.
fn route_item_key(state: &LibraryState, key: &DomKey) {
    let Some(item_id) = state.focused_item_id() else {
        return;
    };
    match key.key.as_str() {
        "n" => state.link.library(GuiLibraryCommand::PlayNext { item_id }),
        _ => state
            .link
            .library(GuiLibraryCommand::AddToQueue { item_id }),
    }
}

/// Close the front end's open menu or overlay with Esc (design D3): the right-click
/// item menu and the picked item's menu first, then the Narrow arrangement's Hero
/// overlay, then the carried Search section. Whether one paints decides the
/// answer, so an Esc with nothing open may not leave the core's Esc unclaimed.
/// The carried Search section's close is the one close the open results have
/// (`search.ts`, row 6.8).
fn route_escape_key(
    state: &LibraryState,
    ui: &super::state::LibraryUiState,
    search: Option<&GuiSearchFrame>,
) -> KeyOutcome {
    if ui.row_menu.is_some() {
        state.close_library_row_menu();
        return KeyOutcome::Handled;
    }
    if ui.item_menu_open {
        state.close_library_item_menu();
        return KeyOutcome::Handled;
    }
    if state.hero_overlay_open(ui) {
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
    KeyOutcome::Core
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

/// Move the focused pill one pill along the row (design D3, step 4): Right and
/// Left walk the pills, and the first of them with no pill focused takes the
/// end it walks from.
fn move_pill_focus(state: &LibraryState, pills: &[GuiMusicPillFrame], step: i32) {
    let focused = state.ui.peek().focused_pill.clone();
    let moved = step_pill_focus(
        pills,
        focused.as_deref(),
        |pill| pill.group_id.clone(),
        step,
    );
    if let Some(pill) = moved {
        state.focus_library_pill(Some(pill.group_id));
    }
}

/// Route a key to the active destination's keys (design D3, step 4), with the
/// pane the window content focused deciding which control the key acts on. On
/// Home the Home keys route (design D12), on a plain Emby library the Emby
/// keys (row 8.1), and the Shows, Audiobooks, Podcasts, and Feeds keys route
/// with their ports (rows 6.5 to 6.7). The Settings placeholder remains
/// (row 8.1); a key under that destination goes to the core's router.
fn route_panel_key(
    state: &LibraryState,
    key: &DomKey,
    ui: &super::state::LibraryUiState,
    sections: &Sections,
    search: Option<&GuiSearchFrame>,
) -> KeyOutcome {
    if key.key == "Escape" && bare(key) {
        return route_escape_key(state, ui, search);
    }
    let list = sections.library_list.peek();
    match list.as_ref().map(|list| &list.destination) {
        Some(GuiDestinationList::Home(_)) => return route_home_panel_key(state, key),
        Some(GuiDestinationList::Emby(_)) => {
            return super::emby_keys::route_emby_panel_key(state, key);
        }
        Some(GuiDestinationList::Shows(_)) => {
            return super::shows_keys::route_shows_panel_key(state, key);
        }
        Some(GuiDestinationList::Audiobooks(_)) => {
            return super::audiobooks_keys::route_audiobooks_panel_key(state, key);
        }
        Some(GuiDestinationList::Podcasts(_)) => {
            return super::podcasts_keys::route_podcasts_panel_key(state, key);
        }
        Some(GuiDestinationList::Feeds(_)) => {
            return super::feeds_keys::route_feeds_panel_key(state, key);
        }
        Some(GuiDestinationList::Music(_)) => {}
        // No carried Library list paints no Library keys: the core takes
        // every one of them (design D2, D10).
        None => return KeyOutcome::Core,
    }
    let Some(music) = super::parts::library_music_list(list.as_ref()) else {
        return KeyOutcome::Core;
    };
    route_music_panel_key(state, key, ui, music)
}

/// The Music destination's own keys (design D3, D12), over the carried Music
/// content.
fn route_music_panel_key(
    state: &LibraryState,
    key: &DomKey,
    ui: &super::state::LibraryUiState,
    music: &GuiMusicList,
) -> KeyOutcome {
    if key.key == "/" && bare(key) {
        state.open_library_filter();
        return KeyOutcome::Handled;
    }
    // Space selects the focused pill (design D6). With
    // no pill focused, it is the core's: Space plays and pauses (design D3,
    // step 5).
    if key.key == " " && !key.modifiers.ctrl_key && !key.meta_key {
        let pill = state.focused_library_pill(&music.pills, ui);
        let Some(pill) = pill else {
            return KeyOutcome::Core;
        };
        state.request_library_pill_select(pill.group_id.clone());
        return KeyOutcome::Handled;
    }
    if ui.pane == Some(LibraryPane::Pills) && bare(key) {
        match key.key.as_str() {
            "ArrowRight" => {
                move_pill_focus(state, pills(music), 1);
                return KeyOutcome::Handled;
            }
            "ArrowLeft" => {
                move_pill_focus(state, pills(music), -1);
                return KeyOutcome::Handled;
            }
            // Enter presses the focused pill, as Space does (design D3, step
            // 4, D6).
            "Enter" => {
                if let Some(pill) = state.focused_library_pill(&music.pills, ui) {
                    let group_id = pill.group_id.clone();
                    state.request_library_pill_select(group_id);
                    return KeyOutcome::Handled;
                }
            }
            _ => {}
        }
    }
    if ui.pane == Some(LibraryPane::List) && bare(key) && route_list_key(state, key) {
        return KeyOutcome::Handled;
    }
    if ui.pane == Some(LibraryPane::Hero) && bare(key) && route_tracklist_key(state, key) {
        return KeyOutcome::Handled;
    }
    if bare(key) && (key.key == "n" || key.key == "a") {
        route_item_key(state, key);
        return KeyOutcome::Handled;
    }
    if bare(key) && is_list_and_hero_key(&key.key) {
        return KeyOutcome::Handled;
    }
    // Nothing else of the Library's answers this key: the core's router does.
    KeyOutcome::Core
}

/// Route one keydown (design D3): the front end does its part, and the answer is
/// what the input forwarder must do with the key. The section facts the routing
/// reads come from the window's own section stores (`gui-change-driven-sections`
/// design D6), read through `peek` — a keydown copies none of their values.
#[must_use]
pub fn route_library_key(
    key: &DomKey,
    sections: Option<&Sections>,
    state: &LibraryState,
    settings: Option<&SettingsState>,
) -> KeyOutcome {
    let Some(sections) = sections else {
        return KeyOutcome::Core;
    };
    // (1) An open core overlay or an armed prefix keeps the keyboard with the
    // core: both are section facts, and the core's own router answers them.
    if sections.overlay.peek().is_some() || sections.status.peek().prefix_armed {
        return KeyOutcome::Core;
    }
    // The Queue filter's text entry is the core's, so while it holds the
    // keyboard every key is the core's to type (`text_entry_focused` in
    // `src/app/input/gui.rs`).
    if sections.summary.peek().filter_active {
        return KeyOutcome::Core;
    }
    // The Settings screen takes every key the overlay step leaves while its
    // frame section is present (add-gui-settings-screen design D11): the
    // window paints the screen in place of the two panels then, so no Library
    // step runs. The Settings steps route ahead of the Library ones (row
    // 8.1); a harness that carries no Settings store answers the core.
    if let Some(frame) = sections.settings.peek().as_ref() {
        return match settings {
            Some(settings) => route_settings_key(key, frame, settings),
            None => KeyOutcome::Core,
        };
    }
    // The attach states carry no slice, and no Library panel means no Library
    // keys: the core takes every one of them. The panel's presence is the
    // window's mount fact (design D2, D10), so a key never acts on a panel that
    // paints nothing.
    // The panel's presence is one fact for the window and the router (design
    // D2, D10): sections that carry no Library list take no Library key,
    // even while the window still holds the content of an earlier frame. The
    // content names the active destination's section (design D12).
    if !LibraryState::library_shown(sections.library_list.peek().as_ref()) {
        return KeyOutcome::Core;
    }
    // The `Ctrl+K` chord (design D3, step 3, row 12.1): the Search field takes
    // the keyboard wherever the Library panel shows, before the open-field
    // checks — even while the filter holds it, where the DOM focus move closes
    // the filter the way any other focus move does. The chord arms the field
    // only while a top bar mounts it: in Mini with Queue focus the panel paints
    // no top bar, and arming a field that holds no element leaves the keyboard
    // dead on its fact, so the chord keeps its core effect there instead.
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
    let ui = state.ui.peek().clone();
    if ui.filter_open {
        return route_filter_key(state, key);
    }
    // (3) The window keys run whenever the Library panel shows.
    if key.key == "Tab" && !key.modifiers.ctrl_key && !key.meta_key && !key.modifiers.alt_key {
        let step = if key.modifiers.shift_key { -1 } else { 1 };
        take_pane_focus(state, sections.focus.peek().panel, step);
        return KeyOutcome::Handled;
    }
    if let Some(place) = destination_place(key) {
        // A digit names a carried tab, never a raw position: it opens the tab
        // the top bar paints at that place, and a digit past the carried tabs
        // opens nothing (design D3, step 3).
        if let Some(tab) = sections
            .library_list
            .peek()
            .as_ref()
            .and_then(|list| list.tabs.get(place.wrapping_sub(1)))
        {
            state.link.library(GuiLibraryCommand::OpenDestination {
                position: tab.position,
            });
        }
        return KeyOutcome::Handled;
    }
    // (4) The Library keys run while the Library panel has Panel focus. The
    // carried Search section rides with them: its Esc close needs it.
    if sections.focus.peek().panel == GuiPanel::Library {
        return route_panel_key(state, key, &ui, sections, sections.search.peek().as_ref());
    }
    // (5) Every other key goes to the core's keyboard router.
    KeyOutcome::Core
}

#[cfg(test)]
mod tests;
