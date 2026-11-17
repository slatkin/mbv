//! The GUI Library's Home projection (add-gui-library-panel design D12, the
//! USER-APPROVED GUI Home design): one flat list in the Music list slot whose
//! two sections are heading rows — `Continue watching` first, then
//! `Recently added` — and the selected row's Hero facts. The section rules,
//! the split title parts, and the gutter date are the TUI's flat projection's
//! (`crates/mbv-components/src/home_content/flat.rs`), rebuilt over the GUI's
//! row vocabulary so each row also carries its stable id, row state, and
//! artwork. The projection reads `App` data only, so the TUI's own
//! projections and paint do not change.

use mbv_components::library_panel::content::ArtworkShape;
use mbv_components::library_panel::hero::hero_content_queue;
use mbv_emby_model::EmbyItem;
use mbv_queue::{QueueItem, QueueItemContentId};
use mbv_ui_model::home_latest::{HomeRecentRow, is_home_recent, provider_timestamp_secs};
use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::tab_selection::TabSelection;
use mbv_ui_model::targets::HomeItemTarget;
use mbv_ui_model::ui_util::row_date;

use super::GuiLibraryArtwork;
use crate::app::App;
use crate::app::state::events::LibEvent;
use mbv_ui_model::meta::MetaSegment;

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
    /// A `Latest episodes` row, addressed by its Emby item id
    /// (gui-collection-dropdown design D9).
    LatestEpisode(String),
    /// An `Upcoming` row, addressed by the premiere's stable key. The row is
    /// display-only, so no Home command acts on it.
    Upcoming(String),
}

impl GuiHomeRowId {
    /// The stable target the Home paths resolve (`App::home_stable_target`),
    /// as the command arms apply it. A `Latest episodes` row resolves through
    /// the `Recently added` Emby path, so it plays and enqueues as such a row
    /// does, never through the Continue Watching resume path. An `Upcoming`
    /// row resolves no target: it is display-only (design D9).
    #[must_use]
    pub fn into_home_item_target(self) -> Option<HomeItemTarget> {
        match self {
            Self::Continue(item_id) => Some(HomeItemTarget::continue_item(item_id)),
            Self::Recent(content_id) => Some(HomeItemTarget::Recent(content_id)),
            Self::LatestEpisode(item_id) => {
                Some(HomeItemTarget::Recent(QueueItemContentId::Emby(item_id)))
            }
            Self::Upcoming(_) => None,
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
    /// the formatted text — not the raw seconds — so the window keeps no
    /// date-formatting rule.
    pub date: Option<String>,
    /// The row's kind.
    pub kind: GuiHomeRowKind,
    /// The row's state: the shared media-list semantic state the queue-item
    /// constructor derives, which carries the Continue Watching resume
    /// progress.
    pub state: MediaSemanticState,
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
    pub meta_rows: Vec<Vec<MetaSegment>>,
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
    /// shape, so the window places the art at the policy's arm.
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

/// The GUI Home `Latest episodes` fetch's completion (gui-collection-dropdown
/// design D9): one Emby TV library's latest episodes, sent through the shared
/// library event channel. Only the GUI Client's launch starts one, so the TUI
/// never receives it.
#[derive(Debug)]
pub(in crate::app) struct GuiHomeLatestEvent {
    pub(in crate::app) library_id: String,
    pub(in crate::app) result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
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
            let date = row_date(provider_timestamp_secs(item));
            recent_rows.push(self.home_item(item, id, feed_name.as_deref(), date));
        }

        // The `Latest episodes` rows (gui-collection-dropdown design D9):
        // every Emby TV library's latest episodes as one list, newest first,
        // with the split show/episode title and the provider-date gutter the
        // TUI's TV Latest rows paint.
        let mut latest_rows = Vec::new();
        for episode in self.gui_home_latest_items() {
            let item = QueueItem::Emby(Box::new(episode));
            let id = GuiHomeRowId::LatestEpisode(item.id().to_owned());
            let date = row_date(provider_timestamp_secs(&item));
            latest_rows.push(self.home_item(&item, id, None, date));
        }
        // The `Upcoming` rows (design D9): every TV library's premieres as one
        // list, soonest first. An Upcoming row is display-only, so it carries
        // no queue item, no artwork, and no selectable identity.
        let upcoming_rows = self.gui_home_upcoming_rows();

        // The selection (design D2, D12): the core's stored row while the
        // list still carries it, else the first item row — Home's Hero
        // default (row 6.2). An `Upcoming` row is never a candidate: it is
        // display-only, so the Hero keeps whatever row it showed.
        let items: Vec<&GuiHomeItem> = continue_rows
            .iter()
            .chain(recent_rows.iter())
            .chain(latest_rows.iter())
            .collect();
        let default_hero = items.first().map(|item| item.row.id.clone());
        let selected = self
            .gui_home_selection
            .clone()
            .filter(|selected| items.iter().any(|item| item.id() == selected))
            .or(default_hero);
        let hero = selected
            .as_ref()
            .and_then(|selected| self.home_hero(&items, selected));

        // The flat rows in paint order: the sections' heading rows, each
        // omitted when it has no rows, and the spacer only between two
        // non-empty sections.
        let sections: [(&str, Vec<GuiHomeRow>); 4] = [
            (
                "Continue watching",
                continue_rows.iter().map(GuiHomeItem::flat_row).collect(),
            ),
            (
                "Recently added",
                recent_rows.iter().map(GuiHomeItem::flat_row).collect(),
            ),
            (
                "Latest episodes",
                latest_rows.iter().map(GuiHomeItem::flat_row).collect(),
            ),
            (
                "Upcoming",
                upcoming_rows.into_iter().map(GuiHomeRow::Item).collect(),
            ),
        ];
        let mut rows = Vec::new();
        for (heading, section_rows) in sections {
            if section_rows.is_empty() {
                continue;
            }
            if !rows.is_empty() {
                rows.push(GuiHomeRow::Spacer);
            }
            rows.push(GuiHomeRow::Heading {
                text: heading.to_string(),
            });
            rows.extend(section_rows);
        }
        GuiHomeProjection {
            loading: self.home_content.loading,
            rows,
            selected,
            hero,
        }
    }

    /// Request the GUI Home's per-library sections at launch
    /// (gui-collection-dropdown design D9): each Emby TV library's latest
    /// episodes through one worker fetch, and its eager Upcoming snapshot
    /// through the once-per-session guard the TV tab already uses. The
    /// launch's catalog-landed step calls this, never a frame, and both
    /// requests are idempotent, so a repeated call re-requests nothing.
    pub(in crate::app) fn spawn_gui_home_sections(&mut self) {
        let tv_libraries: Vec<(usize, String)> = (0..self.libs.len())
            .filter(|lib_idx| self.libs[*lib_idx].library.collection_type == "tvshows")
            .map(|lib_idx| (lib_idx, self.libs[lib_idx].library.id.clone()))
            .collect();
        for (lib_idx, library_id) in tv_libraries {
            self.spawn_gui_home_tv_latest(library_id);
            self.maybe_spawn_tv_upcoming_snapshot(lib_idx);
        }
    }

    /// Start one TV library's `Latest episodes` fetch (design D9): the same
    /// `get_latest_episodes(parent_id, 30)` call the TUI's Latest level makes,
    /// on a worker thread, landing in `gui_home_tv_latest` through its own
    /// event. It never touches `nav_stack`, so the TUI's levels are the TUI's.
    fn spawn_gui_home_tv_latest(&mut self, library_id: String) {
        if self.gui_home_tv_latest.contains_key(&library_id) {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        // The empty entry marks the request, so a later launch step starts no
        // second fetch; the completion replaces it.
        self.gui_home_tv_latest
            .insert(library_id.clone(), Vec::new());
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let result = client.get_latest_episodes(&library_id, 30);
            let _ = tx.send(LibEvent::GuiHomeLatest(GuiHomeLatestEvent {
                library_id,
                result,
            }));
        });
    }

    /// Land one TV library's `Latest episodes` fetch (design D9): a loaded
    /// list replaces the request mark, and a failure leaves the library with
    /// no rows, so its section holds nothing and the log names it.
    pub(in crate::app) fn handle_gui_home_latest(&mut self, event: GuiHomeLatestEvent) {
        let GuiHomeLatestEvent { library_id, result } = event;
        match result {
            Ok(items) => {
                self.gui_home_tv_latest.insert(library_id, items);
            }
            Err(error) => {
                tracing::info!(
                    name: "gui_library.home_latest.failed",
                    target: "gui_library",
                    library_id,
                    error = %error,
                    "the library's latest episodes failed to load"
                );
            }
        }
    }

    /// The merged `Latest episodes` items (design D9): every Emby TV library's
    /// list as one, the newest `date_added` first with the undated items last,
    /// and de-duplicated by item id so an episode both libraries report shows
    /// one time.
    fn gui_home_latest_items(&self) -> Vec<EmbyItem> {
        let mut items: Vec<EmbyItem> = self
            .gui_home_tv_latest
            .values()
            .flatten()
            .cloned()
            .collect();
        items.sort_by_cached_key(|item| {
            (
                std::cmp::Reverse(mbv_ui_model::home_latest::emby_date_added_secs(item)),
                item.id.clone(),
            )
        });
        let mut seen = std::collections::HashSet::new();
        items.retain(|item| seen.insert(item.id.clone()));
        items
    }

    /// The merged `Upcoming` rows (design D9): every TV library's premieres as
    /// one list, the soonest premiere first with the undated rows last. An
    /// Upcoming row is display-only, so it carries no queue item and no
    /// artwork.
    fn gui_home_upcoming_rows(&self) -> Vec<GuiHomeItemRow> {
        let mut premieres: Vec<&mbv_emby_model::UpcomingPremiere> =
            self.tv_upcoming_snapshots.values().flatten().collect();
        premieres.sort_by_cached_key(|premiere| {
            let secs = mbv_feed::parse_pub_date_secs(&premiere.premiere_date);
            (
                secs.is_none(),
                secs.unwrap_or_default(),
                premiere.stable_key(),
            )
        });
        premieres
            .into_iter()
            .map(|premiere| {
                let secs = mbv_feed::parse_pub_date_secs(&premiere.premiere_date);
                GuiHomeItemRow {
                    id: GuiHomeRowId::Upcoming(premiere.stable_key()),
                    title: premiere.series_name.clone(),
                    subtitle: Some(premiere.title.clone()),
                    date: row_date(secs),
                    kind: GuiHomeRowKind::Media,
                    state: MediaSemanticState::Ordinary,
                    art: None,
                }
            })
            .collect()
    }

    /// The `Latest episodes` item the target names (design D9): the row a
    /// `LatestEpisode` identity resolves through the `Recently added` Emby
    /// path. `None` for a content id that is not an Emby item id, and for an
    /// item no library's list holds.
    pub(in crate::app) fn gui_home_latest_item(
        &self,
        content_id: &QueueItemContentId,
    ) -> Option<QueueItem> {
        let QueueItemContentId::Emby(item_id) = content_id else {
            return None;
        };
        self.gui_home_tv_latest
            .values()
            .flatten()
            .find(|item| item.id == *item_id)
            .cloned()
            .map(|item| QueueItem::Emby(Box::new(item)))
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
        let state = MediaSemanticState::from_queue_item(item);
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
        // The artwork rides the artwork policy's declared source: an Emby
        // source is the fetched chain, and the Audiobookshelf covers and the
        // undeclared arms paint without art in Phase 2 (the USER-APPROVED
        // Home design).
        let art = self.gui_emby_hero_art(content.facts.artwork.source.as_ref());
        let art_shape =
            artwork_shape(self.gui_hero_shape(content.facts.artwork.shape, art.as_ref()));
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
