//! The GUI Library's Home projection (add-gui-library-panel design D12, the
//! USER-APPROVED GUI Home design): one flat list in the Music list slot whose
//! two sections are heading rows — `Continue watching` first, then
//! `Recently added` — and the selected row's Hero facts. The section rules,
//! the split title parts, and the gutter date are the TUI's flat projection's
//! (`crates/mbv-components/src/home_content/flat.rs`), rebuilt over the GUI's
//! row vocabulary so each row also carries its stable id, row state, trailing
//! text, and art. A `Continue watching` row's trailing text is the time left
//! (gui-delineated-rows design D9). The projection reads `App` data only — and
//! its row art is one cache read per row, never a fetch — so the TUI's own
//! projections and paint do not change.

use mbv_emby_model::TICKS_PER_SECOND;
use mbv_queue::{QueueItem, QueueItemContentId};
use mbv_ui_model::home_latest::{HomeRecentRow, is_home_recent, provider_timestamp_secs};
use mbv_ui_model::library_hero::producers::hero_content_queue;
use mbv_ui_model::library_hero::{ArtworkShape, ArtworkSource};
use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::tab_selection::TabSelection;
use mbv_ui_model::targets::HomeItemTarget;
use mbv_ui_model::ui_util::row_date;

use super::GuiLibraryArtwork;
use super::artwork::{GuiRowArt, GuiRowMediaKind, queue_item_media_kind};
use crate::app::App;
use crate::gui::GuiCore;
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

    /// The id this row takes in `RowsInView` (gui-delineated-rows design D7):
    /// one opaque string the window sends and the core matches its own Home
    /// rows by, so the row art set never names a position. A `Continue`
    /// watching row carries its Emby item id; a `Recently added` row carries
    /// the queue item's content id, provider first.
    #[must_use]
    pub fn rows_in_view_id(&self) -> String {
        match self {
            Self::Continue(item_id) => format!("continue:{item_id}"),
            Self::Recent(content_id) => format!("recent:{}", content_id_text(content_id)),
        }
    }
}

/// The text a queue item's content id takes inside a `RowsInView` Home id
/// (design D7): one provider word, then the provider's own identity fields.
fn content_id_text(id: &QueueItemContentId) -> String {
    match id {
        QueueItemContentId::Emby(value) => format!("emby:{value}"),
        QueueItemContentId::Feed(value) => format!("feed:{value}"),
        QueueItemContentId::Audiobookshelf {
            library_item_id,
            episode_id,
        } => format!(
            "audiobookshelf:{}:{library_item_id}:{episode_id}",
            library_item_id.len()
        ),
        QueueItemContentId::AudiobookshelfBook { library_item_id } => {
            format!("audiobookshelfBook:{library_item_id}")
        }
        QueueItemContentId::Twitch(value) => format!("twitch:{value}"),
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
    /// The row's trailing text (gui-delineated-rows design D2, D9): the time
    /// left on a `Continue watching` row, and the gutter date on a `Recently
    /// added` row. The GUI carries the formatted text — not the raw seconds —
    /// so the window keeps no time or date rule.
    pub trailing: Option<String>,
    /// The row's kind.
    pub kind: GuiHomeRowKind,
    /// The row's state: the shared media-list semantic state the queue-item
    /// constructor derives, which carries the Continue Watching resume
    /// progress.
    pub state: MediaSemanticState,
    /// The row's art (gui-delineated-rows design D4): the cached row image,
    /// a fetch in flight, a source with no image, or a row kind with no art
    /// source.
    pub art: GuiRowArt,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKind,
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

impl GuiCore {
    /// Whether Home is the active destination (design D2, D12): the
    /// position-0 open destination the Music content gate excludes, and the
    /// only active position whose Library section carries `home` content.
    #[must_use]
    pub fn gui_home_destination(&self) -> bool {
        home_destination(&self.app)
    }

    /// The Home projection the GUI Library panel paints (design D12).
    pub fn gui_home_projection(&mut self) -> GuiHomeProjection {
        home_projection(&mut self.app)
    }
}

/// Whether Home is the active destination over the `App` the GUI core owns
/// (design D2, D12).
#[must_use]
pub(in crate::app::gui_library) fn home_destination(app: &App) -> bool {
    app.tab == TabSelection::Home
}

/// The Home projection over the `App` the GUI core owns (design D12): the flat
/// list's two sections as heading rows and item rows, the core's selected row,
/// and its Hero. The time the 14-day cutoff reads is read once per call, as
/// each TUI Home push reads it once.
pub(in crate::app::gui_library) fn home_projection(app: &mut App) -> GuiHomeProjection {
    let now_secs = mbv_ui_model::feed_age::current_time_secs();
    // The `Continue watching` rows keep the snapshot's order, with no gutter
    // date — the TUI's flat list's rule.
    let continue_items: Vec<QueueItem> = app
        .home_content
        .continue_items
        .iter()
        .cloned()
        .map(|item| QueueItem::Emby(Box::new(item)))
        .collect();
    let mut continue_rows = Vec::new();
    for item in &continue_items {
        let id = GuiHomeRowId::Continue(item.id().to_owned());
        continue_rows.push(home_item(app, item, id, None, time_left(item)));
    }
    // The `Recently added` rows: the merged store-and-Feeds rows inside the
    // 14-day cutoff (`App::home_recent_rows`'s merge order, newest first),
    // each with its gutter date and the Feeds subscription name the split
    // title reads.
    let mut recent_rows = Vec::new();
    for row in app.home_recent_rows(now_secs) {
        let HomeRecentRow { item, feed_name } = &row;
        if !is_home_recent(provider_timestamp_secs(item), now_secs) {
            continue;
        }
        let id = GuiHomeRowId::Recent(item.content_id());
        let date = row_date(provider_timestamp_secs(item));
        recent_rows.push(home_item(app, item, id, feed_name.as_deref(), date));
    }

    // The selection (design D2, D12): the core's stored row while the list
    // still carries it, else the first item row — Home's Hero default (row
    // 6.2).
    let items: Vec<&GuiHomeItem> = continue_rows.iter().chain(recent_rows.iter()).collect();
    let default_hero = items.first().map(|item| item.row.id.clone());
    let selected = app
        .gui
        .home_selection
        .clone()
        .filter(|selected| items.iter().any(|item| item.id() == selected))
        .or(default_hero);
    let hero = selected
        .as_ref()
        .and_then(|selected| home_hero(app, &items, selected));

    // The flat rows in paint order: the sections' heading rows, each omitted
    // when it has no rows, and the spacer only between two non-empty
    // sections.
    let sections: [(&str, Vec<GuiHomeRow>); 2] = [
        (
            "Continue watching",
            continue_rows.iter().map(GuiHomeItem::flat_row).collect(),
        ),
        (
            "Recently added",
            recent_rows.iter().map(GuiHomeItem::flat_row).collect(),
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
        loading: app.home_content.loading,
        rows,
        selected,
        hero,
    }
}

/// One item row beside the queue item that projects it (design D12): the
/// row's title parts, kind, state, trailing text, and art read the item alone.
/// `trailing` is the caller's text for a row whose trailing fact is not the
/// time left (a `Recently added` row's date).
fn home_item(
    app: &App,
    item: &QueueItem,
    id: GuiHomeRowId,
    feed_name: Option<&str>,
    trailing: Option<String>,
) -> GuiHomeItem {
    let parts = item.playback_title_parts(feed_name);
    let (title, subtitle) = match &parts.context {
        Some(context) => (context.text.clone(), Some(parts.title.text.clone())),
        None => (parts.title.text.clone(), None),
    };
    let kind = home_item_kind(item);
    let state = MediaSemanticState::from_queue_item(item);
    let media = queue_item_media_kind(item);
    let art = app.gui_row_art(app.queue_item_row_source(item).as_ref());
    GuiHomeItem {
        item: item.clone(),
        row: GuiHomeItemRow {
            id,
            title,
            subtitle,
            trailing,
            kind,
            state,
            art,
            media,
        },
    }
}

/// The selected row's Hero facts (design D12): the shared hero producers'
/// facts for the row's item, with the artwork fetched through the image cache
/// at the artwork policy's chain. Home has no Workspace. `None` while the
/// selection names no row of the list, which a stale frame's id can only miss.
fn home_hero(
    app: &mut App,
    items: &[&GuiHomeItem],
    selected: &GuiHomeRowId,
) -> Option<GuiHomeHero> {
    let item = items
        .iter()
        .find_map(|item| (item.id() == selected).then_some(*item))?;
    let content = hero_content_queue(&item.item);
    // The artwork rides the artwork policy's declared source: an Emby source
    // is the fetched chain, an Audiobookshelf cover is the Service-scoped
    // cover the Podcasts and Audiobooks Heroes read, and an undeclared arm (a
    // feed entry) paints without art.
    let art = match content.facts.artwork.source.as_ref() {
        Some(ArtworkSource::AudiobookshelfCover {
            library_item_id,
            book,
        }) => super::artwork::abs_cover_art(app, library_item_id, *book),
        source => super::artwork::emby_hero_art(app, source),
    };
    let art_shape = artwork_shape(super::artwork::hero_shape(
        app,
        content.facts.artwork.shape,
        art.as_ref(),
    ));
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

/// The item row's kind: an Emby folder is the container the Home play path
/// refuses; every other Home row is a playable leaf.
fn home_item_kind(item: &QueueItem) -> GuiHomeRowKind {
    if matches!(item, QueueItem::Emby(inner) if inner.is_folder) {
        GuiHomeRowKind::Collection
    } else {
        GuiHomeRowKind::Media
    }
}

/// The time left in a `Continue watching` row's item (design D9): the item's
/// runtime less its saved position, as whole minutes rounded up — `"{m}m
/// left"` under one hour and `"{h}h {m}m left"` from one hour. `None` on an
/// item whose runtime the Service does not declare, which leaves the trailing
/// place empty.
fn time_left(item: &QueueItem) -> Option<String> {
    let QueueItem::Emby(emby) = item else {
        return None;
    };
    if emby.runtime_ticks <= 0 {
        return None;
    }
    let remaining = emby
        .runtime_ticks
        .saturating_sub(emby.playback_position_ticks)
        .max(0);
    let timer = 60 * TICKS_PER_SECOND;
    let minutes = remaining.saturating_add(timer - 1) / timer;
    Some(if minutes < 60 {
        format!("{minutes}m left")
    } else {
        format!("{}h {}m left", minutes / 60, minutes % 60)
    })
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
