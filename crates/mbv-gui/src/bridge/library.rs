//! The `library` command's wire form (add-gui-library-panel design D2): the
//! Library panel's window content sends one semantic request, and the neutral
//! [`mbv::GuiLibraryCommand`] this module builds from it is what the Client
//! core applies. Item identities are stable Service ids, never row positions,
//! so a command built from a stale frame addresses the item that frame painted
//! or nothing.

use mbv_queue::QueueItemContentId;
use serde::{Deserialize, Serialize};

/// One request from the Library panel's window content (design D2). The
/// variants and their field names are the wire vocabulary the front end
/// sends; the translation into [`mbv::GuiLibraryCommand`] is the only place
/// the two shapes meet.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "command",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GuiLibraryCommand {
    /// Open the destination at `position`: the tab position the frame's
    /// destination tabs carry, where 0 is Home.
    OpenDestination { position: usize },
    /// Select the one collection group of the Grouped Music library `libIdx`.
    SelectCollection { lib_idx: usize, group_id: String },
    /// Show the item's detail in the Hero.
    ShowDetail { item_id: String },
    /// Open the album node: load its tracks when the cache does not hold
    /// them.
    OpenAlbum { album_id: String },
    /// Take Panel focus for the Library panel.
    TakeFocus,
    /// Take Panel focus for the Queue panel: Tab across the panel boundary's
    /// far end (design D3, D4). The mirror of [`GuiLibraryCommand::TakeFocus`].
    TakeQueueFocus,
    /// Play the item.
    Play { item_id: String },
    /// Play the item next.
    PlayNext { item_id: String },
    /// Add the item to the queue.
    AddToQueue { item_id: String },
    /// Shuffle the artist whose albums are `albumIds`.
    ShuffleArtist { album_ids: Vec<String> },
    /// Run the item menu's entry at `index`.
    MenuEntry { item_id: String, index: usize },
    /// Carry the item's full menu in the frame (the right-click item menu).
    RowMenu { item_id: String },
    /// Select a Home row (add-gui-library-panel design D12): the row the
    /// Hero shows. The core keeps the selection, so the frame paints it.
    HomeSelect { row: GuiHomeRowId },
    /// Play a Home row (design D12).
    HomePlay { row: GuiHomeRowId },
    /// Enqueue a Home row after the active slot (design D12): the Owner
    /// owns the queue, and the row resolves through the same Home enqueue
    /// path the add carries.
    HomePlayNext { row: GuiHomeRowId },
    /// Enqueue a Home row (design D12).
    HomeQueue { row: GuiHomeRowId },
    /// Set the Shows destination's view mode (add-gui-library-panel design
    /// D12, the USER-APPROVED GUI Shows design): the root level loads
    /// through the TUI's own mode load.
    ShowsSetMode { lib_idx: usize, mode: GuiShowsMode },
    /// Select a Shows row (design D12): the row the Hero shows — the All
    /// series mode's series, or a flat mode's episode or premiere row.
    ShowsSelect { lib_idx: usize, row: String },
    /// Select the season whose episodes the Hero's Workspace shows (design
    /// D12).
    ShowsSelectSeason { lib_idx: usize, season_id: String },
    /// Play a Shows episode (design D12): Enter on the row, or the
    /// double-click a front end sends with this same command.
    ShowsPlay { lib_idx: usize, item_id: String },
    /// Play a Shows episode next (design D12).
    ShowsPlayNext { lib_idx: usize, item_id: String },
    /// Add a Shows episode to the queue (design D12).
    ShowsQueue { lib_idx: usize, item_id: String },
    /// Select a plain Emby-library row (add-gui-library-panel design D12,
    /// row 8.1): the row the Hero shows. A row the active list no longer
    /// holds changes nothing.
    EmbySelect { lib_idx: usize, row: String },
    /// Set the feed-view library's selected group (row 8.1): the TUI's own
    /// selection path. Feed-group mode only; a library that presents none
    /// changes nothing.
    EmbySetFeedGroup {
        lib_idx: usize,
        group: GuiEmbyFeedGroup,
    },
    /// Play a plain Emby-library row (row 8.1): Enter on the row, or the
    /// double-click a front end sends with this same command. A folder or a
    /// boxset plays through the TUI's folder-play rule, so no nested list
    /// opens.
    EmbyPlay { lib_idx: usize, item_id: String },
    /// Play a plain Emby-library row next (row 8.1).
    EmbyPlayNext { lib_idx: usize, item_id: String },
    /// Add a plain Emby-library row to the queue (row 8.1).
    EmbyQueue { lib_idx: usize, item_id: String },
    /// Select an Audiobooks book (add-gui-library-panel design D12, row
    /// 9.1): the book the Hero shows. A book the published catalog no longer
    /// carries changes nothing.
    AbsSelect { lib_idx: usize, row: String },
    /// Play an Audiobooks book (row 9.1): Enter on the book row, or the
    /// double-click a front end sends with this same command. The queue is
    /// replaced with the book at its saved position.
    AbsPlay { lib_idx: usize, item_id: String },
    /// Play or seek from an Audiobooks chapter (row 9.1): Enter on a
    /// Workspace chapter row, or the double-click a front end sends with
    /// this same command.
    AbsPlayChapter {
        lib_idx: usize,
        book_id: String,
        chapter: usize,
    },
    /// Add an Audiobooks book to the queue (row 9.1).
    AbsQueue { lib_idx: usize, item_id: String },
    /// Set the Podcasts destination's pill (add-gui-library-panel design D12,
    /// row 10.1): Latest, or one show by its stable `library_item_id`. The
    /// selected pill's own loads start through the TUI's own loaders.
    PodcastsSetPill {
        lib_idx: usize,
        pill: GuiPodcastPill,
    },
    /// Select the Podcasts episode the Hero shows (row 10.1): the row id the
    /// frame carried — the episode's opaque identity.
    PodcastsSelect { lib_idx: usize, row: String },
    /// Play a Podcasts episode (row 10.1): Enter on the row, or the
    /// double-click a front end sends with this same command.
    PodcastsPlay { lib_idx: usize, item_id: String },
    /// Add a Podcasts episode to the queue (row 10.1).
    PodcastsQueue { lib_idx: usize, item_id: String },
    /// Set the Feeds destination's pill (add-gui-library-panel design D12,
    /// row 11.1): Latest, or one subscription by its URL. A Client holds one
    /// feed tab, so no library index rides here.
    FeedsSetPill { pill: GuiFeedPill },
    /// Select the Feeds entry the Hero shows (row 11.1): the row id the frame
    /// carried — the entry's opaque identity.
    FeedsSelect { row: String },
    /// Play a Feeds entry (row 11.1): Enter on the row, or the double-click a
    /// front end sends with this same command.
    FeedsPlay { item_id: String },
    /// Add a Feeds entry to the queue (row 11.1).
    FeedsQueue { item_id: String },
    /// Send the Search field's query (add-gui-library-panel design D12, row
    /// 12.1): the field's typing sends one, and an empty query clears the
    /// results. Cross-destination, so no library index rides it.
    SearchQuery { query: String },
    /// Activate a Search result (row 12.1): Enter or a press on the row. The
    /// id and type pair is the row's stable Service identity.
    SearchActivate { id: String, item_type: String },
    /// Close the Search results (row 12.1): the Esc in the field.
    SearchClose,
}

/// One Feeds pill (add-gui-library-panel design D12, row 11.1): the merged
/// Latest view, or one subscription by its URL. The pill row is single-select
/// and the state pills are dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum GuiFeedPill {
    Latest,
    Subscription(String),
}

impl From<GuiFeedPill> for mbv::GuiFeedPill {
    fn from(pill: GuiFeedPill) -> Self {
        match pill {
            GuiFeedPill::Latest => Self::Latest,
            GuiFeedPill::Subscription(feed_id) => Self::Subscription(feed_id),
        }
    }
}

impl From<&mbv::GuiFeedPill> for GuiFeedPill {
    fn from(pill: &mbv::GuiFeedPill) -> Self {
        match pill {
            mbv::GuiFeedPill::Latest => Self::Latest,
            mbv::GuiFeedPill::Subscription(feed_id) => Self::Subscription(feed_id.clone()),
        }
    }
}

/// One Podcasts pill (design D12, row 10.1): the library's merged Latest view,
/// one of the TUI's three state pills, or one show by its stable
/// `library_item_id`. The pill row is single-select, as the TUI's one-valued
/// `PillSelection` is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum GuiPodcastPill {
    Latest,
    /// A state pill: the episode's play state the view filters on.
    State(GuiPodcastState),
    Show(String),
}

/// One state pill (design D12, row 10.1), mirroring the TUI's
/// `AudiobookshelfEpisodeFilter` by name on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiPodcastState {
    All,
    Unplayed,
    Played,
}

impl From<GuiPodcastState> for mbv_ui_model::audiobookshelf_browse::AudiobookshelfEpisodeFilter {
    fn from(state: GuiPodcastState) -> Self {
        match state {
            GuiPodcastState::All => Self::All,
            GuiPodcastState::Unplayed => Self::Unplayed,
            GuiPodcastState::Played => Self::Played,
        }
    }
}

impl From<mbv_ui_model::audiobookshelf_browse::AudiobookshelfEpisodeFilter> for GuiPodcastState {
    fn from(state: mbv_ui_model::audiobookshelf_browse::AudiobookshelfEpisodeFilter) -> Self {
        use mbv_ui_model::audiobookshelf_browse::AudiobookshelfEpisodeFilter;
        match state {
            AudiobookshelfEpisodeFilter::All => Self::All,
            AudiobookshelfEpisodeFilter::Unplayed => Self::Unplayed,
            AudiobookshelfEpisodeFilter::Played => Self::Played,
        }
    }
}

impl From<GuiPodcastPill> for mbv::GuiPodcastPill {
    fn from(pill: GuiPodcastPill) -> Self {
        match pill {
            GuiPodcastPill::Latest => Self::Latest,
            GuiPodcastPill::State(state) => Self::State(state.into()),
            GuiPodcastPill::Show(show_id) => Self::Show(show_id),
        }
    }
}

impl From<&mbv::GuiPodcastPill> for GuiPodcastPill {
    fn from(pill: &mbv::GuiPodcastPill) -> Self {
        match pill {
            mbv::GuiPodcastPill::Latest => Self::Latest,
            mbv::GuiPodcastPill::State(state) => Self::State((*state).into()),
            mbv::GuiPodcastPill::Show(show_id) => Self::Show(show_id.clone()),
        }
    }
}

/// One Shows view mode (design D12): the three single-select pills the
/// Shows pill row paints. The TUI's letter-range buckets are not pills
/// here — the All series list's letter rail replaces them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiShowsMode {
    Latest,
    Upcoming,
    All,
}

impl From<GuiShowsMode> for mbv::GuiShowsMode {
    fn from(mode: GuiShowsMode) -> Self {
        match mode {
            GuiShowsMode::Latest => Self::Latest,
            GuiShowsMode::Upcoming => Self::Upcoming,
            GuiShowsMode::All => Self::All,
        }
    }
}

/// One feed-group pill of a feed-view Emby library (add-gui-library-panel
/// design D12, row 8.1): `All`, or one root folder by its stable Service id.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum GuiEmbyFeedGroup {
    All,
    Folder(String),
}

/// One Home row's stable identity (design D12): the identity the Home
/// paths resolve, addressed by Service ids and never by a row position.
/// `Recent` carries the queue item's content id, whose own serde form is
/// itself (`provider` and `value`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum GuiHomeRowId {
    /// A `Continue watching` row's Emby item id.
    Continue(String),
    /// A `Recently added` row's queue-item content id.
    Recent(QueueItemContentId),
}

impl From<GuiLibraryCommand> for mbv::GuiLibraryCommand {
    fn from(command: GuiLibraryCommand) -> Self {
        match command {
            GuiLibraryCommand::OpenDestination { position } => Self::OpenDestination { position },
            GuiLibraryCommand::SelectCollection { lib_idx, group_id } => {
                Self::SelectCollection { lib_idx, group_id }
            }
            GuiLibraryCommand::ShowDetail { item_id } => Self::ShowDetail { item_id },
            GuiLibraryCommand::OpenAlbum { album_id } => Self::OpenAlbum { album_id },
            GuiLibraryCommand::TakeFocus => Self::TakeFocus,
            GuiLibraryCommand::TakeQueueFocus => Self::TakeQueueFocus,
            GuiLibraryCommand::Play { item_id } => Self::Play { item_id },
            GuiLibraryCommand::PlayNext { item_id } => Self::PlayNext { item_id },
            GuiLibraryCommand::AddToQueue { item_id } => Self::AddToQueue { item_id },
            GuiLibraryCommand::ShuffleArtist { album_ids } => Self::ShuffleArtist { album_ids },
            GuiLibraryCommand::MenuEntry { item_id, index } => Self::MenuEntry { item_id, index },
            GuiLibraryCommand::RowMenu { item_id } => Self::RowMenu { item_id },
            GuiLibraryCommand::HomeSelect { row } => Self::HomeSelect {
                row: mbv::GuiHomeRowId::from(&row),
            },
            GuiLibraryCommand::HomePlay { row } => Self::HomePlay {
                row: mbv::GuiHomeRowId::from(&row),
            },
            GuiLibraryCommand::HomePlayNext { row } => Self::HomePlayNext {
                row: mbv::GuiHomeRowId::from(&row),
            },
            GuiLibraryCommand::HomeQueue { row } => Self::HomeQueue {
                row: mbv::GuiHomeRowId::from(&row),
            },
            GuiLibraryCommand::ShowsSetMode { lib_idx, mode } => Self::ShowsSetMode {
                lib_idx,
                mode: mbv::GuiShowsMode::from(mode),
            },
            GuiLibraryCommand::ShowsSelect { lib_idx, row } => Self::ShowsSelect { lib_idx, row },
            GuiLibraryCommand::ShowsSelectSeason { lib_idx, season_id } => {
                Self::ShowsSelectSeason { lib_idx, season_id }
            }
            GuiLibraryCommand::ShowsPlay { lib_idx, item_id } => {
                Self::ShowsPlay { lib_idx, item_id }
            }
            GuiLibraryCommand::ShowsPlayNext { lib_idx, item_id } => {
                Self::ShowsPlayNext { lib_idx, item_id }
            }
            GuiLibraryCommand::ShowsQueue { lib_idx, item_id } => {
                Self::ShowsQueue { lib_idx, item_id }
            }
            GuiLibraryCommand::EmbySelect { lib_idx, row } => Self::EmbySelect { lib_idx, row },
            GuiLibraryCommand::EmbySetFeedGroup { lib_idx, group } => Self::EmbySetFeedGroup {
                lib_idx,
                group: match group {
                    GuiEmbyFeedGroup::All => mbv::GuiEmbyFeedGroup::All,
                    GuiEmbyFeedGroup::Folder(folder_id) => mbv::GuiEmbyFeedGroup::Folder(folder_id),
                },
            },
            GuiLibraryCommand::EmbyPlay { lib_idx, item_id } => Self::EmbyPlay { lib_idx, item_id },
            GuiLibraryCommand::EmbyPlayNext { lib_idx, item_id } => {
                Self::EmbyPlayNext { lib_idx, item_id }
            }
            GuiLibraryCommand::EmbyQueue { lib_idx, item_id } => {
                Self::EmbyQueue { lib_idx, item_id }
            }
            GuiLibraryCommand::AbsSelect { lib_idx, row } => Self::AbsSelect { lib_idx, row },
            GuiLibraryCommand::AbsPlay { lib_idx, item_id } => Self::AbsPlay { lib_idx, item_id },
            GuiLibraryCommand::AbsPlayChapter {
                lib_idx,
                book_id,
                chapter,
            } => Self::AbsPlayChapter {
                lib_idx,
                book_id,
                chapter,
            },
            GuiLibraryCommand::AbsQueue { lib_idx, item_id } => Self::AbsQueue { lib_idx, item_id },
            GuiLibraryCommand::PodcastsSetPill { lib_idx, pill } => Self::PodcastsSetPill {
                lib_idx,
                pill: mbv::GuiPodcastPill::from(pill),
            },
            GuiLibraryCommand::PodcastsSelect { lib_idx, row } => {
                Self::PodcastsSelect { lib_idx, row }
            }
            GuiLibraryCommand::PodcastsPlay { lib_idx, item_id } => {
                Self::PodcastsPlay { lib_idx, item_id }
            }
            GuiLibraryCommand::PodcastsQueue { lib_idx, item_id } => {
                Self::PodcastsQueue { lib_idx, item_id }
            }
            GuiLibraryCommand::FeedsSetPill { pill } => Self::FeedsSetPill {
                pill: mbv::GuiFeedPill::from(pill),
            },
            GuiLibraryCommand::FeedsSelect { row } => Self::FeedsSelect { row },
            GuiLibraryCommand::FeedsPlay { item_id } => Self::FeedsPlay { item_id },
            GuiLibraryCommand::FeedsQueue { item_id } => Self::FeedsQueue { item_id },
            GuiLibraryCommand::SearchQuery { query } => Self::SearchQuery { query },
            GuiLibraryCommand::SearchActivate { id, item_type } => {
                Self::SearchActivate { id, item_type }
            }
            GuiLibraryCommand::SearchClose => Self::SearchClose,
        }
    }
}

/// One row id's core identity, resolving the wire type the front end sends.
/// The frame mirrors the same shape on the way out, so a command built from
/// a carried row addresses exactly that row.
impl From<&GuiHomeRowId> for mbv::GuiHomeRowId {
    fn from(row: &GuiHomeRowId) -> Self {
        match row {
            GuiHomeRowId::Continue(item_id) => Self::Continue(item_id.clone()),
            GuiHomeRowId::Recent(content_id) => Self::Recent(content_id.clone()),
        }
    }
}
