//! The Movies/HomeVideos/Generic Emby destinations' embedded content owner
//! (task 6.1, design D2/D3). This owner serves Generic, Movies, and `HomeVideos`;
//! TV and Music have their own owners. A plain type — never mounted, focused,
//! subscribed, or given a `ComponentId` — that keeps the shell-projected
//! browse rows, the letter/feed-group pill state, the one shared canonical
//! `MediaList` owner of the active level's rows, and the embedded Inline
//! Search session. It produces the panel's [`LibraryPanelContent`] per frame
//! and translates the panel's slot events and forwarded chords into the same
//! typed `Msg`s the former `BrowserComponent` emitted for these three kinds
//! (`shell/emby_library.rs::handle_emby_library_request` and `shell/messages.rs`'s
//! `Browser*`/`EmbyLibrary*` dispatch are unchanged and keyed only by the
//! active tab, so they apply unmodified to messages this owner emits).
//!
//! TV's browsing uses its own owner; nothing here is shared state with it.
//!
//! The selected item's hero comes from the shared `hero_content_emby`
//! producer (design D5) and is rendered by the Library Hero overlay — this
//! owner never builds a banner layout or fetches an image itself (task 5.10's shell
//! projection does that, generically, for every migrated owner).

use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;

use super::inline_search::InlineSearch;
use super::library_panel::content::HeroImageState;
use super::library_panel::LibraryKind;
use super::media_list::{
    letter_grouped_rows, MediaKind, MediaListCarrier, MediaListRow, MediaListTrailing,
    MediaSemanticState,
};
use super::msg::{Msg, ShellRequest};
use crate::app::render::{effective_sort_str, LetterFilter};

mod input;
mod panel;

/// Which selector policy the projected Emby content exposes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) enum EmbySelectorMode {
    #[default]
    None,
    FeedGroups,
    Letters,
}

/// Browse identity used to decide when a projected position should be applied.
#[derive(Clone, Default, PartialEq, Eq)]
pub(in crate::app) struct EmbyLibraryIdentity {
    pub(in crate::app) depth: usize,
    pub(in crate::app) parent_id: String,
    pub(in crate::app) letter_filter: Option<usize>,
    pub(in crate::app) sort_by: String,
    pub(in crate::app) sort_order: String,
    pub(in crate::app) unplayed_only: bool,
    pub(in crate::app) feed_group: Option<usize>,
}

fn latest_row_projection(item: &EmbyItem) -> (String, Option<String>, Option<MediaListTrailing>) {
    let item = QueueItem::Emby(Box::new(item.clone()));
    let parts = item.playback_title_parts(None);
    let (primary, secondary) = match parts.context {
        Some(context) => (context.text, Some(parts.title.text)),
        None => (parts.title.text, None),
    };
    let trailing = crate::app::state::home_latest::provider_timestamp_secs(&item)
        .map(crate::app::ui_util::fmt_publish_date_short)
        .filter(|date| !date.is_empty())
        .map(MediaListTrailing::Gutter);
    (primary, secondary, trailing)
}

fn row_for(item: &EmbyItem, latest: bool) -> MediaListRow<String> {
    let primary = if item.is_folder && item.item_type == "Folder" && item.total_count > 0 {
        format!("{} \u{b7} {} items", item.display_name(), item.total_count)
    } else if item.is_folder && item.unplayed_item_count > 0 && item.item_type != "Series" {
        format!("{} [{}]", item.display_name(), item.unplayed_item_count)
    } else {
        item.display_name()
    };
    let (primary, secondary, trailing) = if latest {
        latest_row_projection(item)
    } else {
        (
            primary,
            None,
            (!item.is_folder && item.production_year > 0)
                .then(|| MediaListTrailing::Gutter(item.production_year.to_string())),
        )
    };
    MediaListRow::Item {
        target: item.id.clone(),
        primary,
        secondary,
        trailing,
        duration: None,
        kind: MediaKind::Collection,
        semantic_state: MediaSemanticState::from_emby(item),
    }
}

/// One shell content push (mirrors the Emby library owner's content push, plus the
/// letter/feed-group pill facts the old `NarrowBrowseExtras`/wide-Movies pill
/// row carried separately; task 6.1 unifies them into the Selector row,
/// design D8).
pub(in crate::app) struct BrowserOwnerPush {
    pub items: Vec<EmbyItem>,
    pub latest_items: Vec<EmbyItem>,
    pub total_count: usize,
    pub library_total: Option<usize>,
    pub letter_filter: Option<LetterFilter>,
    pub loading: bool,
    /// Selector policy: feed-group pills and letter pills are alternatives.
    pub selector_mode: EmbySelectorMode,
    pub feed_groups: Vec<String>,
    /// The group folders' Service content IDs, aligned 1:1 with
    /// `feed_groups` (task 2.1): the launch snapshot resolves the selected
    /// group pill to its content ID, never to the display name above.
    pub feed_group_ids: Vec<String>,
    pub feed_group_cursor: usize,
}

/// The embedded content owner for Movies, `HomeVideos` and Generic Emby
/// libraries (design D2, task 6.1). Plain type; the mounted `LibraryPanel`
/// borrows it for content and slot events.
pub(in crate::app) struct EmbyLibraryContent {
    kind: LibraryKind,
    browse_items: Vec<EmbyItem>,
    latest_items: Vec<EmbyItem>,
    latest_mode: bool,
    latest_marker: bool,
    saved_browse_position: Option<(String, usize)>,
    total_count: usize,
    library_total: Option<usize>,
    letter_filter: Option<LetterFilter>,
    loading: bool,
    selector_mode: EmbySelectorMode,
    feed_groups: Vec<String>,
    feed_group_ids: Vec<String>,
    feed_group_cursor: usize,
    /// The one shared canonical owner of the active level's rows; the panel
    /// drives its Wide/Inline presentation from its own breakpoint (design
    /// D3/D4) — this owner never chooses a presentation itself.
    carrier: MediaListCarrier<String>,
    last_identity: Option<EmbyLibraryIdentity>,
    /// The rows the last `feed_owner` projection produced: identical
    /// projections skip `carrier.set_content`, so an ordinary no-op sync
    /// never invalidates the presentation's painted frame (design D6 — a
    /// configured-but-unpainted owner claims nothing; the Home owner avoids
    /// this by pushing at writer seams only, this owner re-projects per
    /// sync, so the skip must live here).
    last_projected_rows: Option<Vec<MediaListRow<String>>>,
    /// The projection's image state for the current hero (task 5.10): set by
    /// the shell, read by the painters through the panel content.
    hero_image: HeroImageState,
    hero_scroll: usize,
    /// The embedded Inline Search control (design D3): this owner is its
    /// sole event boundary; the panel places its box in the Selector row's
    /// rect and its results in the list box (`ListSlot::Search`).
    inline_search: InlineSearch,
}

impl EmbyLibraryContent {
    pub(in crate::app) fn new(kind: LibraryKind) -> Self {
        Self {
            kind,
            browse_items: Vec::new(),
            latest_items: Vec::new(),
            latest_mode: false,
            latest_marker: false,
            saved_browse_position: None,
            total_count: 0,
            library_total: None,
            letter_filter: None,
            loading: false,
            selector_mode: EmbySelectorMode::None,
            feed_groups: Vec::new(),
            feed_group_ids: Vec::new(),
            feed_group_cursor: 0,
            carrier: MediaListCarrier::new(),
            last_identity: None,
            last_projected_rows: None,
            hero_image: HeroImageState::None,
            hero_scroll: 0,
            inline_search: InlineSearch::new(),
        }
    }

    fn items(&self) -> &[EmbyItem] {
        if self.latest_mode {
            &self.latest_items
        } else {
            &self.browse_items
        }
    }

    /// Replace the shell-owned position-free content snapshot: the shared
    /// owner's `set_content` preserves the selected target and locally
    /// clamps otherwise (design D3); position re-seeds only through the
    /// separately identity-gated `apply_position`.
    pub(in crate::app) fn set_content(&mut self, push: BrowserOwnerPush) {
        self.browse_items = push.items;
        self.latest_items = push.latest_items;
        self.total_count = push.total_count;
        self.library_total = push.library_total;
        self.letter_filter = push.letter_filter;
        self.loading = push.loading;
        self.selector_mode = push.selector_mode;
        self.feed_groups = push.feed_groups;
        self.feed_group_ids = push.feed_group_ids;
        self.feed_group_cursor = push.feed_group_cursor;
        let restore_browse_position = if self.latest_mode {
            None
        } else {
            self.saved_browse_position.take()
        };
        self.feed_owner();
        if let Some((target, scroll)) = restore_browse_position {
            self.carrier.select_target(&target);
            self.carrier.set_scroll(scroll);
        }
    }

    pub(in crate::app) fn latest_mode(&self) -> bool {
        self.latest_mode
    }

    pub(in crate::app) fn set_latest_marker(&mut self, marker: bool) {
        self.latest_marker = marker;
    }

    pub(in crate::app) fn set_latest_mode(&mut self, latest: bool) {
        if self.latest_mode == latest {
            return;
        }
        if latest {
            self.saved_browse_position = self
                .carrier
                .selected_target()
                .cloned()
                .map(|target| (target, self.carrier.scroll()));
        }
        self.latest_mode = latest;
        self.feed_owner();
    }

    /// Explicit, identity-gated resting-position re-seed (mirrors
    /// the former `BrowserComponent`'s position application): the shell calls this only when
    /// `note_browse_identity` reports a real identity change (drill-in,
    /// go-back, letter-filter reset, sort change, feed/home-video group
    /// switch). Within one identity no position crosses the boundary.
    pub(in crate::app) fn apply_position(&mut self, cursor: usize, scroll: usize) {
        let target = self
            .items()
            .get(cursor.min(self.items().len().saturating_sub(1)))
            .map(|item| item.id.clone());
        if let Some(target) = target.as_ref() {
            self.carrier.select_target(target);
        }
        self.carrier.set_scroll(scroll);
    }

    /// Records the browse identity of the current shell content push and
    /// reports whether it differs from the previous push (mirrors
    /// the former `BrowserComponent`'s identity tracking).
    pub(in crate::app) fn note_browse_identity(&mut self, identity: EmbyLibraryIdentity) -> bool {
        let changed = self.last_identity.as_ref() != Some(&identity);
        self.last_identity = Some(identity);
        if changed {
            self.hero_scroll = 0;
        }
        changed
    }

    /// The authoritative selection of the shared owner, as an `items` index
    /// (the owner is the only cursor store).
    pub(in crate::app) fn cursor(&self) -> usize {
        self.carrier
            .selected_target()
            .and_then(|target| self.items().iter().position(|item| item.id == *target))
            .unwrap_or(0)
    }

    pub(in crate::app) fn scroll(&self) -> usize {
        self.carrier.scroll()
    }

    fn true_total(&self) -> usize {
        self.library_total.unwrap_or(self.total_count)
    }

    /// Project the mirrored items into provider-neutral rows: letter-grouped
    /// `Heading`/`Spacer`/`Item` rows for a large library (or an active
    /// letter pill), natural-sorted plain rows otherwise (mirrors
    /// the former `BrowserComponent`'s row projection).
    fn feed_owner(&mut self) {
        let grouped =
            !self.latest_mode && (self.true_total() >= 50 || self.letter_filter.is_some());
        let rows: Vec<MediaListRow<String>> = if grouped {
            let pairs: Vec<(String, MediaListRow<String>)> = self
                .items()
                .iter()
                .map(|item| (effective_sort_str(item).to_string(), row_for(item, false)))
                .collect();
            letter_grouped_rows(pairs, self.true_total(), self.letter_filter.is_some())
        } else {
            self.items()
                .iter()
                .map(|item| row_for(item, self.latest_mode))
                .collect()
        };
        // Ordinary refresh (design D3): an unchanged projection preserves
        // the shared owner's painted frame instead of re-issuing it.
        if self.last_projected_rows.as_ref() != Some(&rows) {
            self.carrier.set_content(rows.clone());
            self.last_projected_rows = Some(rows);
        }
    }

    /// The selected item, gated the same way the old wide Movies hero was
    /// (no hero for a folder; Movies libraries additionally require the
    /// selected item to actually be a `Movie`, not e.g. a `BoxSet` folder).
    fn hero_item(&self) -> Option<&EmbyItem> {
        let item = self.items().get(self.cursor())?;
        (!item.is_folder && (self.kind != LibraryKind::Movies || item.item_type == "Movie"))
            .then_some(item)
    }

    fn selected_effect_item(&self) -> Option<EmbyItem> {
        self.items().get(self.cursor()).cloned()
    }

    fn pick_selector(&mut self, index: usize) -> Msg {
        if index == 0 {
            self.set_latest_mode(true);
            return Msg::Shell(Box::new(ShellRequest::EmbyLibraryLatestSelected));
        }
        let was_latest = self.latest_mode;
        self.set_latest_mode(false);
        let target = if was_latest
            && self.selector_mode == EmbySelectorMode::Letters
            && index == 1
            && self.letter_filter.is_none()
        {
            usize::MAX
        } else {
            index - 1
        };
        Msg::Shell(Box::new(if was_latest {
            ShellRequest::EmbyLibraryLatestExit { target }
        } else {
            ShellRequest::EmbyLibraryPillClick { target: index - 1 }
        }))
    }
}
