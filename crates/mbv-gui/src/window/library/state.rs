//! The Library panel's own state (`add-gui-library-panel` design D1, the port
//! of `lib/library/state.ts`): what the window content owns and the core never
//! reads there. Two kinds stay apart — the content the `library` window
//! section carries, and the interaction state the panel's keys and controls
//! move.
//!
//! The core owns the data selection (which collections are selected, the picked
//! item), so nothing here mirrors it: a press on the collection dropdown sends
//! the collection command the frame names, and the frame carries the choice
//! back. The dropdown's open fact and its filter text are interaction state, so
//! they live here (gui-collection-dropdown design D7).

use dioxus_native::prelude::*;

use mbv::GuiCollections;
use mbv::{GuiCollectionTarget, GuiListSort, GuiShowsView, GuiSortTarget};

use crate::bridge::frame::library::{GuiCollectionsFrame as _, GuiLibraryDetail, GuiLibraryList};
use crate::bridge::library::{GuiLibraryCommand, GuiRowArtScopeFrame};
use crate::window::arrangement::GuiArrangement;
use crate::window::channel::CoreLink;
use crate::window::clock::Clock;
use crate::window::sections::Sections;

use std::sync::Arc;

use super::row_memo::RowsMemo;
use super::scroll::{ListScroll, initial_list_scroll, reset_scroll_top};

/// The right-click item menu's facts (`state.ts`'s `rowMenu`): the row a row
/// right-press named, with the pointer position it opened at. The menu paints
/// only while the frame carries that row's entries, so a stale open state
/// paints nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct RowMenu {
    /// The row the right-press named, by its destination identity.
    pub row: mbv::GuiLibraryMenuRow,
    /// The pointer's client x when the menu opened.
    pub x: f32,
    /// The pointer's client y when the menu opened.
    pub y: f32,
    /// Whether the menu offers View Detail: `false` for a row whose press
    /// picks no item (a Music artist row), which has no Hero to open.
    pub detail: bool,
}

/// The Library panel's interaction state (`state.ts`'s `LibraryUiState`).
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryUiState {
    /// Whether a text field of the panel holds the keyboard (design D3,
    /// step 2).
    pub filter_open: bool,
    /// The filter's text: the `/` field types it, and it narrows the list
    /// without case (design D7).
    pub filter: String,
    /// The collection dropdown's interaction state (gui-collection-dropdown
    /// design D7): `None` while the dropdown is closed, and `Some(filter)`
    /// while it is open, holding the filter text the open popover narrows its
    /// entries by. One value, so a close clears the filter by construction.
    pub collections: Option<String>,
    /// The Shows Hero's season dropdown's interaction state (user request
    /// 2026-11-19, the season pills' replacement): `None` while closed, and
    /// `Some(filter)` while open, holding the filter text the open popover
    /// narrows its season rows by — the collection dropdown's one-value
    /// pattern, so a close clears the filter by construction.
    pub shows_seasons: Option<String>,
    /// The list's open tree node row ids (design D7): an artist and an album
    /// each open and close on their own.
    pub open_nodes: Vec<String>,
    /// The node row ids the user closed while the filter narrowed the rows:
    /// the filter opens a node that holds a match, so the close half needs an
    /// override to stay shut (design D7's open and close). A new filter and a
    /// cleared one drop it.
    pub filter_closed: Vec<String>,
    /// The top bar's Search control's window facts (user request 2026-11-19):
    /// whether the bar is too narrow for the inline field, and whether the
    /// collapsed panel paints.
    pub search_control: SearchControlState,
    /// Whether the Narrow arrangement's Hero overlay is dismissed (design D4,
    /// D3): only the row menu's View Detail paints it again — a press or a pick
    /// never does — and Esc or a press on its backdrop closes it.
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
            filter_open: false,
            filter: String::new(),
            collections: None,
            shows_seasons: None,
            search_control: SearchControlState::default(),
            open_nodes: Vec::new(),
            filter_closed: Vec::new(),
            // No View Detail has happened yet: the Narrow arrangement shows
            // the list, not an overlay Hero for the core's default row.
            hero_dismissed: true,
            item_menu_open: false,
            row_menu: None,
        }
    }
}

/// The top bar's Search control's window facts (user request 2026-11-19).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchControlState {
    /// Whether the top bar is too narrow for the inline Search field: the
    /// bar's measure writes it, and the collapsed control reads it — the
    /// icon-and-panel form. A Reset UI State clears it, and the next measure
    /// rewrite restores it.
    pub collapsed: bool,
    /// Whether the collapsed Search control's panel paints: the icon toggles
    /// it, and a press outside it, Esc, and a result activation close it.
    /// `false` while the field paints inline.
    pub panel_open: bool,
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
    /// The active tab the carried Library list section last named (design D2
    /// of `gui-pointer-target-interaction`), `None` before the first carried
    /// section. The destination reset rides a change of this value, so the
    /// reset does not depend on which surface opened the destination.
    pub last_destination_tab: Signal<Option<Option<usize>>>,
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
            audiobooks_ui: Signal::new(super::audiobooks::AudiobooksUiState::initial()),
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
            last_destination_tab: Signal::new(None),
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

    /// Open the list's filter: `/` while the pointer rests over the Library
    /// column (design D2, step 3 of `gui-pointer-target-interaction`).
    pub fn open_library_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = true);
    }

    /// Close the filter with Esc: the text goes with it, so a closed filter
    /// narrows nothing (design D3, step 2). The scroll follows the widened
    /// rows, and the filter's own open and close over the nodes goes with the
    /// text.
    pub fn close_library_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.filter_open = false;
            ui.filter.clear();
            ui.filter_closed.clear();
        });
    }

    /// Accept the filter with Enter: the field closes and the list keeps the text
    /// (design D3, step 2).
    pub fn accept_library_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
    }

    /// Ask for a collection row to be pressed (gui-collection-dropdown design
    /// D5, D6): the All row and the Clear control send All, and every other key
    /// toggles its collection. The frame names the destination, so every
    /// collection destination shares this one press path.
    pub fn press_collection(&self, collections: &GuiCollections, key: &str) {
        self.press_collection_target(&collections.target, key);
    }

    /// Report the stable ids of the rows a destination paints, plus the
    /// shell's overscan (gui-delineated-rows design D7): the core resolves each
    /// id's row art source and queues its fetch once navigation is idle. The
    /// scope names the list whose rows these are, so a set of a list that is
    /// not the active destination queues nothing.
    pub fn report_rows_in_view(&self, scope: GuiRowArtScopeFrame, ids: Vec<String>) {
        self.link
            .library(GuiLibraryCommand::RowsInView { scope, ids });
    }

    /// Ask for a collection row of one destination `target` to be pressed
    /// (gui-collection-dropdown design D5): the dropdown's All row and
    /// checkbox rows capture the target alone, so a repaint clones no whole
    /// frame into a kept-alive handler.
    pub fn press_collection_target(&self, target: &GuiCollectionTarget, key: &str) {
        self.link
            .library(GuiCollections::pressed_target(target, key));
    }

    /// Set one flat list's sort (gui-flat-list-sort design D7): the command
    /// names the target and the chosen sort, and the list's scroll returns to
    /// the top, because the whole list reorders under it. The filter text
    /// stays: it narrows the reordered rows.
    pub fn select_list_sort(&self, target: GuiSortTarget, sort: GuiListSort) {
        self.link
            .library(GuiLibraryCommand::ListSort { target, sort });
        reset_scroll_top(self.sort_target_scroll(target));
    }

    /// The scroll store of the list `target` sorts (gui-flat-list-sort design
    /// D7): the plain Emby list's store serves its feed-group mode too, and
    /// the Shows New list shares the Shows destination's store.
    fn sort_target_scroll(&self, target: GuiSortTarget) -> Signal<ListScroll> {
        match target {
            GuiSortTarget::Library(_) => self.scrolls.emby,
            GuiSortTarget::Audiobooks(_) => self.scrolls.audiobooks,
            GuiSortTarget::Podcasts(_) => self.scrolls.podcasts,
            GuiSortTarget::Feeds => self.scrolls.feeds,
            GuiSortTarget::ShowsNew(_) => self.scrolls.shows,
        }
    }

    /// Select the Shows destination's one view (gui-collection-dropdown
    /// design D8): the command names the library and the view. The dropdown's
    /// own press handler closes the popover; this method sends only the
    /// selection, so a caller that reuses the open state cannot send it twice.
    pub fn select_shows_view(&self, lib_idx: usize, view: GuiShowsView) {
        self.link
            .library(GuiLibraryCommand::ShowsSetView { lib_idx, view });
    }

    /// Open or close the collection dropdown (design D7): a press on the
    /// trigger opens it when it is closed, and closes it when it is open. A
    /// close drops the filter text, so the next open starts empty.
    pub fn toggle_collections_dropdown(&self) {
        if self.collections_open() {
            self.close_collections_dropdown();
        } else {
            let mut ui = self.ui;
            ui.with_mut(|ui| ui.collections = Some(String::new()));
        }
    }

    /// Close the collection dropdown and clear its filter text (design D7): a
    /// press outside it, Esc, a destination change, and a second press on the
    /// trigger all land here. A press while it is closed already writes
    /// nothing: the window's every-press path calls this on each press, and the
    /// write would repaint the panel for no change.
    pub fn close_collections_dropdown(&self) {
        if !self.collections_open() {
            return;
        }
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.collections = None);
    }

    /// Whether the collection dropdown is open: `Some(filter)` is open, `None`
    /// is closed (design D7).
    #[must_use]
    pub fn collections_open(&self) -> bool {
        self.ui.peek().collections.is_some()
    }

    /// Type into the collection dropdown's filter (design D7): the entries
    /// narrow by the Library filter's match rule, and checks change nothing.
    pub fn set_collections_filter(&self, filter: String) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.collections = Some(filter));
    }

    /// Open or close the Shows Hero's season dropdown (user request
    /// 2026-11-19): a press on the trigger opens it when it is closed and
    /// closes it when it is open. A close drops the filter text, so the next
    /// open starts empty.
    pub fn toggle_shows_seasons_dropdown(&self) {
        if self.shows_seasons_open() {
            self.close_shows_seasons_dropdown();
        } else {
            let mut ui = self.ui;
            ui.with_mut(|ui| ui.shows_seasons = Some(String::new()));
        }
    }

    /// Close the Shows Hero's season dropdown and clear its filter text: a
    /// press outside it, Esc, a destination change, and a second press on the
    /// trigger all land here. A press while it is closed already writes
    /// nothing: the window's every-press path calls this on each press, and
    /// the write would repaint the panel for no change.
    pub fn close_shows_seasons_dropdown(&self) {
        if !self.shows_seasons_open() {
            return;
        }
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.shows_seasons = None);
    }

    /// Whether the Shows Hero's season dropdown is open: `Some(filter)` is
    /// open, `None` is closed.
    #[must_use]
    pub fn shows_seasons_open(&self) -> bool {
        self.ui.peek().shows_seasons.is_some()
    }

    /// Type into the Shows Hero's season dropdown's filter: the season rows
    /// narrow by the Library filter's match rule.
    pub fn set_shows_seasons_filter(&self, filter: String) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.shows_seasons = Some(filter));
    }

    /// Record the top bar's narrowness (user request 2026-11-19): the bar's
    /// measure effect writes its collapse decision on every change. A write
    /// the value already holds changes nothing, so a re-measure that reads the
    /// same width repaints nothing.
    pub fn set_search_collapsed(&self, collapsed: bool) {
        if self.search_collapsed() == collapsed {
            return;
        }
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.search_control.collapsed = collapsed;
            if !collapsed {
                // The inline field is back: the panel state is not its own.
                ui.search_control.panel_open = false;
            }
        });
    }

    /// Whether the top bar is too narrow for the inline Search field (user
    /// request 2026-11-19): the key router reads it at `Ctrl+K`, and the
    /// control paints the icon-and-panel form on it.
    #[must_use]
    pub fn search_collapsed(&self) -> bool {
        self.ui.peek().search_control.collapsed
    }

    /// Open the collapsed Search control's panel (user request 2026-11-19):
    /// the `Ctrl+K` chord on a collapsed bar, and the icon's press, land
    /// here. The field the panel paints mounts on the fact, so the keyboard
    /// arms the next frame.
    pub fn open_search_panel(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.search_control.panel_open = true);
    }

    /// Close the collapsed Search control's panel (user request 2026-11-19):
    /// a press outside the panel, Esc, a result activation, and a second
    /// press on the icon land here. A close while the field paints inline is
    /// a no-op.
    pub fn close_search_panel(&self) {
        if !self.search_panel_open() {
            return;
        }
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.search_control.panel_open = false);
    }

    /// Whether the collapsed Search control's panel paints (user request
    /// 2026-11-19).
    #[must_use]
    pub fn search_panel_open(&self) -> bool {
        self.ui.peek().search_control.panel_open
    }

    /// Toggle the collapsed Search control's panel (user request 2026-11-19):
    /// the icon's press.
    pub fn toggle_search_panel(&self) {
        if self.search_panel_open() {
            self.close_search_panel();
        } else {
            self.open_search_panel();
        }
    }

    /// Whether the Narrow arrangement paints the Hero overlay (design D4, D3):
    /// a picked item, an arrangement that overlays rather than splits, and no
    /// dismissal since the row menu's View Detail. The Hero is the Music Hero, the Home Hero, the
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

    /// Open the destination tab at `position`: the command goes to the core,
    /// and the carried `current_tab` the core answers with runs the
    /// destination reset ([`Self::settle_destination_tab`]), so this press and
    /// every other way of opening a destination share one reset.
    pub fn open_destination(&self, position: usize) {
        self.link
            .library(GuiLibraryCommand::OpenDestination { position });
    }

    /// Reset the state an open leaves behind (design D2 of
    /// `gui-pointer-target-interaction`): the core's default row is not a
    /// pick, so the Narrow arrangement shows the new destination's list and
    /// paints no overlay Hero, and an open row menu, item menu, or collection
    /// dropdown closes (gui-collection-dropdown design D7: a destination change
    /// closes the dropdown and clears its filter).
    pub fn reset_for_destination(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.hero_dismissed = true;
            ui.item_menu_open = false;
            ui.row_menu = None;
            ui.collections = None;
            ui.shows_seasons = None;
        });
    }

    /// Run the destination reset when the carried active tab changes from the
    /// last value the window saw (design D2 of
    /// `gui-pointer-target-interaction`): a press on a tab, a window digit, a
    /// router tab step, and the launch restore's open all change
    /// `current_tab`, so they share this one reset. The first value seen only
    /// sets the baseline, so a window start paints the restored destination as
    /// the launch left it.
    pub fn settle_destination_tab(&self, current_tab: Option<usize>) {
        let mut last = self.last_destination_tab;
        let previous = *last.peek();
        if previous == Some(current_tab) {
            return;
        }
        last.set(Some(current_tab));
        if previous.is_some() {
            self.reset_for_destination();
        }
    }

    /// Close the Narrow Hero overlay (design D3): Esc while it paints, and a press
    /// on its backdrop. A later View Detail paints it again.
    pub fn close_library_hero(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
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

    /// The row-menu identity of a Library destination row: the row's stable
    /// Service id, which the core resolves against its destination's stores.
    #[must_use]
    pub fn row_menu_item(item_id: String) -> mbv::GuiLibraryMenuRow {
        mbv::GuiLibraryMenuRow::Item(item_id)
    }

    /// Open the right-click item menu for `row` (the row-menu command): the
    /// next frame carries the row's full entries for the menu to paint at the
    /// pointer position, and a chosen entry sends its index for that row. The
    /// Hero's own menu closes with it — one menu is up at a time.
    pub fn open_library_row_menu(&self, row: mbv::GuiLibraryMenuRow, x: f32, y: f32) {
        self.open_row_menu(row, x, y, true);
    }

    fn open_row_menu(&self, row: mbv::GuiLibraryMenuRow, x: f32, y: f32, detail: bool) {
        self.link
            .library(GuiLibraryCommand::RowMenu { row: row.clone() });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.item_menu_open = false;
            ui.row_menu = Some(RowMenu { row, x, y, detail });
        });
    }

    /// Open a row's item menu from its right-click (design D8, D15): the row's own
    /// stable identity, or nothing when the row names no item — a letter heading, a spacer,
    /// or a node the Service does not resolve. Every destination's list opens its
    /// menu through this one call, so the pointer rule cannot drift between tabs.
    /// The Dioxus handler calls `prevent_default` itself and reads the event's
    /// client coordinates; the open-or-drop decision lives here.
    pub fn open_row_menu_at(&self, row: Option<mbv::GuiLibraryMenuRow>, x: f32, y: f32) {
        if let Some(row) = row {
            self.open_library_row_menu(row, x, y);
        }
    }

    /// Open the item menu of a row whose press picks no item (a Music artist
    /// row): the same menu without View Detail, because the Hero has no item
    /// to show for the row.
    pub fn open_row_menu_without_detail_at(
        &self,
        row: Option<mbv::GuiLibraryMenuRow>,
        x: f32,
        y: f32,
    ) {
        if let Some(row) = row {
            self.open_row_menu(row, x, y, false);
        }
    }

    /// View Detail, the row menu's entry (user request 2026-11-20): the menu
    /// closes and the Narrow Hero overlay paints for the row the right-click
    /// selected. It is the only way the overlay opens.
    pub fn view_library_detail(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.hero_dismissed = false;
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
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
                row: row_menu.row,
                index,
            });
        }
    }
}

mod rows;

#[cfg(test)]
mod tests;
