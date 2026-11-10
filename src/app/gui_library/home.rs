//! The GUI Library's Home projection (add-gui-library-panel design D12, the
//! USER-APPROVED GUI Home design): one flat list in the Music list slot whose
//! two sections are heading rows — `Continue watching` first, then
//! `Recently added` — and the selected row's Hero facts. The section rules,
//! the split title parts, and the gutter date are the TUI's flat projection's
//! (`crates/mbv-components/src/home_content/flat.rs`), rebuilt over the GUI's
//! row vocabulary so each row also carries its stable id, row state, and
//! artwork. The projection reads `App` data only, so the TUI's own
//! projections and paint do not change.

use mbv_components::library_panel::content::{ArtworkShape, ArtworkSource};
use mbv_components::library_panel::hero::hero_content_queue;
use mbv_queue::{QueueItem, QueueItemContentId};
use mbv_ui_model::home_latest::{HomeRecentRow, is_home_recent, provider_timestamp_secs};
use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::tab_selection::TabSelection;
use mbv_ui_model::targets::HomeItemTarget;
use mbv_ui_model::ui_util::fmt_publish_date_short;

use super::GuiLibraryArtwork;
use crate::app::App;

/// One Home row's stable identity (design D12): the identity
/// `App::home_stable_target` resolves, addressed by Service ids and never by
/// a row position. `Recent` wraps the queue item's content id, whose own
/// wire form is the `QueueItemContentId` serde's (`provider`/`value`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiHomeRowId {
    /// A `Continue watching` row, addressed by its Emby item id.
    Continue(String),
    /// A `Recently added` row, addressed by the item's queue-item content id.
    Recent(QueueItemContentId),
}

impl GuiHomeRowId {
    /// The stable target the Home paths resolve (`App::home_stable_target`),
    /// as the command arms apply it.
    #[must_use]
    pub fn into_home_item_target(self) -> HomeItemTarget {
        match self {
            Self::Continue(item_id) => HomeItemTarget::continue_item(item_id),
            Self::Recent(content_id) => HomeItemTarget::Recent(content_id),
        }
    }

    /// Whether this row is a `Continue watching` row: the flag
    /// `App::home_play_target` and its enqueue path key their resume
    /// behaviour on.
    #[must_use]
    pub fn is_continue(&self) -> bool {
        matches!(self, Self::Continue(_))
    }
}

/// One item row's state, the shared media-list semantic state's Home arms
/// (`MediaSemanticState::from_queue_item`): a resume position is `Active`
/// with the bounded percentage (no percentage when the runtime is unknown),
/// `played` without a position is `Played`, otherwise the row is `Ordinary`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiHomeRowState {
    Ordinary,
    Played,
    Active { progress: Option<u8> },
}

/// One item row's kind: `Collection` when the row targets a container — a
/// folder-shaped Home item the play path refuses — and `Media` when it
/// targets a playable leaf, which is every other row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiHomeRowKind {
    Collection,
    Media,
}

/// One item row of the flat Home list (design D12): the stable id, the split
/// title parts the TUI's flat projection paints, the gutter date, the
/// Continue Watching progress where the item carries it, and the row's
/// artwork.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiHomeItemRow {
    /// The row's stable identity: the id a select, play, or enqueue command
    /// sends, and the id the frame's `selected` carries.
    pub id: GuiHomeRowId,
    /// The row's primary label: the context part's text when the item splits
    /// its title (a `Recently added` episode's show), else the item's own
    /// title.
    pub title: String,
    /// The row's secondary label: the item's own title when a context part
    /// holds `title`. `None` on an unsplit row.
    pub subtitle: Option<String>,
    /// The gutter date, as the TUI's flat projection formats it:
    /// `fmt_publish_date_short` over the provider timestamp, a `day month`
    /// text the window content paints as-is. `None` on `Continue watching`
    /// rows and on an item with no parseable provider date. The GUI carries
    /// the formatted text — not the raw seconds — so the front end keeps no
    /// date-formatting rule.
    pub date: Option<String>,
    /// The row's kind.
    pub kind: GuiHomeRowKind,
    /// The row's state: the media-list semantic state's Home arms, which
    /// carry the Continue Watching resume progress.
    pub state: GuiHomeRowState,
    /// The row's artwork, display-encoded for Emby items only: the
    /// Audiobookshelf and Feed rows paint without art in Phase 2 (the
    /// USER-APPROVED Home design). A `Some` carries the cached art, the
    /// placeholder, or the loading state, exactly as the queue card
    /// resolves it.
    pub art: Option<GuiLibraryArtwork>,
}

/// The selected row's Hero facts (design D12): one title, ordered meta rows
/// with the duration row's position and the resume-progress row's position,
/// the cleaned overview, and the Hero's artwork. Home has no Workspace, and
/// the projection carries no letter rail or pill data. The facts come from
/// the shared hero producers (`hero_content_queue`), the same facts the TUI
/// paints for the same item, so the two Clients cannot drift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiHomeHero {
    /// The hero's title.
    pub title: String,
    /// The meta rows in display order.
    pub meta_rows: Vec<String>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role, when one does. The shared
    /// Emby hero producer fills none, so a Home Emby hero carries `None`
    /// exactly as the TUI's does; the Audiobookshelf book producer fills a
    /// resume percent when the snapshot carries one.
    pub progress_row: Option<usize>,
    /// The cleaned overview text, when the item's payload carries one.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves: the artwork policy's declared
    /// shape, so the front end places the art at the policy's arm.
    pub art_shape: GuiArtShape,
    /// The Hero's artwork, display-encoded for Emby items only (the
    /// USER-APPROVED Home design: the Audiobookshelf and Feed rows paint
    /// without art in Phase 2), fetched through the image cache at the
    /// artwork policy's chain. `None` on the rows without art; the window
    /// content paints its own placeholder there, as the shared header
    /// painter does.
    pub art: Option<GuiLibraryArtwork>,
}

/// The Hero artwork's box shape, mirroring the artwork policy's declared
/// arms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiArtShape {
    /// 16:9 artwork spanning the pane's content width.
    Landscape,
    /// 1:1 artwork beside the title and meta.
    Square,
    /// 2:3 artwork beside the title and meta.
    Portrait,
}

/// One row of the flat Home list: a section heading, the spacer between the
/// two sections, or an item row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiHomeRow {
    /// A section heading row: the section's name as the TUI paints it.
    Heading { text: String },
    /// The gap between the two sections.
    Spacer,
    /// One item row.
    Item(GuiHomeItemRow),
}

/// The Home projection the GUI Library panel paints (design D12): the flat
/// list's rows in paint order, the core's selected row's identity, and the
/// selected row's Hero facts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiHomeProjection {
    /// Whether the Home content's first fetch has not landed yet: the list
    /// paints its loading line, as the TUI's own `ListSlot::Empty` does while
    /// its source is still loading.
    pub loading: bool,
    /// The flat list's rows in paint order: the `Continue watching` heading
    /// row and item rows, then the `Recently added` heading row and item
    /// rows. A heading only paints above its rows, a section with no rows is
    /// omitted, and the spacer paints only between two non-empty sections —
    /// the TUI's flat list's rules. An all-empty list projects no rows.
    pub rows: Vec<GuiHomeRow>,
    /// The core's selected row (design D2, D12): the row the select command
    /// last named while its row still exists, else the first item row —
    /// Home's Hero default, so the Hero paints as soon as a section lands.
    /// `None` while no item row exists.
    pub selected: Option<GuiHomeRowId>,
    /// The selected row's Hero facts; `None` while no row is selected.
    pub hero: Option<GuiHomeHero>,
}

/// One item row beside the queue item that projects it: the row carries the
/// paint facts, and the hero and the artwork read the item. Internal to one
/// projection run.
struct GuiHomeItem {
    row: GuiHomeItemRow,
    item: QueueItem,
}

impl GuiHomeItem {
    /// The row's stable identity.
    fn id(&self) -> &GuiHomeRowId {
        &self.row.id
    }

    /// The flat row the item paints.
    fn flat_row(&self) -> GuiHomeRow {
        GuiHomeRow::Item(self.row.clone())
    }
}

impl App {
    /// Whether Home is the active destination (design D2, D12): the
    /// position-0 open destination the Music content gate excludes, and the
    /// only active position whose Library section carries `home` content.
    #[must_use]
    pub fn gui_home_destination(&self) -> bool {
        self.tab == TabSelection::Home
    }

    /// The Home projection the GUI Library panel paints (design D12): the
    /// flat list's two sections as heading rows and item rows, the core's
    /// selected row, and its Hero. The time the 14-day cutoff reads is read
    /// once per call, as each TUI Home push reads it once.
    pub fn gui_home_projection(&mut self) -> GuiHomeProjection {
        let now_secs = mbv_render::screens::feeds_model::current_time_secs();
        // The `Continue watching` rows keep the snapshot's order, with no
        // gutter date — the TUI's flat list's rule.
        let continue_items: Vec<QueueItem> = self
            .home_content
            .continue_items
            .iter()
            .cloned()
            .map(|item| QueueItem::Emby(Box::new(item)))
            .collect();
        let mut continue_rows = Vec::new();
        for item in &continue_items {
            let id = GuiHomeRowId::Continue(item.id().to_owned());
            continue_rows.push(self.home_item(item, id, None, None));
        }
        // The `Recently added` rows: the merged store-and-Feeds rows inside
        // the 14-day cutoff (`App::home_recent_rows`'s merge order, newest
        // first), each with its gutter date and the Feeds subscription name
        // the split title reads.
        let mut recent_rows = Vec::new();
        for row in self.home_recent_rows(now_secs) {
            let HomeRecentRow { item, feed_name } = &row;
            if !is_home_recent(provider_timestamp_secs(item), now_secs) {
                continue;
            }
            let id = GuiHomeRowId::Recent(item.content_id());
            let date = provider_timestamp_secs(item).map(fmt_publish_date_short);
            recent_rows.push(self.home_item(item, id, feed_name.as_deref(), date));
        }

        // The selection (design D2, D12): the core's stored row while the
        // list still carries it, else the first item row — Home's Hero
        // default (row 6.2).
        let items: Vec<&GuiHomeItem> = continue_rows.iter().chain(recent_rows.iter()).collect();
        let default_hero = items.first().map(|item| item.row.id.clone());
        let selected = self
            .gui_home_selection
            .clone()
            .filter(|selected| items.iter().any(|item| item.id() == selected))
            .or(default_hero);
        let hero = selected
            .as_ref()
            .and_then(|selected| self.home_hero(&items, selected));

        // The flat rows in paint order: the two sections' heading rows, each
        // omitted when it has no rows, and the spacer only between two
        // non-empty sections.
        let mut rows = Vec::new();
        if !continue_rows.is_empty() {
            rows.push(GuiHomeRow::Heading {
                text: "Continue watching".to_string(),
            });
            rows.extend(continue_rows.iter().map(GuiHomeItem::flat_row));
        }
        if !recent_rows.is_empty() {
            if !continue_rows.is_empty() {
                rows.push(GuiHomeRow::Spacer);
            }
            rows.push(GuiHomeRow::Heading {
                text: "Recently added".to_string(),
            });
            rows.extend(recent_rows.iter().map(GuiHomeItem::flat_row));
        }
        GuiHomeProjection {
            loading: self.home_content.loading,
            rows,
            selected,
            hero,
        }
    }

    /// One item row beside the queue item that projects it (design D12):
    /// the row's title parts, kind, state, date, and artwork read the item
    /// alone.
    fn home_item(
        &mut self,
        item: &QueueItem,
        id: GuiHomeRowId,
        feed_name: Option<&str>,
        date: Option<String>,
    ) -> GuiHomeItem {
        let parts = item.playback_title_parts(feed_name);
        let (title, subtitle) = match &parts.context {
            Some(context) => (context.text.clone(), Some(parts.title.text.clone())),
            None => (parts.title.text.clone(), None),
        };
        let kind = home_item_kind(item);
        let state = home_item_state(item);
        let art = match item {
            // Emby rows fetch their card art through the image cache; the
            // other providers paint without art in Phase 2.
            QueueItem::Emby(emby) => Some(self.gui_home_row_artwork(emby)),
            _ => None,
        };
        GuiHomeItem {
            item: item.clone(),
            row: GuiHomeItemRow {
                id,
                title,
                subtitle,
                date,
                kind,
                state,
                art,
            },
        }
    }

    /// The selected row's Hero facts (design D12): the shared hero
    /// producers' facts for the row's item, with the artwork fetched through
    /// the image cache at the artwork policy's chain. Home has no
    /// Workspace. `None` while the selection names no row of the list, which
    /// a stale frame's id can only miss.
    fn home_hero(
        &mut self,
        items: &[&GuiHomeItem],
        selected: &GuiHomeRowId,
    ) -> Option<GuiHomeHero> {
        let item = items
            .iter()
            .find_map(|item| (item.id() == selected).then_some(*item))?;
        let content = hero_content_queue(&item.item);
        let art_shape = artwork_shape(content.facts.artwork.shape);
        // The artwork rides the artwork policy's declared source: an Emby
        // source is the fetched chain, and the Audiobookshelf covers and the
        // undeclared arms paint without art in Phase 2 (the USER-APPROVED
        // Home design).
        let art = match &content.facts.artwork.source {
            Some(ArtworkSource::Emby {
                item_id,
                series_id,
                image_types,
                cache_key,
            }) => Some(self.gui_home_hero_artwork(cache_key, item_id, series_id, image_types)),
            Some(_) | None => None,
        };
        Some(GuiHomeHero {
            title: content.facts.title.clone(),
            meta_rows: content.facts.meta_rows.clone(),
            duration_row: content.facts.duration_row,
            progress_row: content.facts.progress_row,
            overview: content.overview.clone(),
            art_shape,
            art,
        })
    }
}

/// The item row's kind: an Emby folder is the container the Home play path
/// refuses; every other Home row is a playable leaf.
fn home_item_kind(item: &QueueItem) -> GuiHomeRowKind {
    if matches!(item, QueueItem::Emby(inner) if inner.is_folder) {
        GuiHomeRowKind::Collection
    } else {
        GuiHomeRowKind::Media
    }
}

/// The item row's state, the media-list semantic state's Home arms: a resume
/// position is `Active` with its bounded percentage, `played` without one is
/// `Played`, and an undated music row is `Ordinary` per that state's rule.
/// Shared with the Shows rows' states (design D12), which read the same
/// media-list vocabulary.
pub(in crate::app::gui_library) fn home_item_state(item: &QueueItem) -> GuiHomeRowState {
    media_row_state(&MediaSemanticState::from_queue_item(item))
}

/// One media-list semantic state's row-state arm, mapped onto the shared
/// row-state vocabulary. `from_queue_item` derives `Active`, `Played`, and
/// `Ordinary` only; the `NowPlaying` arm stays exhaustive for the shared
/// vocabulary.
pub(in crate::app::gui_library) fn media_row_state(state: &MediaSemanticState) -> GuiHomeRowState {
    match state {
        MediaSemanticState::Active { progress } | MediaSemanticState::NowPlaying { progress } => {
            GuiHomeRowState::Active {
                progress: progress.map(mbv_ui_model::media_list::ActiveProgress::percent),
            }
        }
        MediaSemanticState::Played => GuiHomeRowState::Played,
        MediaSemanticState::Ordinary => GuiHomeRowState::Ordinary,
    }
}

/// The Hero's art box shape, mirrored from the artwork policy's declared
/// arms. Shared with the Shows heroes (design D12), which read the same
/// policy.
pub(in crate::app::gui_library) fn artwork_shape(shape: ArtworkShape) -> GuiArtShape {
    match shape {
        ArtworkShape::Landscape => GuiArtShape::Landscape,
        ArtworkShape::Square => GuiArtShape::Square,
        ArtworkShape::Portrait => GuiArtShape::Portrait,
    }
}
