//! The lettered destinations' one row model (`gui-tech-debt-simplification`
//! design D4): Emby, Audiobooks, Podcasts, and Feeds paint the same lettered
//! list over their own carried rows and their own list, detail, and
//! interaction state. [`LetteredDestination`] names those parts with four
//! zero-sized adapters, so the row model and the list's state methods are one
//! generic body and each destination keeps only its data. The headings
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
use mbv::{GuiCollections, GuiListSortFrame};

use crate::bridge::frame::library::{
    GuiAudiobooksRowFrame, GuiEmbyRowFrame, GuiEntryRowFrame, GuiLibraryDetail, GuiLibraryList,
};
use crate::bridge::library::GuiRowArtScopeFrame;
use crate::window::library::list::{
    GeometryRow, LIBRARY_ROW_HEIGHT, LetteredRow, ListMove, SortLetter, build_lettered_rows,
    filter_match, jump_to_letter, lettered_rail, scroll_step,
};
use crate::window::library::scroll::{ListScroll, set_scroll_top};
use crate::window::pointer_target::PointerList;
use crate::window::service_mark::ServiceMark;

use super::list_state::ListUiState;
use super::state::LibraryState;

mod adapters;

pub use adapters::{AudiobooksList, EmbyList, FeedsList, PodcastsList};

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

/// The notice lines a lettered list paints above its rows (design D4): the
/// load line every destination paints with the same text, and the failure and
/// empty lines, each with its own attribute and text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LetteredNotices {
    /// The load line's attribute.
    pub loading_attr: &'static str,
    /// The failure line's attribute.
    pub failed_attr: &'static str,
    /// The empty line's attribute.
    pub empty_attr: &'static str,
    /// The failure line's text.
    pub failed_text: &'static str,
    /// The empty line's text.
    pub empty_text: &'static str,
}

/// The load facts a lettered list part carries (design D4): each notice line
/// paints while its fact holds. The empty line paints only over a landed row
/// set, so a landing list shows the load line alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LetteredLoadFacts {
    /// Whether the list has not landed yet.
    pub loading: bool,
    /// Whether the list's fetch failed.
    pub failed: bool,
    /// Whether the landed list holds no row.
    pub empty: bool,
}

impl LetteredLoadFacts {
    /// The load facts of a part that says whether it loads, whether it
    /// failed, and whether it holds no row: the empty line is the fact that
    /// holds over a landed row set alone.
    #[must_use]
    pub fn of(loading: bool, failed: bool, no_rows: bool) -> Self {
        Self {
            loading,
            failed,
            empty: !loading && !failed && no_rows,
        }
    }
}

/// The facts a lettered list's header paints (design D4): the Service mark or
/// title that names the destination, the selection label where one paints,
/// the collection selector and sort control where they paint, and the `/`
/// filter field's texts. The filter text and the field's callbacks are the
/// list's, not the header's, so they do not ride here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetteredHeader {
    /// The mark that names the destination at the heading line's left end.
    pub mark: ServiceMark,
    /// The carried selection's label: empty where the destination paints
    /// none.
    pub label: String,
    /// The destination's own label attribute, when it has one.
    pub label_attr: Option<&'static str>,
    /// The destination's collection selector, when it has one.
    pub collections: Option<GuiCollections>,
    /// The destination's sort control, when its list has one.
    pub sort: Option<GuiListSortFrame>,
    /// The filter field's placeholder, which names the selection it narrows.
    pub placeholder: String,
    /// The filter field's accessible name.
    pub aria_label: &'static str,
}

/// One of the Library's lettered destinations (design D4): the plain Emby
/// library, Audiobooks, Podcasts, or Feeds. A destination is a zero-sized
/// adapter that names its carried row, its list and detail parts, its
/// interaction state, its scroll store, and its own painted data, so the row
/// model, the list's wiring, and the list component are one generic body over
/// the four. The adapters compare and copy, so the component names one as its
/// type parameter.
pub trait LetteredDestination: Sized + Copy + PartialEq + 'static {
    /// The carried row the destination paints.
    type Row: CarriedRow + Clone;

    /// The destination's list part.
    type List;

    /// The destination's detail part.
    type Detail;

    /// The attribute word the destination's DOM hooks carry (design D4): the
    /// list column (`data-library-<word>-list-column`), the scroll shell's
    /// list and row hooks, the letter headings
    /// (`data-library-<word>-letter`), and the item rows.
    const ATTR: &'static str;

    /// The list's padding, as the scroll shell applies it.
    const PADDING: &'static str;

    /// The scope the list reports its rows in view under, so the core queues
    /// their row art (gui-delineated-rows design D7).
    const ART_SCOPE: GuiRowArtScopeFrame;

    /// The list kind the destination's scroller carries as its pointer marker
    /// (`gui-pointer-target-interaction` D1).
    const POINTER_LIST: PointerList;

    /// The destination's notice lines and their texts (design D4).
    const NOTICES: LetteredNotices;

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

    /// The row the list paints raised: the core's selected row in `detail`,
    /// as the frame painted it.
    fn selected(detail: &Self::Detail) -> Option<&str>;

    /// Whether the detail part carries a Hero (design D12): the Hero's
    /// actions send only while the frame holds both the selection and a
    /// Hero.
    fn hero_present(detail: &Self::Detail) -> bool;

    /// The load facts the destination's list part carries (design D4): the
    /// load line, the failure line, and the empty line each paint while their
    /// fact holds.
    fn load_facts(list: &Self::List) -> LetteredLoadFacts;

    /// The header the destination paints above its rows: its mark, its
    /// selection label, its collection selector, its sort control, and its
    /// filter field's texts.
    fn header(list: Option<&Self::List>) -> LetteredHeader;

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

    /// One painted row of the list (design D4): the letter heading, or the
    /// item row the destination paints. The destination's own paint code, so
    /// the component paints every row through this one call.
    fn paint_row(
        row: &LetteredRow<LetteredItemRow<Self::Row>>,
        library: &LibraryState,
        selected: Option<&str>,
    ) -> Element;

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

#[cfg(test)]
mod tests;
