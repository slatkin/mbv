//! The Library panel's own state (`add-gui-library-panel` design D1, the port
//! of `lib/library/state.ts`): what the window content owns and the core never
//! reads there. Two kinds stay apart — the content the `library` window
//! section carries, and the interaction state the panel's keys and controls
//! move.
//!
//! The core owns the data selection (which collections are selected, the picked
//! item), so nothing here mirrors it: the pill toggle the keys request is an
//! intent the pill row turns into the set-collections command (design D6).

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiDestinationList, GuiLibraryDetail, GuiLibraryList, GuiMusicPillFrame,
};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::Clock;
use crate::window::sections::Sections;

use std::sync::Arc;

use super::hero::{hero_tracks, move_track_cursor, resolve_track_cursor};
use super::list::ListMove;
use super::parts::library_music_detail;
use super::row_memo::RowsMemo;
use super::scroll::{ListScroll, initial_list_scroll};

/// The panes Tab moves focus across (design D3, step 3). The Queue panel is
/// the pane past the Hero, but it takes Panel focus through the core's focus
/// stack, which no key command reaches (design D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryPane {
    /// The destination's pill row.
    Pills,
    /// The list.
    List,
    /// The Hero.
    Hero,
}

/// The order Tab and Shift+Tab walk the panes in (design D3, step 3).
const PANES: &[LibraryPane] = &[LibraryPane::Pills, LibraryPane::List, LibraryPane::Hero];

/// The Library panel's panes on Home (design D12): the flat list and the
/// Hero. Home paints no pill row and no letter rail, so the pane walk skips
/// them.
pub const HOME_PANES: &[LibraryPane] = &[LibraryPane::List, LibraryPane::Hero];

/// The Library panel's panes on Shows (design D12, row 7.1): the mode pills,
/// the list, and the Hero. The Upcoming mode paints no Hero, so the walk skips
/// it there.
pub const SHOWS_PANES: &[LibraryPane] = &[LibraryPane::Pills, LibraryPane::List, LibraryPane::Hero];
/// The Shows walk while the detail paints no Hero.
pub const SHOWS_PANES_NO_HERO: &[LibraryPane] = &[LibraryPane::Pills, LibraryPane::List];

/// The Library panel's panes on the Audiobooks destination (design D12, row
/// 9.1): the book list and the Hero. No pill row rides on the destination — the
/// letter rail replaces the TUI's surname buckets — and the Hero's chapter
/// Workspace belongs to the Hero pane.
pub const AUIB_PANES: &[LibraryPane] = &[LibraryPane::List, LibraryPane::Hero];
/// The Audiobooks walk while the detail paints no Hero.
pub const AUIB_PANES_NO_HERO: &[LibraryPane] = &[LibraryPane::List];

/// The Library panel's panes on a plain Emby library (design D12, row 8.1):
/// the list and the Hero, because a plain library paints no pills — its letter
/// rail replaces the TUI's letter-range pill row.
pub const EMBY_PANES: &[LibraryPane] = &[LibraryPane::List, LibraryPane::Hero];
/// The plain Emby walk while the detail paints no Hero.
pub const EMBY_PANES_NO_HERO: &[LibraryPane] = &[LibraryPane::List];

/// The Library panel's panes on a feed-group Emby library (design D12, row
/// 8.1): the feed-group pills, the list, and the Hero.
pub const EMBY_FEED_PANES: &[LibraryPane] =
    &[LibraryPane::Pills, LibraryPane::List, LibraryPane::Hero];
/// The feed-group Emby walk while the detail paints no Hero.
pub const EMBY_FEED_PANES_NO_HERO: &[LibraryPane] = &[LibraryPane::Pills, LibraryPane::List];

/// The Library panel's panes on the Podcasts destination (design D12, row
/// 10.1): the episode pills, the date-gutter list, and the Hero. A frame
/// without a Hero leaves the walk on the pills and the list.
pub const PODCASTS_PANES: &[LibraryPane] =
    &[LibraryPane::Pills, LibraryPane::List, LibraryPane::Hero];
/// The Podcasts walk while the detail paints no Hero.
pub const PODCASTS_PANES_NO_HERO: &[LibraryPane] = &[LibraryPane::Pills, LibraryPane::List];

/// The Library panel's panes on the Feeds destination (design D12, row
/// 11.1): the view pills, the date-gutter list, and the Hero. A frame
/// without a Hero leaves the walk on the pills and the list.
pub const FEEDS_PANES: &[LibraryPane] = &[LibraryPane::Pills, LibraryPane::List, LibraryPane::Hero];
/// The Feeds walk while the detail paints no Hero.
pub const FEEDS_PANES_NO_HERO: &[LibraryPane] = &[LibraryPane::Pills, LibraryPane::List];

/// The right-click item menu's facts (`state.ts`'s `rowMenu`): the item a row
/// right-press named, with the pointer position it opened at. The menu paints
/// only while the frame carries that item's entries, so a stale open state
/// paints nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct RowMenu {
    /// The item the row named.
    pub item_id: String,
    /// The pointer's client x when the menu opened.
    pub x: f32,
    /// The pointer's client y when the menu opened.
    pub y: f32,
}

/// The Library panel's interaction state (`state.ts`'s `LibraryUiState`).
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryUiState {
    /// The pane the window content focuses, `None` before the first walk.
    pub pane: Option<LibraryPane>,
    /// Whether a text field of the panel holds the keyboard (design D3,
    /// step 2).
    pub filter_open: bool,
    /// The filter's text: the `/` field types it, and it narrows the list
    /// without case (design D7).
    pub filter: String,
    /// The pill Left and Right move the focus to, or `None` while no pill has
    /// it (design D6).
    pub focused_pill: Option<String>,
    /// The pill selection the keys asked for: Space and Enter select the
    /// focused pill (design D6). The pill row turns it into the
    /// select-collection command.
    pub pill_select: Option<String>,
    /// The list's cursor row id, or `None` while no row carries the cursor
    /// (design D7). The cursor and the picked item are different: the cursor
    /// moves by keys, and the picked album is the core's.
    pub cursor: Option<String>,
    /// The list's open tree node row ids (design D7): an artist and an album
    /// each open and close on their own.
    pub open_nodes: Vec<String>,
    /// The node row ids the user closed while the filter narrowed the rows:
    /// the filter opens a node that holds a match, so the close half needs an
    /// override to stay shut (design D7's open and close). A new filter and a
    /// cleared one drop it.
    pub filter_closed: Vec<String>,
    /// The tracklist's cursor track id (design D3): the Hero's tracklist row
    /// the movement keys rest on while the Hero pane has the pane focus.
    pub track_cursor: Option<String>,
    /// Whether the Narrow arrangement's Hero overlay is dismissed (design D4,
    /// D3): a pick paints it again, and Esc or a press on its backdrop closes
    /// it.
    pub hero_dismissed: bool,
    /// Whether the picked item's menu is open (design D8): More actions opens
    /// it, a chosen entry runs and closes it, and Esc closes it.
    pub item_menu_open: bool,
    /// The right-click item menu (the row-menu command), `None` while no
    /// right-click menu is up.
    pub row_menu: Option<RowMenu>,
}

impl LibraryUiState {
    /// The state a freshly mounted window content starts with
    /// (`state.ts`'s `initialLibraryUi`).
    #[must_use]
    pub fn initial() -> Self {
        Self {
            pane: Some(LibraryPane::Pills),
            filter_open: false,
            filter: String::new(),
            focused_pill: None,
            pill_select: None,
            cursor: None,
            open_nodes: Vec::new(),
            filter_closed: Vec::new(),
            track_cursor: None,
            hero_dismissed: false,
            item_menu_open: false,
            row_menu: None,
        }
    }
}

/// The window's Library interaction store (D15): the port of `state.ts`'s
/// module-level stores as one context-provided struct. The root creates it
/// once and provides it as context; the key router and the painters read and
/// write it. A clone shares every field's storage, so the key router's closure
/// and the painters see one state.
#[derive(Clone)]
pub struct LibraryState {
    /// The panel's interaction state (`state.ts`'s `libraryUi`).
    pub ui: Signal<LibraryUiState>,
    /// The Music list's scroll store (`scroll.ts`'s `musicScroll`).
    pub music_scroll: Signal<ListScroll>,
    /// The other destinations' scroll stores (`scroll.ts`'s module stores).
    pub scrolls: super::scroll::LibraryScrolls,
    /// The Home list's interaction state (`home.ts`'s `homeUi`).
    pub home_ui: Signal<super::home::HomeUiState>,
    /// The Emby list's interaction state (`emby.ts`'s `embyUi`).
    pub emby_ui: Signal<super::emby::EmbyUiState>,
    /// The Shows list's interaction state (`shows.ts`'s `showsUi`).
    pub shows_ui: Signal<super::shows::ShowsUiState>,
    /// The Shows Hero's Workspace cursor (`shows-workspace.ts`'s
    /// `showsWorkspace`).
    pub shows_workspace: Signal<super::shows_workspace::ShowsWorkspaceState>,
    /// The Audiobooks list's interaction state (`audiobooks.ts`'s
    /// `audiobooksUi`).
    pub audiobooks_ui: Signal<super::audiobooks::AudiobooksUiState>,
    /// The Podcasts list's interaction state (`podcasts.ts`'s
    /// `podcastsUi`).
    pub podcasts_ui: Signal<super::podcasts::PodcastsUiState>,
    /// The Feeds list's interaction state (`feeds.ts`'s `feedsUi`).
    pub feeds_ui: Signal<super::feeds::FeedsUiState>,
    /// The Search field's state (`search.ts`'s `librarySearch`).
    pub search: Signal<super::search::LibrarySearchState>,
    /// The pending query send's timer (D10), or `None` while none is armed.
    pub search_send: Signal<Option<crate::window::clock::TimerId>>,
    /// The Audiobooks Hero's Workspace cursor (`audiobooks-workspace.ts`'s
    /// `audiobooksWorkspace`).
    pub audiobooks_workspace: Signal<super::audiobooks_workspace::AudiobooksWorkspaceState>,
    /// The Home list's row memo (`home-list.ts`'s `homeRowsOf`).
    pub home_rows_memo: super::home_list::HomeRowsMemo,
    /// The Emby list's row memo (`emby-list.ts`'s `embyRowsOf`).
    pub emby_rows_memo: super::emby_list::EmbyRowsMemo,
    /// The Shows list's row memo (`shows-list.ts`'s `showsRowsOf`).
    pub shows_rows_memo: super::shows_list::ShowsRowsMemo,
    /// The Audiobooks list's row memo (`audiobooks-list.ts`'s
    /// `audiobooksRowsOf`).
    pub audiobooks_rows_memo: super::audiobooks_list::AudiobooksRowsMemo,
    /// The Podcasts list's row memo (`podcasts-list.ts`'s
    /// `podcastsRowsOf`).
    pub podcasts_rows_memo: super::podcasts_list::PodcastsRowsMemo,
    /// The Feeds list's row memo (`feeds-list.ts`'s `feedsRowsOf`).
    pub feeds_rows_memo: super::feeds_list::FeedsRowsMemo,
    /// The playhead's predicted position, in ticks (design D5): the
    /// Audiobooks chapter Workspace reads it to mark the playing chapter row.
    /// The playhead's own memo, shared — never a second prediction.
    pub playhead_position: Memo<f64>,
    /// The section reads the interaction state resolves against
    /// (`parts.ts`'s readers over the `libraryList` and `libraryDetail`
    /// stores).
    pub sections: Sections,
    /// The core link the Library commands go through
    /// (`lib/bridge/library.ts`'s `sendLibraryCommand`).
    pub link: CoreLink,
    /// The clock the Search query's debounce arms (D10).
    pub clock: Arc<dyn Clock>,
    /// The album row a reveal waits to scroll into view, until the list is
    /// measured (`state.ts`'s module-level `pendingRevealRow`).
    pub pending_reveal_row: Signal<Option<String>>,
    /// The canvas tree's row memo (`state.ts`'s `musicRowsOf`, built over
    /// `row-memo.ts`).
    pub rows_memo: RowsMemo,
}

impl std::fmt::Debug for LibraryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryState")
            .field("ui", &self.ui.peek().clone())
            .field("music_scroll", &self.music_scroll.peek().clone())
            .finish_non_exhaustive()
    }
}

impl LibraryState {
    /// The store a window start builds: the freshly mounted interaction
    /// state over the current sections, core link, and the playhead's
    /// predicted position (the Audiobooks chapter Workspace's marked row reads
    /// it).
    #[must_use]
    pub fn new(
        sections: Sections,
        link: CoreLink,
        playhead_position: Memo<f64>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            ui: Signal::new(LibraryUiState::initial()),
            music_scroll: Signal::new(initial_list_scroll()),
            scrolls: super::scroll::LibraryScrolls::new(),
            home_ui: Signal::new(super::home::HomeUiState::initial()),
            emby_ui: Signal::new(super::emby::initial_emby_ui()),
            shows_ui: Signal::new(super::shows::ShowsUiState::initial()),
            shows_workspace: Signal::new(super::shows_workspace::ShowsWorkspaceState::initial()),
            audiobooks_ui: Signal::new(super::audiobooks::AudiobooksUiState::initial()),
            audiobooks_workspace: Signal::new(
                super::audiobooks_workspace::AudiobooksWorkspaceState::initial(),
            ),
            podcasts_ui: Signal::new(super::podcasts::initial_podcasts_ui()),
            feeds_ui: Signal::new(super::feeds::initial_feeds_ui()),
            search: Signal::new(super::search::initial_library_search()),
            search_send: Signal::new(None),
            home_rows_memo: super::home_list::HomeRowsMemo::default(),
            emby_rows_memo: super::emby_list::EmbyRowsMemo::default(),
            shows_rows_memo: super::shows_list::ShowsRowsMemo::default(),
            audiobooks_rows_memo: super::audiobooks_list::AudiobooksRowsMemo::default(),
            podcasts_rows_memo: super::podcasts_list::PodcastsRowsMemo::default(),
            feeds_rows_memo: super::feeds_list::FeedsRowsMemo::default(),
            playhead_position,
            sections,
            link,
            clock,
            pending_reveal_row: Signal::new(None),
            rows_memo: RowsMemo::default(),
        }
    }

    /// Whether the Library panel shows (design D2, D10, D12): the
    /// `libraryList` section's value names an active destination to paint. The
    /// window mounts the panel on this one fact and the key router gates on
    /// it, so no Library key acts while the panel paints nothing.
    #[must_use]
    pub fn library_shown(list: Option<&GuiLibraryList>) -> bool {
        list.is_some()
    }

    /// The panes the active destination paints (design D12): the one walk on
    /// Home skips the panes it does not paint, on Shows it skips the Hero the
    /// Upcoming mode does not paint, and on an Emby library it skips the pill
    /// row a plain library does not paint and the Hero a frame without one
    /// does not.
    #[must_use]
    pub fn active_library_panes(&self) -> &'static [LibraryPane] {
        let list = self.sections.library_list.peek();
        match list.as_deref().map(|list| &list.destination) {
            Some(GuiDestinationList::Home(_)) => HOME_PANES,
            Some(GuiDestinationList::Shows(_)) => {
                if self.library_detail_has_hero() {
                    SHOWS_PANES
                } else {
                    SHOWS_PANES_NO_HERO
                }
            }
            Some(GuiDestinationList::Emby(emby)) => {
                Self::emby_panes(emby.feed_group, self.library_detail_has_hero())
            }
            Some(GuiDestinationList::Audiobooks(_)) => {
                if self.library_detail_has_hero() {
                    AUIB_PANES
                } else {
                    AUIB_PANES_NO_HERO
                }
            }
            Some(GuiDestinationList::Podcasts(_)) => {
                if self.library_detail_has_hero() {
                    PODCASTS_PANES
                } else {
                    PODCASTS_PANES_NO_HERO
                }
            }
            Some(GuiDestinationList::Feeds(_)) => {
                if self.library_detail_has_hero() {
                    FEEDS_PANES
                } else {
                    FEEDS_PANES_NO_HERO
                }
            }
            _ => PANES,
        }
    }

    /// The Emby walk's panes: the feed-group mode's pill row and the Hero ride
    /// only where the frame paints them (row 8.1).
    #[must_use]
    fn emby_panes(feed_group: bool, has_hero: bool) -> &'static [LibraryPane] {
        match (feed_group, has_hero) {
            (true, true) => EMBY_FEED_PANES,
            (true, false) => EMBY_FEED_PANES_NO_HERO,
            (false, true) => EMBY_PANES,
            (false, false) => EMBY_PANES_NO_HERO,
        }
    }

    /// Whether the active destination's detail part paints a Hero: every
    /// destination's detail carries a `hero`, `None` while none is picked.
    #[must_use]
    fn library_detail_has_hero(&self) -> bool {
        let detail = self.sections.library_detail.peek();
        match detail.as_deref() {
            Some(GuiLibraryDetail::Music(detail)) => detail.hero.is_some(),
            Some(GuiLibraryDetail::Home(detail)) => detail.hero.is_some(),
            Some(GuiLibraryDetail::Shows(detail)) => detail.hero.is_some(),
            Some(GuiLibraryDetail::Emby(detail)) => detail.hero.is_some(),
            Some(GuiLibraryDetail::Audiobooks(detail)) => detail.hero.is_some(),
            Some(GuiLibraryDetail::Podcasts(detail)) => detail.hero.is_some(),
            Some(GuiLibraryDetail::Feeds(detail)) => detail.hero.is_some(),
            None => false,
        }
    }

    /// Open the list's filter: `/` with Library Panel focus (design D3, step 4).
    pub fn open_library_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = true);
    }

    /// Close the filter with Esc: the text goes with it, so a closed filter
    /// narrows nothing (design D3, step 2). The cursor and the scroll follow the
    /// widened rows, and the filter's own open and close over the nodes goes with
    /// the text.
    pub fn close_library_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.filter_open = false;
            ui.filter.clear();
            ui.filter_closed.clear();
        });
        self.settle_cursor();
    }

    /// Accept the filter with Enter: the field closes and the list keeps the text
    /// (design D3, step 2). The cursor and the scroll follow the rows the same way
    /// a text change moves them.
    pub fn accept_library_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        self.settle_cursor();
    }

    /// Focus a pane: the window content of Tab and Shift+Tab (design D3, step 3).
    /// Taking the Hero's pane paints the Hero again where the arrangement overlays
    /// it, so the keys never act on a pane that paints nothing.
    pub fn focus_library_pane(&self, pane: LibraryPane) {
        let mut ui = self.ui;
        ui.with_mut(|current| {
            *current = with_hero_pane(LibraryUiState {
                pane: Some(pane),
                ..current.clone()
            });
        });
    }

    /// Move the pane focus `step` panes along, wrapping at the ends. The walk
    /// skips the panes the active destination does not paint (design D12): Home
    /// walks the list and the Hero only.
    pub fn move_library_pane(&self, step: i32) {
        let panes = self.active_library_panes();
        let current = self.ui.peek().clone();
        let at = panes
            .iter()
            .position(|pane| Some(*pane) == active_library_pane(&current, panes))
            .unwrap_or(0);
        let next = if step >= 0 {
            at.saturating_add(step.unsigned_abs() as usize) % panes.len()
        } else {
            (at + panes.len()).saturating_sub(step.unsigned_abs() as usize) % panes.len()
        };
        let pane = panes[next];
        let mut ui = self.ui;
        ui.with_mut(|current| {
            *current = with_hero_pane(LibraryUiState {
                pane: Some(pane),
                ..current.clone()
            });
        });
    }

    /// Focus a pill, or no pill while `group_id` is `None`.
    pub fn focus_library_pill(&self, group_id: Option<String>) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.focused_pill = group_id);
    }

    /// Ask for the focused pill to be selected: Space and Enter request it (design
    /// D6).
    pub fn request_library_pill_select(&self, group_id: String) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.pill_select = Some(group_id));
    }

    /// Select the collection pill `group_id` of the Grouped Music library
    /// `lib_idx` (design D6): one group is always selected, as in the TUI. The
    /// core stores the choice per destination and loads the group, and the pills
    /// paint the core's choice back, so the row and the list cannot disagree.
    pub fn select_library_pill(&self, lib_idx: usize, group_id: String) {
        self.link
            .library(GuiLibraryCommand::SelectCollection { lib_idx, group_id });
    }

    /// Turn the keys' pill request into the select-collection command the pill
    /// sends (design D6). The intent is consumed here, so one key sends one
    /// command.
    pub fn apply_library_pill_select(&self, lib_idx: usize) {
        let Some(pill_select) = self.ui.peek().pill_select.clone() else {
            return;
        };
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.pill_select = None);
        self.select_library_pill(lib_idx, pill_select);
    }

    /// The pill the pill row's focus holds, resolved against the carried content
    /// so a stale id focuses nothing.
    #[must_use]
    pub fn focused_library_pill<'a>(
        &self,
        pills: &'a [GuiMusicPillFrame],
        ui: &LibraryUiState,
    ) -> Option<&'a GuiMusicPillFrame> {
        if ui.pane != Some(LibraryPane::Pills) {
            return None;
        }
        let focused = ui.focused_pill.as_deref()?;
        pills.iter().find(|pill| pill.group_id == focused)
    }

    /// The active Music detail's Hero, `None` while another destination holds
    /// the window or no album is picked.
    #[must_use]
    fn music_hero(&self) -> Option<crate::bridge::frame::library::GuiMusicHeroFrame> {
        let detail = self.sections.library_detail.peek();
        library_music_detail(detail.as_deref()).and_then(|detail| detail.hero.clone())
    }

    /// Move the Hero's tracklist cursor one movement along (design D3): the
    /// movement keys move the tracklist's cursor, not the list's, while the Hero
    /// pane has the pane focus. The movement starts from the cursor the painted
    /// tracklist shows, so the first key lands beside it.
    pub fn move_library_track_cursor(&self, mv: ListMove) {
        let hero = self.music_hero();
        let tracks = hero_tracks(hero.as_ref());
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            let cursor = move_track_cursor(
                &tracks,
                resolve_track_cursor(&tracks, ui.track_cursor.as_deref()).as_deref(),
                mv,
            );
            if cursor != ui.track_cursor {
                ui.track_cursor = cursor;
            }
        });
    }

    /// The item the focused pane's cursor names (design D3, D8): the tracklist's
    /// track while the Hero pane has the pane focus, and the list's row while the
    /// list has it. `None` while no cursor rests on an item, so the key that acts
    /// on it sends no command. The Hero pane's track is the one the painted
    /// tracklist shows, so Enter acts before any movement key has moved the
    /// cursor.
    #[must_use]
    pub fn focused_item_id(&self) -> Option<String> {
        let ui = self.ui.peek().clone();
        if ui.pane == Some(LibraryPane::Hero) {
            let hero = self.music_hero();
            let tracks = hero_tracks(hero.as_ref());
            return resolve_track_cursor(&tracks, ui.track_cursor.as_deref());
        }
        if ui.pane == Some(LibraryPane::List) {
            let row = self.cursor_row(&ui);
            return row.as_ref().and_then(Self::action_item_id);
        }
        None
    }

    /// Whether the Narrow arrangement paints the Hero overlay (design D4, D3):
    /// a picked item, an arrangement that overlays rather than splits, and no
    /// dismissal since that pick. The Hero is the Music Hero, the Home Hero, the
    /// Shows Hero, or a plain Emby library's Hero (design D12), whichever
    /// destination is active. Esc and a press on the backdrop close it. The
    /// `arrangement` is the one value the window hands both this predicate's
    /// caller and the panel's paint (design D4): the keys never read a report
    /// that may not have landed yet.
    #[must_use]
    pub fn hero_overlay_open(&self, ui: &LibraryUiState, arrangement: GuiArrangement) -> bool {
        arrangement == GuiArrangement::Narrow
            && !ui.hero_dismissed
            && self.library_detail_has_hero()
    }

    /// Close the Narrow Hero overlay (design D3): Esc while it paints, and a press
    /// on its backdrop. A later pick, or a Tab onto the Hero's pane, paints it
    /// again. While the Hero pane held the pane focus, the focus falls back to the
    /// list, so the keys never rest on a pane that paints nothing.
    pub fn close_library_hero(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if ui.pane == Some(LibraryPane::Hero) {
                ui.pane = Some(LibraryPane::List);
            }
            ui.hero_dismissed = true;
            ui.item_menu_open = false;
        });
    }

    /// Open the picked item's menu (design D8): More actions paints the entries the
    /// frame carries.
    pub fn open_library_item_menu(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if !ui.item_menu_open {
                ui.item_menu_open = true;
                ui.row_menu = None;
            }
        });
    }

    /// Close the picked item's menu (design D8): a chosen entry closes it, and so
    /// does Esc.
    pub fn close_library_item_menu(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.item_menu_open = false);
    }

    /// Open the right-click item menu for `item_id` (the row-menu command): the
    /// next frame carries the item's full entries for the menu to paint at the
    /// pointer position, and a chosen entry sends its index for that item. The
    /// Hero's own menu closes with it — one menu is up at a time.
    pub fn open_library_row_menu(&self, item_id: String, x: f32, y: f32) {
        self.link.library(GuiLibraryCommand::RowMenu {
            item_id: item_id.clone(),
        });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.item_menu_open = false;
            ui.row_menu = Some(RowMenu { item_id, x, y });
        });
    }

    /// Open a row's item menu from its right-click (design D8, D15): the row's own
    /// stable id, or nothing when the row names no item — a letter heading, a spacer,
    /// or a node the Service does not resolve. Every destination's list opens its
    /// menu through this one call, so the pointer rule cannot drift between tabs.
    /// The Dioxus handler calls `prevent_default` itself and reads the event's
    /// client coordinates; the open-or-drop decision lives here.
    pub fn open_row_menu_at(&self, item_id: Option<String>, x: f32, y: f32) {
        if let Some(item_id) = item_id {
            self.open_library_row_menu(item_id, x, y);
        }
    }

    /// Close the right-click item menu: a chosen entry closes it, and so do Esc,
    /// a press outside it, and a scroll of the surface the row painted in.
    pub fn close_library_row_menu(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.row_menu = None);
    }

    /// Run the right-click menu's entry at `index` for the item it opened on: the
    /// same menu-entry command the Hero's menu sends, resolved by that item's
    /// stable id in the core. The menu closes behind the press.
    pub fn run_library_row_menu_entry(&self, index: usize) {
        let row_menu = self.ui.peek().row_menu.clone();
        self.close_library_row_menu();
        if let Some(row_menu) = row_menu {
            self.link.library(GuiLibraryCommand::MenuEntry {
                item_id: row_menu.item_id,
                index,
            });
        }
    }
}

/// The pane of `ui` the walk starts from: the pane itself while the active
/// destination paints it, and the walk's first pane otherwise — a destination
/// switch that leaves the last destination's pane never walks from a pane
/// that paints nothing.
#[must_use]
fn active_library_pane(ui: &LibraryUiState, panes: &[LibraryPane]) -> Option<LibraryPane> {
    match ui.pane {
        Some(pane) if panes.contains(&pane) => Some(pane),
        _ => panes.first().copied(),
    }
}

/// The state with the Hero pane ready while it takes the pane focus (design
/// D3, D4): the Narrow arrangement paints the Hero again, so the keys never act
/// on a pane that paints nothing, and the tracklist's cursor is derived from the
/// tracks that paint rather than seeded here.
#[must_use]
fn with_hero_pane(mut ui: LibraryUiState) -> LibraryUiState {
    if ui.pane == Some(LibraryPane::Hero) {
        ui.hero_dismissed = false;
    }
    ui
}

mod rows;

#[cfg(test)]
mod tests;
