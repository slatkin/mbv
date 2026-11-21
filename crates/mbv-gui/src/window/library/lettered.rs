//! The lettered destinations' one row model (`gui-tech-debt-simplification`
//! design D4): Emby, Audiobooks, Podcasts, and Feeds paint the same lettered
//! list over their own carried rows and their own list, detail, and
//! interaction state. [`LetteredDestination`] names those parts with four
//! zero-sized adapters, so the row model, the row memo, and the list's wiring
//! are one generic body and each destination keeps only its data. The headings
//! and the rail stay one fact the core derives (gui-flat-list-sort design D6,
//! D8): a name sort paints one heading per run of the same letter and a date
//! sort paints none, as [`build_lettered_rows`] builds them.
//!
//! The row model is plain data over fixed heights, so the geometry is asserted
//! without a layout. Home and Shows keep their own row modules: Home's rows
//! carry headings and spacers, and Shows' rows carry the New and Upcoming
//! modes (design D4).

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiAudiobooksRowFrame, GuiEmbyDetail, GuiEmbyList,
    GuiEmbyRowFrame, GuiEntryRowFrame, GuiFeedsDetail, GuiFeedsList, GuiLibraryDetail,
    GuiLibraryList, GuiPodcastsDetail, GuiPodcastsList,
};
use crate::window::library::list::{
    GeometryRow, LIBRARY_ROW_HEIGHT, LetteredRow, ListMove, SortLetter, build_lettered_rows,
    filter_match, jump_to_letter, lettered_rail, scroll_step,
};
use crate::window::library::parts::{
    library_audiobooks_detail, library_audiobooks_list, library_emby_detail, library_emby_list,
    library_feeds_detail, library_feeds_list, library_podcasts_detail, library_podcasts_list,
};
use crate::window::library::scroll::{ListScroll, set_scroll_top};

use super::list_state::ListUiState;
use super::state::LibraryState;

/// The entry row's height in logical pixels (gui-delineated-rows design D2):
/// the shared top-level row part's height, which paints the entry's two-line
/// title and its date gutter.
pub const FEEDS_ROW_HEIGHT: f32 = LIBRARY_ROW_HEIGHT;

/// The episode row's height in logical pixels (gui-delineated-rows design D2):
/// the shared top-level row part's height, which paints the episode's
/// two-line title and its date gutter.
pub const PODCASTS_ROW_HEIGHT: f32 = LIBRARY_ROW_HEIGHT;

/// The facts the shared lettered builder reads from a destination's carried
/// row (design D4): the row's stable identity, the title its filter matches,
/// and the rail letter of its sort key. Each destination's carried row frame
/// implements it, so the builder never names a frame.
pub trait CarriedRow {
    /// The row's stable identity, as the carried frame names it.
    fn id(&self) -> &str;

    /// The row's primary label: the filter narrows on it.
    fn title(&self) -> &str;

    /// The row's rail letter, `None` under a date sort.
    fn sort_letter(&self) -> Option<char>;
}

impl CarriedRow for GuiEmbyRowFrame {
    fn id(&self) -> &str {
        &self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn sort_letter(&self) -> Option<char> {
        self.sort_letter
    }
}

impl CarriedRow for GuiAudiobooksRowFrame {
    fn id(&self) -> &str {
        &self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn sort_letter(&self) -> Option<char> {
        self.sort_letter
    }
}

impl CarriedRow for GuiEntryRowFrame {
    fn id(&self) -> &str {
        &self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn sort_letter(&self) -> Option<char> {
        self.sort_letter
    }
}

/// One painted item row of a lettered list (design D4): its stable id, its top
/// edge and height, and the carried row it paints. A heading is structural and
/// stays a [`LetteredRow::Heading`]. The height is the destination's
/// [`LetteredDestination::ROW_HEIGHT`], so the list's geometry and its painted
/// rows agree.
#[derive(Debug, Clone, PartialEq)]
pub struct LetteredItemRow<Row> {
    /// The row's stable key: the pointer marker's value, the each-block's key,
    /// and the id the scroll geometry reads.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The carried row the item paints.
    pub row: Row,
}

impl<Row: CarriedRow> GeometryRow for LetteredItemRow<Row> {
    fn id(&self) -> &str {
        &self.id
    }

    fn top(&self) -> f32 {
        self.top
    }

    fn height(&self) -> f32 {
        self.height
    }
}

impl<Row: CarriedRow> SortLetter for LetteredItemRow<Row> {
    fn sort_letter(&self) -> Option<char> {
        self.row.sort_letter()
    }
}

/// One of the Library's lettered destinations (design D4): the plain Emby
/// library, Audiobooks, Podcasts, or Feeds. A destination is a zero-sized
/// adapter that names its carried row, its list and detail parts, its
/// interaction state, and its scroll store, so the row model and the list's
/// wiring are one generic body over the four. The adapters compare and copy,
/// so a generic component takes one as a prop.
pub trait LetteredDestination: Sized + Copy + PartialEq + 'static {
    /// The carried row the destination paints.
    type Row: CarriedRow + Clone;

    /// The destination's list part.
    type List;

    /// The destination's detail part.
    type Detail;

    /// The heading id prefix (gui-flat-list-sort design D8): each
    /// destination's letter headings keep their own hook.
    const HEADING_KEY: &'static str;

    /// The painted row height in logical pixels (gui-delineated-rows design
    /// D2): the shared top-level row part's height.
    const ROW_HEIGHT: f32 = LIBRARY_ROW_HEIGHT;

    /// The active destination's list part, or `None` while another
    /// destination holds the window.
    fn list(list: Option<&GuiLibraryList>) -> Option<&Self::List>;

    /// The active destination's detail part, or `None` while another
    /// destination holds the window.
    fn detail(detail: Option<&GuiLibraryDetail>) -> Option<&Self::Detail>;

    /// The destination's filter state: every lettered destination's `/`
    /// filter is the one [`ListUiState`] text, so the generic filter, scroll,
    /// letter-jump, measure, and scroll methods read and write it here.
    fn ui(state: &LibraryState) -> Signal<ListUiState>;

    /// The destination's scroll store (design D9 of
    /// `gui-change-driven-sections`).
    fn scroll(state: &LibraryState) -> Signal<ListScroll>;

    /// The destination's painted rows under its filter's current text: the
    /// carried rows narrowed by [`ListUiState::filter`], under the letter
    /// headings a name sort paints (design D4).
    fn painted_rows(state: &LibraryState) -> Arc<[LetteredRow<LetteredItemRow<Self::Row>>]>;

    /// Select the painted row (design D4, D12): the select command the core
    /// stores, whose frame paints the Hero. A heading names no row, so its
    /// select changes nothing.
    fn select(state: &LibraryState, row: &LetteredRow<LetteredItemRow<Self::Row>>);

    /// Play the item `item_id` names: Enter on the hovered row, and a
    /// double-click's play.
    fn play(state: &LibraryState, item_id: &str);

    /// Add the item `item_id` names to the queue's tail: `a` on the hovered
    /// row.
    fn queue(state: &LibraryState, item_id: &str);

    /// Play the item `item_id` names next: `n` on the hovered row. Only Emby
    /// overrides this; a destination with no play-next sends nothing, so `n`
    /// is consumed and does nothing there.
    fn play_next(state: &LibraryState, item_id: &str) {
        let _ = (state, item_id);
    }
}

/// The plain Emby-library destination's adapter (design D4): its carried rows
/// are the Emby row frames, and its list, detail, ui, and scroll are the Emby
/// ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbyList;

impl LetteredDestination for EmbyList {
    type Row = GuiEmbyRowFrame;
    type List = GuiEmbyList;
    type Detail = GuiEmbyDetail;

    const HEADING_KEY: &'static str = "emby-letter";

    fn list(list: Option<&GuiLibraryList>) -> Option<&Self::List> {
        library_emby_list(list)
    }

    fn detail(detail: Option<&GuiLibraryDetail>) -> Option<&Self::Detail> {
        library_emby_detail(detail)
    }

    fn ui(state: &LibraryState) -> Signal<ListUiState> {
        state.emby_ui
    }

    fn scroll(state: &LibraryState) -> Signal<ListScroll> {
        state.scrolls.emby
    }

    fn painted_rows(state: &LibraryState) -> Arc<[LetteredRow<LetteredItemRow<Self::Row>>]> {
        state.emby_rows()
    }

    fn select(state: &LibraryState, row: &LetteredRow<LetteredItemRow<Self::Row>>) {
        state.select_emby_row(row);
    }

    fn play(state: &LibraryState, item_id: &str) {
        state.play_emby_row(item_id);
    }

    fn queue(state: &LibraryState, item_id: &str) {
        state.queue_emby_row(item_id);
    }

    fn play_next(state: &LibraryState, item_id: &str) {
        state.play_emby_row_next(item_id);
    }
}

/// The Audiobooks destination's adapter (design D4): its carried rows are the
/// book row frames, and its list, detail, ui, and scroll are the Audiobooks
/// ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudiobooksList;

impl LetteredDestination for AudiobooksList {
    type Row = GuiAudiobooksRowFrame;
    type List = GuiAudiobooksList;
    type Detail = GuiAudiobooksDetail;

    const HEADING_KEY: &'static str = "abs-letter";

    fn list(list: Option<&GuiLibraryList>) -> Option<&Self::List> {
        library_audiobooks_list(list)
    }

    fn detail(detail: Option<&GuiLibraryDetail>) -> Option<&Self::Detail> {
        library_audiobooks_detail(detail)
    }

    fn ui(state: &LibraryState) -> Signal<ListUiState> {
        state.audiobooks_ui
    }

    fn scroll(state: &LibraryState) -> Signal<ListScroll> {
        state.scrolls.audiobooks
    }

    fn painted_rows(state: &LibraryState) -> Arc<[LetteredRow<LetteredItemRow<Self::Row>>]> {
        let filter = state.audiobooks_ui.peek().filter.clone();
        state.audiobooks_rows(&filter)
    }

    fn select(state: &LibraryState, row: &LetteredRow<LetteredItemRow<Self::Row>>) {
        state.select_audiobooks_row(row);
    }

    fn play(state: &LibraryState, item_id: &str) {
        state.play_audiobooks_row(item_id);
    }

    fn queue(state: &LibraryState, item_id: &str) {
        state.queue_audiobooks_row(item_id);
    }
}

/// The Podcasts destination's adapter (design D4): its carried rows are the
/// entry row frames, and its list, detail, ui, and scroll are the Podcasts
/// ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PodcastsList;

impl LetteredDestination for PodcastsList {
    type Row = GuiEntryRowFrame;
    type List = GuiPodcastsList;
    type Detail = GuiPodcastsDetail;

    const HEADING_KEY: &'static str = "podcasts-letter";

    fn list(list: Option<&GuiLibraryList>) -> Option<&Self::List> {
        library_podcasts_list(list)
    }

    fn detail(detail: Option<&GuiLibraryDetail>) -> Option<&Self::Detail> {
        library_podcasts_detail(detail)
    }

    fn ui(state: &LibraryState) -> Signal<ListUiState> {
        state.podcasts_ui
    }

    fn scroll(state: &LibraryState) -> Signal<ListScroll> {
        state.scrolls.podcasts
    }

    fn painted_rows(state: &LibraryState) -> Arc<[LetteredRow<LetteredItemRow<Self::Row>>]> {
        state.podcasts_rows()
    }

    fn select(state: &LibraryState, row: &LetteredRow<LetteredItemRow<Self::Row>>) {
        if let LetteredRow::Item(item) = row {
            state.select_podcasts_row(item);
        }
    }

    fn play(state: &LibraryState, item_id: &str) {
        state.play_podcasts_row(item_id);
    }

    fn queue(state: &LibraryState, item_id: &str) {
        state.queue_podcasts_row(item_id);
    }
}

/// The Feeds destination's adapter (design D4): its carried rows are the entry
/// row frames, and its list, detail, ui, and scroll are the Feeds ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedsList;

impl LetteredDestination for FeedsList {
    type Row = GuiEntryRowFrame;
    type List = GuiFeedsList;
    type Detail = GuiFeedsDetail;

    const HEADING_KEY: &'static str = "feeds-letter";

    fn list(list: Option<&GuiLibraryList>) -> Option<&Self::List> {
        library_feeds_list(list)
    }

    fn detail(detail: Option<&GuiLibraryDetail>) -> Option<&Self::Detail> {
        library_feeds_detail(detail)
    }

    fn ui(state: &LibraryState) -> Signal<ListUiState> {
        state.feeds_ui
    }

    fn scroll(state: &LibraryState) -> Signal<ListScroll> {
        state.scrolls.feeds
    }

    fn painted_rows(state: &LibraryState) -> Arc<[LetteredRow<LetteredItemRow<Self::Row>>]> {
        state.feeds_rows()
    }

    fn select(state: &LibraryState, row: &LetteredRow<LetteredItemRow<Self::Row>>) {
        if let LetteredRow::Item(item) = row {
            state.select_feeds_row(item);
        }
    }

    fn play(state: &LibraryState, item_id: &str) {
        state.play_feeds_row(item_id);
    }

    fn queue(state: &LibraryState, item_id: &str) {
        state.queue_feeds_row(item_id);
    }
}

/// The one set of `LibraryState` methods the lettered destinations share
/// (design D4): a destination's `/` filter, its scroll step, its rail jump, its
/// measured viewport, and its native scroll offset all land on the state the
/// adapter names, so each of the four keeps only its data. The rows each method
/// reads are the destination's own [`LetteredDestination::painted_rows`].
impl LibraryState {
    /// Narrow destination `D`'s list to the titles that contain `filter`,
    /// without case (design D7's filter rule). The filter narrows the rows and
    /// selects nothing, and the scroll stays inside the narrower list.
    pub fn set_lettered_filter<D: LetteredDestination>(&self, filter: String) {
        D::ui(self).with_mut(|ui| ui.filter = filter);
        self.clamp_lettered_scroll::<D>();
    }

    /// Close destination `D`'s filter with Esc (design D3, step 2): the text
    /// goes with it, so a closed filter narrows nothing. The scroll follows the
    /// widened rows.
    pub fn close_lettered_filter<D: LetteredDestination>(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        D::ui(self).with_mut(|ui| ui.filter.clear());
        self.clamp_lettered_scroll::<D>();
    }

    /// Scroll destination `D`'s list by one movement key (design D6): Up and
    /// Down move by the height of the row at the top edge, Page Up and Page
    /// Down by the viewport height, and Home and End go to the list's ends,
    /// inside the list's range.
    pub fn step_lettered_scroll<D: LetteredDestination>(&self, mv: ListMove) {
        let rows = D::painted_rows(self);
        let held = *D::scroll(self).peek();
        set_scroll_top(D::scroll(self), &rows, scroll_step(&rows, held, mv));
    }

    /// Jump to a letter of destination `D`'s rail (design D7; gui-flat-list-sort
    /// design D8): the list scrolls so the first row under the letter paints
    /// fully. A letter without rows changes nothing.
    pub fn jump_lettered_to_letter<D: LetteredDestination>(&self, letter: char) {
        let rows = D::painted_rows(self);
        let view = *D::scroll(self).peek();
        let Some((_, scroll_top)) = jump_to_letter(&lettered_rail(&rows), &rows, letter, view)
        else {
            return;
        };
        set_scroll_top(D::scroll(self), &rows, scroll_top);
    }

    /// Report destination `D`'s measured viewport height: the rows in view
    /// follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_lettered_list<D: LetteredDestination>(&self, viewport_height: f32) {
        let mut scroll = D::scroll(self);
        scroll.with_mut(|scroll| scroll.viewport_height = viewport_height);
        self.clamp_lettered_scroll::<D>();
    }

    /// Report destination `D`'s native scroll offset: a user scroll moves the
    /// rows in view, and the offset stays inside the list's range.
    pub fn scroll_lettered_list<D: LetteredDestination>(&self, scroll_top: f32) {
        let rows = D::painted_rows(self);
        set_scroll_top(D::scroll(self), &rows, scroll_top);
    }

    /// Hold destination `D`'s scroll offset inside the rows the filter leaves.
    fn clamp_lettered_scroll<D: LetteredDestination>(&self) {
        let rows = D::painted_rows(self);
        let held = *D::scroll(self).peek();
        set_scroll_top(D::scroll(self), &rows, held.scroll_top);
    }
}

/// The painted rows of a lettered destination's list (design D4;
/// gui-flat-list-sort design D8): the carried rows narrowed by the filter,
/// each under the letter heading of its rail letter — one heading per run of
/// the same letter. A date sort's rows carry no letter, so they paint flat,
/// with no heading and no rail. Every row carries its top edge and height, so
/// the list paints a window of rows over native scroll.
#[must_use]
pub fn build_rows<D: LetteredDestination>(
    rows: &[D::Row],
    filter: &str,
) -> Vec<LetteredRow<LetteredItemRow<D::Row>>> {
    build_lettered_rows(
        rows,
        D::ROW_HEIGHT,
        D::HEADING_KEY,
        |row| filter_match(row.title(), filter),
        CarriedRow::sort_letter,
        |row, top, height| LetteredItemRow {
            id: row.id().to_owned(),
            row: row.clone(),
            top,
            height,
        },
    )
}

/// The rows the memo handed back last, with the cheap key and the interaction
/// facts they were built from. The rows sit behind an `Arc` because the former
/// client semantics the memo preserves are shared state: one scroll step reads
/// the same rows the last one built, and the list repaints only its visible
/// window.
#[derive(Debug, Clone)]
struct EmbyMemoEntry {
    /// The `libraryList` section revision the rows came from (design D6): a
    /// section's value at one revision is immutable, so equal revisions carry
    /// equal rows and the hit test never compares the union.
    revision: Option<u64>,
    filter: String,
    rows: Arc<[LetteredRow<LetteredItemRow<GuiEmbyRowFrame>>]>,
}

/// The Emby rows, memoized on the carried rows and the filter (design D9 of
/// `gui-change-driven-sections`). One slot, because a list paints one content
/// at a time.
#[derive(Clone, Default)]
pub struct EmbyRowsMemo {
    last: Signal<Option<EmbyMemoEntry>>,
}

impl std::fmt::Debug for EmbyRowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbyRowsMemo").finish_non_exhaustive()
    }
}

impl EmbyRowsMemo {
    /// The rows `build_rows::<EmbyList>` leaves over the carried rows and the
    /// interaction facts: the ones it built last while the arguments are the
    /// same, and a fresh build otherwise. The rows come keyed on the
    /// `libraryList` section revision they borrowed from (design D6); the
    /// caller must pass the slice the section's current value carries.
    #[must_use]
    pub fn rows(
        &self,
        revision: Option<u64>,
        rows: &[GuiEmbyRowFrame],
        filter: &str,
    ) -> Arc<[LetteredRow<LetteredItemRow<GuiEmbyRowFrame>>]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a filter's scroll clamp) would re-queue itself on the write
        // below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.filter == filter
        {
            return Arc::clone(&entry.rows);
        }
        let built: Arc<[LetteredRow<LetteredItemRow<GuiEmbyRowFrame>>]> =
            build_rows::<EmbyList>(rows, filter).into();
        let mut last = self.last;
        last.set(Some(EmbyMemoEntry {
            revision,
            filter: filter.to_owned(),
            rows: Arc::clone(&built),
        }));
        built
    }
}

/// The rows the memo handed back last, with the cheap key and the interaction
/// facts they were built from. The rows sit behind an `Arc` because the former
/// client semantics the memo preserves are shared state: one scroll step reads
/// the same rows the last one built, and the list repaints only its visible
/// window.
#[derive(Debug, Clone)]
struct AudiobooksMemoEntry {
    /// The `libraryList` section revision the rows came from (design D6): a
    /// section's value at one revision is immutable, so equal revisions carry
    /// equal rows and the hit test never compares the union.
    revision: Option<u64>,
    filter: String,
    rows: Arc<[LetteredRow<LetteredItemRow<GuiAudiobooksRowFrame>>]>,
}

/// The Audiobooks rows, memoized on the carried rows and the filter (design
/// D9 of `gui-change-driven-sections`). One slot, because a list paints one
/// content at a time.
#[derive(Clone, Default)]
pub struct AudiobooksRowsMemo {
    last: Signal<Option<AudiobooksMemoEntry>>,
}

impl std::fmt::Debug for AudiobooksRowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudiobooksRowsMemo").finish_non_exhaustive()
    }
}

impl AudiobooksRowsMemo {
    /// The rows `build_rows::<AudiobooksList>` leaves over the carried rows
    /// and the filter: the ones it built last while the arguments are the
    /// same, and a fresh build otherwise. The rows come keyed on the
    /// `libraryList` section revision they borrowed from (design D6); the
    /// caller must pass the slice the section's current value carries.
    #[must_use]
    pub fn rows(
        &self,
        revision: Option<u64>,
        rows: &[GuiAudiobooksRowFrame],
        filter: &str,
    ) -> Arc<[LetteredRow<LetteredItemRow<GuiAudiobooksRowFrame>>]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a filter's scroll clamp) would re-queue itself on the write
        // below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.filter == filter
        {
            return Arc::clone(&entry.rows);
        }
        let built: Arc<[LetteredRow<LetteredItemRow<GuiAudiobooksRowFrame>>]> =
            build_rows::<AudiobooksList>(rows, filter).into();
        let mut last = self.last;
        last.set(Some(AudiobooksMemoEntry {
            revision,
            filter: filter.to_owned(),
            rows: Arc::clone(&built),
        }));
        built
    }
}

/// The rows the memo handed back last, with the cheap key and the interaction
/// facts they were built from. The rows sit behind an `Arc` because the former
/// client semantics the memo preserves are shared state: one scroll step reads
/// the same rows the last one built, and the list repaints only its visible
/// window.
#[derive(Debug, Clone)]
struct PodcastsMemoEntry {
    /// The `libraryList` section revision the rows came from (design D6): a
    /// section's value at one revision is immutable, so equal revisions carry
    /// equal rows and the hit test never compares the union.
    revision: Option<u64>,
    filter: String,
    rows: Arc<[LetteredRow<LetteredItemRow<GuiEntryRowFrame>>]>,
}

/// The Podcasts rows, memoized on the carried rows and the filter (design D9
/// of `gui-change-driven-sections`). One slot, because a list paints one
/// content at a time.
#[derive(Clone, Default)]
pub struct PodcastsRowsMemo {
    last: Signal<Option<PodcastsMemoEntry>>,
}

impl std::fmt::Debug for PodcastsRowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodcastsRowsMemo").finish_non_exhaustive()
    }
}

impl PodcastsRowsMemo {
    /// The rows `build_rows::<PodcastsList>` leaves over the carried rows and
    /// the filter: the ones it built last while the arguments are the same,
    /// and a fresh build otherwise. The rows come keyed on the `libraryList`
    /// section revision they borrowed from (design D6); the caller must pass
    /// the slice the section's current value carries.
    #[must_use]
    pub fn rows(
        &self,
        revision: Option<u64>,
        rows: &[GuiEntryRowFrame],
        filter: &str,
    ) -> Arc<[LetteredRow<LetteredItemRow<GuiEntryRowFrame>>]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a filter's scroll clamp) would re-queue itself on the write
        // below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.filter == filter
        {
            return Arc::clone(&entry.rows);
        }
        let built: Arc<[LetteredRow<LetteredItemRow<GuiEntryRowFrame>>]> =
            build_rows::<PodcastsList>(rows, filter).into();
        let mut last = self.last;
        last.set(Some(PodcastsMemoEntry {
            revision,
            filter: filter.to_owned(),
            rows: Arc::clone(&built),
        }));
        built
    }
}

/// The rows the memo handed back last, with the cheap key and the interaction
/// facts they were built from. The rows sit behind an `Arc` because the former
/// client semantics the memo preserves are shared state: one scroll step reads
/// the same rows the last one built, and the list repaints only its visible
/// window.
#[derive(Debug, Clone)]
struct FeedsMemoEntry {
    /// The `libraryList` section revision the rows came from (design D6): a
    /// section's value at one revision is immutable, so equal revisions carry
    /// equal rows and the hit test never compares the union.
    revision: Option<u64>,
    filter: String,
    rows: Arc<[LetteredRow<LetteredItemRow<GuiEntryRowFrame>>]>,
}

/// The Feeds rows, memoized on the carried rows and the filter (design D9 of
/// `gui-change-driven-sections`). One slot, because a list paints one content
/// at a time.
#[derive(Clone, Default)]
pub struct FeedsRowsMemo {
    last: Signal<Option<FeedsMemoEntry>>,
}

impl std::fmt::Debug for FeedsRowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeedsRowsMemo").finish_non_exhaustive()
    }
}

impl FeedsRowsMemo {
    /// The rows `build_rows::<FeedsList>` leaves over the carried rows and the
    /// filter: the ones it built last while the arguments are the same, and a
    /// fresh build otherwise. The rows come keyed on the `libraryList` section
    /// revision they borrowed from (design D6); the caller must pass the slice
    /// the section's current value carries.
    #[must_use]
    pub fn rows(
        &self,
        revision: Option<u64>,
        rows: &[GuiEntryRowFrame],
        filter: &str,
    ) -> Arc<[LetteredRow<LetteredItemRow<GuiEntryRowFrame>>]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a filter's scroll clamp) would re-queue itself on the write
        // below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.filter == filter
        {
            return Arc::clone(&entry.rows);
        }
        let built: Arc<[LetteredRow<LetteredItemRow<GuiEntryRowFrame>>]> =
            build_rows::<FeedsList>(rows, filter).into();
        let mut last = self.last;
        last.set(Some(FeedsMemoEntry {
            revision,
            filter: filter.to_owned(),
            rows: Arc::clone(&built),
        }));
        built
    }
}

#[cfg(test)]
mod tests;
