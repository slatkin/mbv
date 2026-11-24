//! The lettered destinations' four adapters (`gui-tech-debt-simplification`
//! design D4): the plain Emby library, Audiobooks, Podcasts, and Feeds. Each
//! adapter names its carried row, its list and detail parts, its DOM hooks,
//! its header facts, its notice texts, and its row painter, so
//! [`LetteredDestination`] has one generic body over the four and each
//! destination keeps only its data and its paint code.
//!
//! The row painters paint the shared top-level row part: each destination's
//! rows keep their attributes, their kinds, and their trail parts, and the two
//! entry lists pass their carried play state so the part derives the title's
//! tone and the thumb's run. The press and the double-click ride the adapter's
//! command methods, so the generic component calls one press path for all
//! four. The rows' hover controls ride those same methods
//! (`gui-row-hover-actions` design D3, D4), and only an Emby `Collection`
//! row's Shuffle reaches the row menu's own shuffle entry.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiAudiobooksRowFrame, GuiCollectionsFrame as _,
    GuiEmbyDetail, GuiEmbyList, GuiEmbyRowFrame, GuiEmbyRowKindFrame, GuiEntryRowFrame,
    GuiFeedsDetail, GuiFeedsList, GuiLibraryDetail, GuiLibraryList, GuiPodcastsDetail,
    GuiPodcastsList,
};
use crate::bridge::library::GuiRowArtScopeFrame;
use crate::window::components::library_letter_heading::paint_letter_heading;
use crate::window::components::list_row::{FlatRow, LibraryRowThumb, LibraryTrailPart};
use crate::window::library::list::LetteredRow;
use crate::window::library::list_state::ListUiState;
use crate::window::library::parts::{
    library_audiobooks_detail, library_audiobooks_list, library_emby_detail, library_emby_list,
    library_feeds_detail, library_feeds_list, library_podcasts_detail, library_podcasts_list,
};
use crate::window::library::scroll::ListScroll;
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::service_mark::ServiceMark;

use super::{
    LetteredDestination, LetteredHeader, LetteredItemRow, LetteredLoadFacts, LetteredNotices,
};

/// The plain Emby-library destination's adapter (design D4): its carried rows
/// are the Emby row frames, and its list, detail, ui, and scroll are the Emby
/// ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbyList;

impl LetteredDestination for EmbyList {
    type Row = GuiEmbyRowFrame;
    type List = GuiEmbyList;
    type Detail = GuiEmbyDetail;

    const ATTR: &'static str = "emby";
    const PADDING: &'static str = "8px 0 8px 8px";
    const ART_SCOPE: GuiRowArtScopeFrame = GuiRowArtScopeFrame::Emby;
    const POINTER_LIST: PointerList = PointerList::Emby;
    const NOTICES: LetteredNotices = LetteredNotices {
        loading_attr: "data-library-emby-loading",
        failed_attr: "data-library-emby-failed",
        empty_attr: "data-library-emby-empty",
        failed_text: "Couldn’t load this library.",
        empty_text: "No items",
    };
    const HEADING_KEY: &'static str = "emby-letter";

    fn list(list: Option<&GuiLibraryList>) -> Option<&Self::List> {
        library_emby_list(list)
    }

    fn detail(detail: Option<&GuiLibraryDetail>) -> Option<&Self::Detail> {
        library_emby_detail(detail)
    }

    fn selected(detail: &Self::Detail) -> Option<&str> {
        detail.selected.as_deref()
    }

    fn hero_present(detail: &Self::Detail) -> bool {
        detail.hero.is_some()
    }

    fn load_facts(list: &Self::List) -> LetteredLoadFacts {
        LetteredLoadFacts::of(list.loading, list.failed, list.rows.is_empty())
    }

    fn header(list: Option<&Self::List>) -> LetteredHeader {
        let selection_names = list
            .map(|list| list.collection_names.clone())
            .unwrap_or_default();
        let placeholder = if selection_names.is_empty() {
            "Filter".to_owned()
        } else {
            format!("Filter {}", selection_names.join(" + "))
        };
        LetteredHeader {
            mark: ServiceMark::Emby,
            label: String::new(),
            label_attr: None,
            collections: list.and_then(|list| list.collections.clone()),
            sort: list.and_then(|list| list.sort.clone()),
            placeholder,
            aria_label: "Filter items",
        }
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

    fn paint_row(
        row: &LetteredRow<LetteredItemRow<Self::Row>>,
        library: &LibraryState,
        selected: Option<&str>,
    ) -> Element {
        paint_emby_row(row, library, selected)
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
}

/// The row kind word the item row's hook paints (`data-library-emby-row`).
fn kind_word(kind: GuiEmbyRowKindFrame) -> &'static str {
    match kind {
        GuiEmbyRowKindFrame::Collection => "collection",
        GuiEmbyRowKindFrame::Media => "media",
    }
}

/// One painted row of the plain Emby-library list: the letter heading or the
/// item row. Its hover offers Play and Add on every row, and Shuffle on a
/// `Collection` row alone (`gui-row-hover-actions` design D3).
fn paint_emby_row(
    row: &LetteredRow<LetteredItemRow<GuiEmbyRowFrame>>,
    library: &LibraryState,
    selected: Option<&str>,
) -> Element {
    match row {
        LetteredRow::Heading(heading) => paint_letter_heading("emby", heading),
        LetteredRow::Item(item) => {
            let selected_row = selected == Some(item.id.as_str());
            let press_row = row.clone();
            let press_library = library.clone();
            let play_id = item.row.id.clone();
            let play_library = library.clone();
            let hover = library.item_hover_presses(
                item.row.id.clone(),
                EmbyList::play,
                EmbyList::queue,
                item.row.kind == GuiEmbyRowKindFrame::Collection,
            );
            let menu_id = item.row.id.clone();
            let menu_library = library.clone();
            rsx! {
                FlatRow {
                    attr: "emby",
                    kind: kind_word(item.row.kind).to_owned(),
                    id: item.id.clone(),
                    title: item.row.title.clone(),
                    subtitle: item.row.subtitle.clone(),
                    trail: item.row.date.clone(),
                    trail_part: LibraryTrailPart::Date,
                    selected: selected_row,
                    thumb: Some(LibraryRowThumb {
                        art: item.row.art.clone(),
                        media: item.row.media,
                    }),
                    on_press: Callback::new(move |()| {
                        EmbyList::select(&press_library, &press_row);
                    }),
                    on_play: Callback::new(move |()| EmbyList::play(&play_library, &play_id)),
                    on_hover_play: hover.play,
                    on_hover_queue: hover.queue,
                    on_hover_shuffle: hover.shuffle,
                    on_context: Some(Callback::new(
                        move |(x, y): (f32, f32)| menu_library.open_row_menu_at(
                            Some(LibraryState::row_menu_item(menu_id.clone())),
                            x,
                            y,
                        ),
                    )),
                }
            }
        }
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

    const ATTR: &'static str = "abs";
    const PADDING: &'static str = "8px 0 8px 8px";
    const ART_SCOPE: GuiRowArtScopeFrame = GuiRowArtScopeFrame::Audiobooks;
    const POINTER_LIST: PointerList = PointerList::Audiobooks;
    const NOTICES: LetteredNotices = LetteredNotices {
        loading_attr: "data-library-abs-loading",
        failed_attr: "data-library-abs-failed",
        empty_attr: "data-library-abs-empty",
        failed_text: "Couldn’t load this catalog.",
        empty_text: "No audiobooks",
    };
    const HEADING_KEY: &'static str = "abs-letter";

    fn list(list: Option<&GuiLibraryList>) -> Option<&Self::List> {
        library_audiobooks_list(list)
    }

    fn detail(detail: Option<&GuiLibraryDetail>) -> Option<&Self::Detail> {
        library_audiobooks_detail(detail)
    }

    fn selected(detail: &Self::Detail) -> Option<&str> {
        detail.selected.as_deref()
    }

    fn hero_present(detail: &Self::Detail) -> bool {
        detail.hero.is_some()
    }

    fn load_facts(list: &Self::List) -> LetteredLoadFacts {
        LetteredLoadFacts::of(list.loading, list.failed, list.rows.is_empty())
    }

    fn header(list: Option<&Self::List>) -> LetteredHeader {
        LetteredHeader {
            mark: ServiceMark::Audiobookshelf,
            label: String::new(),
            label_attr: None,
            collections: None,
            sort: list.and_then(|list| list.sort.clone()),
            placeholder: "Filter books".to_owned(),
            aria_label: "Filter books",
        }
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

    fn paint_row(
        row: &LetteredRow<LetteredItemRow<Self::Row>>,
        library: &LibraryState,
        selected: Option<&str>,
    ) -> Element {
        paint_audiobooks_row(row, library, selected)
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

/// One painted row of the book list: the letter heading or the book row. A
/// book is no collection, so its hover offers Play and Add and no Shuffle
/// (`gui-row-hover-actions` design D3).
fn paint_audiobooks_row(
    row: &LetteredRow<LetteredItemRow<GuiAudiobooksRowFrame>>,
    library: &LibraryState,
    selected: Option<&str>,
) -> Element {
    match row {
        LetteredRow::Heading(heading) => paint_letter_heading("abs", heading),
        LetteredRow::Item(item) => {
            let selected_row = selected == Some(item.id.as_str());
            let press_row = row.clone();
            let press_library = library.clone();
            let play_id = item.row.id.clone();
            let play_library = library.clone();
            let hover = library.item_hover_presses(
                item.row.id.clone(),
                AudiobooksList::play,
                AudiobooksList::queue,
                false,
            );
            let menu_id = item.row.id.clone();
            let menu_library = library.clone();
            rsx! {
                FlatRow {
                    attr: "abs",
                    kind: "book".to_owned(),
                    id: item.id.clone(),
                    title: item.row.title.clone(),
                    subtitle: item.row.subtitle.clone(),
                    trail: Some(item.row.duration.clone()),
                    trail_part: LibraryTrailPart::Duration,
                    selected: selected_row,
                    thumb: Some(LibraryRowThumb {
                        art: item.row.art.clone(),
                        media: item.row.media,
                    }),
                    on_press: Callback::new(move |()| {
                        AudiobooksList::select(&press_library, &press_row);
                    }),
                    on_play: Callback::new(move |()| {
                        AudiobooksList::play(&play_library, &play_id);
                    }),
                    on_hover_play: hover.play,
                    on_hover_queue: hover.queue,
                    on_context: Some(Callback::new(
                        move |(x, y): (f32, f32)| menu_library.open_row_menu_at(
                            Some(LibraryState::row_menu_item(menu_id.clone())),
                            x,
                            y,
                        ),
                    )),
                }
            }
        }
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

    const ATTR: &'static str = "podcasts";
    const PADDING: &'static str = "8px 8px 8px 8px";
    const ART_SCOPE: GuiRowArtScopeFrame = GuiRowArtScopeFrame::Podcasts;
    const POINTER_LIST: PointerList = PointerList::Podcasts;
    const NOTICES: LetteredNotices = LetteredNotices {
        loading_attr: "data-library-podcasts-loading",
        failed_attr: "data-library-podcasts-failed",
        empty_attr: "data-library-podcasts-empty",
        failed_text: "Couldn’t load this library.",
        empty_text: "No episodes",
    };
    const HEADING_KEY: &'static str = "podcasts-letter";

    fn list(list: Option<&GuiLibraryList>) -> Option<&Self::List> {
        library_podcasts_list(list)
    }

    fn detail(detail: Option<&GuiLibraryDetail>) -> Option<&Self::Detail> {
        library_podcasts_detail(detail)
    }

    fn selected(detail: &Self::Detail) -> Option<&str> {
        detail.selected.as_deref()
    }

    fn hero_present(detail: &Self::Detail) -> bool {
        detail.hero.is_some()
    }

    fn load_facts(list: &Self::List) -> LetteredLoadFacts {
        LetteredLoadFacts::of(list.loading, list.failed, list.rows.is_empty())
    }

    fn header(list: Option<&Self::List>) -> LetteredHeader {
        let label = list.map_or_else(|| "All".to_owned(), |list| list.collections.label());
        LetteredHeader {
            mark: ServiceMark::Audiobookshelf,
            placeholder: format!("Filter {label}"),
            label,
            label_attr: Some("data-library-podcasts-selection-label"),
            collections: list.map(|list| list.collections.clone()),
            sort: list.and_then(|list| list.sort.clone()),
            aria_label: "Filter episodes",
        }
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

    fn paint_row(
        row: &LetteredRow<LetteredItemRow<Self::Row>>,
        library: &LibraryState,
        selected: Option<&str>,
    ) -> Element {
        paint_podcasts_row(row, library, selected)
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

/// One painted row of the Podcasts list: the letter heading, or the episode
/// row.
fn paint_podcasts_row(
    row: &LetteredRow<LetteredItemRow<GuiEntryRowFrame>>,
    library: &LibraryState,
    selected: Option<&str>,
) -> Element {
    match row {
        LetteredRow::Heading(heading) => paint_letter_heading("podcasts", heading),
        LetteredRow::Item(row) => paint_podcasts_item(row, library, selected),
    }
}

/// One painted episode row: the row's press selects the episode the Hero
/// shows, a double-click plays it, and a right-press opens the row menu. Its
/// hover controls run the destination's play and queue paths (design D4).
fn paint_podcasts_item(
    row: &LetteredItemRow<GuiEntryRowFrame>,
    library: &LibraryState,
    selected: Option<&str>,
) -> Element {
    let press_item = row.clone();
    let press_library = library.clone();
    let play_id = row.row.id.clone();
    let play_library = library.clone();
    let hover = library.item_hover_presses(
        row.row.id.clone(),
        PodcastsList::play,
        PodcastsList::queue,
        false,
    );
    let menu_id = row.row.id.clone();
    let menu_library = library.clone();
    let pressed_row = LetteredRow::Item(press_item);
    rsx! {
        FlatRow {
            attr: "podcasts",
            kind: "episode".to_owned(),
            id: row.id.clone(),
            title: row.row.title.clone(),
            subtitle: row.row.subtitle.clone(),
            trail: row.row.date.clone(),
            trail_part: LibraryTrailPart::Date,
            play: Some(row.row.state.clone()),
            selected: Some(row.id.as_str()) == selected,
            thumb: Some(LibraryRowThumb {
                art: row.row.art.clone(),
                media: row.row.media,
            }),
            on_press: Callback::new(move |()| PodcastsList::select(&press_library, &pressed_row)),
            on_play: Callback::new(move |()| PodcastsList::play(&play_library, &play_id)),
            on_hover_play: hover.play,
            on_hover_queue: hover.queue,
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library
                    .open_row_menu_at(Some(LibraryState::row_menu_item(menu_id.clone())), x, y);
            })),
        }
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

    const ATTR: &'static str = "feeds";
    const PADDING: &'static str = "8px 8px 8px 8px";
    const ART_SCOPE: GuiRowArtScopeFrame = GuiRowArtScopeFrame::Feeds;
    const POINTER_LIST: PointerList = PointerList::Feeds;
    const NOTICES: LetteredNotices = LetteredNotices {
        loading_attr: "data-library-feeds-loading",
        failed_attr: "data-library-feeds-failed",
        empty_attr: "data-library-feeds-empty",
        failed_text: "Couldn’t load this destination.",
        empty_text: "No entries",
    };
    const HEADING_KEY: &'static str = "feeds-letter";

    fn list(list: Option<&GuiLibraryList>) -> Option<&Self::List> {
        library_feeds_list(list)
    }

    fn detail(detail: Option<&GuiLibraryDetail>) -> Option<&Self::Detail> {
        library_feeds_detail(detail)
    }

    fn selected(detail: &Self::Detail) -> Option<&str> {
        detail.selected.as_deref()
    }

    fn hero_present(detail: &Self::Detail) -> bool {
        detail.hero.is_some()
    }

    fn load_facts(list: &Self::List) -> LetteredLoadFacts {
        LetteredLoadFacts::of(list.loading, list.failed, list.rows.is_empty())
    }

    fn header(list: Option<&Self::List>) -> LetteredHeader {
        let label = list.map_or_else(|| "All".to_owned(), |list| list.collections.label());
        LetteredHeader {
            mark: ServiceMark::feeds(list.and_then(|list| list.platform)),
            placeholder: format!("Filter {label}"),
            label,
            label_attr: Some("data-library-feeds-selection-label"),
            collections: list.map(|list| list.collections.clone()),
            sort: list.and_then(|list| list.sort.clone()),
            aria_label: "Filter entries",
        }
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

    fn paint_row(
        row: &LetteredRow<LetteredItemRow<Self::Row>>,
        library: &LibraryState,
        selected: Option<&str>,
    ) -> Element {
        paint_feeds_row(row, library, selected)
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

/// One painted row of the Feeds list: the letter heading, or the entry row.
fn paint_feeds_row(
    row: &LetteredRow<LetteredItemRow<GuiEntryRowFrame>>,
    library: &LibraryState,
    selected: Option<&str>,
) -> Element {
    match row {
        LetteredRow::Heading(heading) => paint_letter_heading("feeds", heading),
        LetteredRow::Item(row) => paint_feeds_item(row, library, selected),
    }
}

/// One painted entry row: a press selects the entry the Hero shows, a
/// double-click plays it, and a right-press opens the row menu. Its hover
/// controls run the destination's play and queue paths (design D4).
fn paint_feeds_item(
    row: &LetteredItemRow<GuiEntryRowFrame>,
    library: &LibraryState,
    selected: Option<&str>,
) -> Element {
    let press_item = row.clone();
    let press_library = library.clone();
    let play_id = row.row.id.clone();
    let play_library = library.clone();
    let hover =
        library.item_hover_presses(row.row.id.clone(), FeedsList::play, FeedsList::queue, false);
    let menu_id = row.row.id.clone();
    let menu_library = library.clone();
    let pressed_row = LetteredRow::Item(press_item);
    rsx! {
        FlatRow {
            attr: "feeds",
            kind: "entry".to_owned(),
            id: row.id.clone(),
            title: row.row.title.clone(),
            subtitle: row.row.subtitle.clone(),
            trail: row.row.date.clone(),
            trail_part: LibraryTrailPart::Date,
            play: Some(row.row.state.clone()),
            selected: Some(row.id.as_str()) == selected,
            thumb: Some(LibraryRowThumb {
                art: row.row.art.clone(),
                media: row.row.media,
            }),
            on_press: Callback::new(move |()| FeedsList::select(&press_library, &pressed_row)),
            on_play: Callback::new(move |()| FeedsList::play(&play_library, &play_id)),
            on_hover_play: hover.play,
            on_hover_queue: hover.queue,
            on_context: Some(Callback::new(move |(x, y): (f32, f32)| {
                menu_library
                    .open_row_menu_at(Some(LibraryState::row_menu_item(menu_id.clone())), x, y);
            })),
        }
    }
}
