//! The Library section of a frame (add-gui-library-panel design D2): the
//! active destination's content, carried whole only when its revision changes,
//! with the Hero's artwork as the `data:` URLs the window paints.
//!
//! A Library list can hold thousands of rows, so the section carries a
//! revision and a frame re-carries the content only when that revision
//! changes; the front end keeps the last carried value. The art is part of the
//! content, so a landed image bumps the revision and rides the same frame as
//! the facts it belongs to.

use mbv::{
    GuiLibraryArtwork, GuiLibraryMenuItem, GuiMusicAlbum, GuiMusicArtist, GuiMusicHero,
    GuiMusicProjection, GuiMusicTrack, GuiMusicTrackTotal,
};
use mbv_images::QUEUE_CARD_PLACEHOLDER_BYTES;
use mbv_ui_model::music_grouping::ArtistKey;
use serde::Serialize;

use crate::artwork::data_url;
use mbv::GuiLibraryTab;

/// The Library panel's frame section (design D2): the content revision, and
/// the content itself when this frame carries it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiLibraryFrame {
    /// The revision of the content the window holds. It changes when any part
    /// of the content changes, the art included.
    pub revision: u64,
    /// The content this frame carries. `None` while the revision is the one
    /// the window already holds: the front end keeps its last carried value.
    /// A window-content reload's `report_layout` re-carries it. Boxed, so an
    /// unchanged frame's payload keeps only the revision.
    pub content: Option<Box<GuiLibraryContent>>,
}

/// The content of the Library panel (design D2): the destination tabs the
/// top bar paints, and the active destination's pills, list rows, Hero facts,
/// and Hero artwork.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiLibraryContent {
    /// The presented destination tabs in catalog order (design D2): one per
    /// Grouped Music library, phase 1's presented set. The top bar paints
    /// only these — no stub for a destination the GUI does not present yet.
    pub tabs: Vec<GuiLibraryTabFrame>,
    /// The active tab's position while the active destination is one of the
    /// tabs (design D2): the tab the top bar marks current. `None` for Home,
    /// Feeds, and every destination the GUI does not present yet.
    pub current_tab: Option<usize>,
    /// The Grouped Music library's content while Music is the active
    /// destination; `None` for a destination that presents no content yet.
    pub music: Option<GuiMusicFrame>,
}

/// One destination tab of the top bar (design D2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiLibraryTabFrame {
    /// The tab position the open-destination command takes, where 0 is Home.
    pub position: usize,
    /// The tab's label: the library's display name.
    pub label: String,
}

impl From<GuiLibraryTab> for GuiLibraryTabFrame {
    fn from(tab: GuiLibraryTab) -> Self {
        Self {
            position: tab.position,
            label: tab.label,
        }
    }
}

/// The Music destination's content (design D5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicFrame {
    /// The Grouped Music library index the set-collections command names
    /// (design D2, D6): the library whose group pills these are.
    pub lib_idx: usize,
    /// The group pills in the group level's order (design D6).
    pub pills: Vec<GuiMusicPillFrame>,
    /// The selected collection names in pill order (design D6): the names
    /// the list header and the Library Status bar location read, carried
    /// once for the whole panel. The Hero header's own copy rides in
    /// `hero.collections`, present exactly when the Hero is.
    pub collection_names: Vec<String>,
    /// Whether one of the selected groups' album lists has not landed yet.
    pub loading: bool,
    /// The failure line naming the selected groups whose album list failed.
    pub failure: Option<String>,
    /// The union of the selected groups' artists in artist sort-name order.
    pub artists: Vec<GuiMusicArtistFrame>,
    /// The Hero's facts; `None` while the window content has picked no album
    /// of the union.
    pub hero: Option<GuiMusicHeroFrame>,
    /// The Hero's square artwork (design D5): present exactly when `hero` is.
    pub art: Option<GuiLibraryArt>,
    /// The id of the Emby track playing at the playhead (add-gui-library-panel,
    /// the Library's playing row): the Queue panel's own authority — the Owner
    /// status and the playback queue's item at the active index — carried so
    /// the tracklist's and the list's playing rows paint what the Queue's
    /// active slot paints. `None` while idle or while the active slot holds no
    /// Emby item.
    pub playing_track_id: Option<String>,
    /// The right-click item menu the window content last opened (the row-menu
    /// command): the item it named with the TUI menu builder's full entries for
    /// it. `None` while no right-click has named an item.
    pub row_menu: Option<GuiMusicRowMenu>,
}

/// The Library's right-click item menu (the row-menu command): the item the
/// entries are about, and the TUI menu builder's entries in the builder's
/// order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicRowMenu {
    /// The item's stable Service id the command named.
    pub item_id: String,
    /// The TUI menu builder's entries for the item.
    pub entries: Vec<GuiMusicMenuEntryFrame>,
}

/// One group pill of the Music destination (design D6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicPillFrame {
    /// The group's stable Service id: the set-collections command and the
    /// collection store key it.
    pub group_id: String,
    /// The pill's label.
    pub name: String,
    /// Whether the core's stored selection holds the group.
    pub selected: bool,
    /// Whether the pill is a collection pill (multi-select) rather than a
    /// single-select one. Every Music pill is a collection pill.
    pub collection: bool,
}

/// One artist row of the Music list (design D7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicArtistFrame {
    /// The stable artist identity the union groups by.
    pub artist_key: GuiArtistKey,
    /// The artist's display name.
    pub name: String,
    /// The letter heading above the row, and the rail's letter: the first
    /// letter of the article-stripped sort name, or `#`.
    pub sort_letter: char,
    /// The artist's albums in the union's order.
    pub albums: Vec<GuiMusicAlbumFrame>,
}

/// An artist's stable identity, mirroring `mbv_ui_model::ArtistKey`: the
/// Service-resolved identity, or the fallback name when the Service resolves
/// none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum GuiArtistKey {
    Service(String),
    Fallback(String),
}

/// One album row of the Music list, and the Hero's picked album.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicAlbumFrame {
    /// The album's stable Service id: the commands that name it, and the key
    /// its artwork and its track fetch use.
    pub album_id: String,
    /// The album's display name.
    pub name: String,
    /// The album's release year; empty when the Service holds none.
    pub year: String,
    /// The cached tracklist; `None` until the album node's open loads it.
    pub tracks: Option<Vec<GuiMusicTrackFrame>>,
    /// The album's track count and total time, present exactly when `tracks`
    /// is.
    pub total: Option<GuiMusicTrackTotalFrame>,
}

/// One tracklist row of the album block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicTrackFrame {
    /// The track's stable Service id.
    pub track_id: String,
    /// The track's display name.
    pub name: String,
    /// The number the row paints.
    pub number: i64,
    /// The runtime, `M:SS` or `H:MM:SS`; empty when unknown.
    pub duration: String,
}

/// A track count with its total time, mirroring `mbv::GuiMusicTrackTotal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicTrackTotalFrame {
    pub count: usize,
    pub duration: String,
}

/// The Hero's facts for the picked album (design D5): the artist header, the
/// picked album block, and its tracklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicHeroFrame {
    /// The picked album's artist.
    pub artist: String,
    /// The selected collection names in pill order.
    pub collections: Vec<String>,
    /// The artist's album count in the union.
    pub album_count: usize,
    /// The artist's track count and total time, present once every one of the
    /// artist's union albums holds a cached tracklist.
    pub artist_tracks: Option<GuiMusicTrackTotalFrame>,
    /// The picked album, with its tracklist when the cache holds it.
    pub album: GuiMusicAlbumFrame,
    /// The picked item's menu entries (design D8): the shared builder's entries
    /// for the album, which More actions paints. Empty when the item offers no
    /// menu.
    pub menu: Vec<GuiMusicMenuEntryFrame>,
}

/// One entry of the picked item's menu, as the window paints it (design D8):
/// the label, the key hint the row menu shows when the entry has one, and the
/// entry's index in the shared builder's order — the index a chosen entry
/// sends back, so a front end that paints only some entries still executes
/// the one it painted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicMenuEntryFrame {
    pub label: String,
    pub key_hint: Option<String>,
    pub index: usize,
}

/// The Library Hero's square artwork (design D5), as the window paints it.
/// The image and placeholder payloads are `data:` URLs: the bytes are the
/// mbv image cache's, so the window fetches no image itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum GuiLibraryArt {
    /// The cached artwork's display encode, as a `data:image/jpeg` URL.
    Image(String),
    /// The bundled placeholder, as a `data:image/webp` URL.
    Placeholder(String),
    /// The art fetch is in flight or awaits its retry.
    Loading,
}

impl GuiMusicFrame {
    /// The Music destination's content for `projection`, with the library
    /// index the set-collections command names, the Hero's artwork `art` —
    /// present exactly when the projection holds a Hero — the picked item's
    /// menu `menu`, and the playing track's id `playing_track_id`.
    pub(crate) fn from_projection(
        lib_idx: usize,
        projection: &GuiMusicProjection,
        art: Option<GuiLibraryArt>,
        menu: &[GuiLibraryMenuItem],
        row_menu: Option<&mbv::GuiLibraryRowMenu>,
        playing_track_id: Option<String>,
    ) -> Self {
        let hero = projection.hero.as_ref().map(|hero| {
            let mut frame = GuiMusicHeroFrame::from(hero);
            frame.menu = menu.iter().map(GuiMusicMenuEntryFrame::from).collect();
            frame
        });
        let art = hero.as_ref().and(art);
        Self {
            lib_idx,
            pills: projection
                .pills
                .iter()
                .map(|pill| GuiMusicPillFrame {
                    group_id: pill.group_id.clone(),
                    name: pill.name.clone(),
                    selected: pill.selected,
                    collection: true,
                })
                .collect(),
            collection_names: projection.collection_names.clone(),
            loading: projection.loading,
            failure: projection.failure.clone(),
            artists: projection
                .artists
                .iter()
                .map(GuiMusicArtistFrame::from)
                .collect(),
            art,
            hero,
            playing_track_id,
            row_menu: row_menu.map(GuiMusicRowMenu::from),
        }
    }
}

impl From<&mbv::GuiLibraryRowMenu> for GuiMusicRowMenu {
    fn from(menu: &mbv::GuiLibraryRowMenu) -> Self {
        Self {
            item_id: menu.item_id.clone(),
            entries: menu
                .entries
                .iter()
                .map(GuiMusicMenuEntryFrame::from)
                .collect(),
        }
    }
}

impl From<&GuiMusicArtist> for GuiMusicArtistFrame {
    fn from(artist: &GuiMusicArtist) -> Self {
        Self {
            artist_key: GuiArtistKey::from(&artist.artist_key),
            name: artist.name.clone(),
            sort_letter: artist.sort_letter,
            albums: artist.albums.iter().map(GuiMusicAlbumFrame::from).collect(),
        }
    }
}

impl From<&GuiMusicAlbum> for GuiMusicAlbumFrame {
    fn from(album: &GuiMusicAlbum) -> Self {
        Self {
            album_id: album.album_id.clone(),
            name: album.name.clone(),
            year: album.year.clone(),
            tracks: album
                .tracks
                .as_ref()
                .map(|tracks| tracks.iter().map(GuiMusicTrackFrame::from).collect()),
            total: album.total.as_ref().map(GuiMusicTrackTotalFrame::from),
        }
    }
}

impl From<&GuiMusicTrack> for GuiMusicTrackFrame {
    fn from(track: &mbv::GuiMusicTrack) -> Self {
        Self {
            track_id: track.track_id.clone(),
            name: track.name.clone(),
            number: track.number,
            duration: track.duration.clone(),
        }
    }
}

impl From<&GuiMusicTrackTotal> for GuiMusicTrackTotalFrame {
    fn from(total: &GuiMusicTrackTotal) -> Self {
        Self {
            count: total.count,
            duration: total.duration.clone(),
        }
    }
}

impl From<&GuiLibraryMenuItem> for GuiMusicMenuEntryFrame {
    fn from(entry: &GuiLibraryMenuItem) -> Self {
        Self {
            label: entry.label.clone(),
            key_hint: entry.key_hint.clone(),
            index: entry.index,
        }
    }
}

impl From<&GuiMusicHero> for GuiMusicHeroFrame {
    fn from(hero: &GuiMusicHero) -> Self {
        Self {
            artist: hero.artist.clone(),
            collections: hero.collections.clone(),
            album_count: hero.album_count,
            artist_tracks: hero
                .artist_tracks
                .as_ref()
                .map(GuiMusicTrackTotalFrame::from),
            album: GuiMusicAlbumFrame::from(&hero.album),
            menu: Vec::new(),
        }
    }
}

impl From<&ArtistKey> for GuiArtistKey {
    fn from(key: &ArtistKey) -> Self {
        match key {
            ArtistKey::Service(id) => Self::Service(id.clone()),
            ArtistKey::Fallback(id) => Self::Fallback(id.clone()),
        }
    }
}

impl From<&GuiLibraryArtwork> for GuiLibraryArt {
    fn from(artwork: &GuiLibraryArtwork) -> Self {
        match artwork {
            GuiLibraryArtwork::Image { jpeg, .. } => Self::Image(data_url("image/jpeg", jpeg)),
            GuiLibraryArtwork::Placeholder => {
                Self::Placeholder(data_url("image/webp", QUEUE_CARD_PLACEHOLDER_BYTES))
            }
            GuiLibraryArtwork::Loading => Self::Loading,
        }
    }
}
