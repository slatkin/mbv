//! The Music destination's Library content (add-gui-library-panel design D5,
//! D6, D7): the group pills, the artist and album tree, and the Hero of the
//! picked album. `gui-change-driven-sections` design D8 splits the destination
//! into a list part ([`GuiMusicList`]) and a detail part ([`GuiMusicDetail`]).
//! Both ride the `libraryList` and `libraryDetail` sections.

use mbv::{
    GuiLibraryMenuItem, GuiMusicAlbum, GuiMusicArtist, GuiMusicHero, GuiMusicProjection,
    GuiMusicTrack, GuiMusicTrackTotal,
};
use mbv_ui_model::music_grouping::ArtistKey;
use serde::Serialize;

use super::GuiMusicMenuEntryFrame;
use crate::art::GuiArtRef;

/// The Music destination's list part (design D8): the group pills, the union
/// of the selected groups' artists with their albums and cached tracklists,
/// and the load facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicList {
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
}

/// The Music destination's detail part (design D8): the Hero of the picked
/// album, its square artwork, and the id of the playing track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicDetail {
    /// The Hero's facts; `None` while the window content has picked no album
    /// of the union.
    pub hero: Option<GuiMusicHeroFrame>,
    /// The Hero's square artwork (design D5): present exactly when `hero` is.
    pub art: Option<GuiArtRef>,
    /// The id of the Emby track playing at the playhead (add-gui-library-panel,
    /// the Library's playing row): the Queue panel's own authority — the Owner
    /// status and the playback queue's item at the active index — carried so
    /// the tracklist's and the list's playing rows paint what the Queue's
    /// active slot paints. `None` while idle or while the active slot holds no
    /// Emby item.
    pub playing_track_id: Option<String>,
}

impl GuiMusicList {
    /// The Music list part for `projection` with the library index the
    /// set-collections command names.
    pub(crate) fn from_projection(lib_idx: usize, projection: &GuiMusicProjection) -> Self {
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
        }
    }
}

impl GuiMusicDetail {
    /// The Music detail part for `projection`: the Hero with the picked item's
    /// menu `menu`, its artwork `art` (present exactly when the projection holds
    /// a Hero), and the playing track's id `playing_track_id`.
    pub(crate) fn from_projection(
        projection: &GuiMusicProjection,
        art: Option<GuiArtRef>,
        menu: &[GuiLibraryMenuItem],
        playing_track_id: Option<String>,
    ) -> Self {
        let hero = projection.hero.as_ref().map(|hero| {
            let mut frame = GuiMusicHeroFrame::from(hero);
            frame.menu = menu.iter().map(GuiMusicMenuEntryFrame::from).collect();
            frame
        });
        let art = hero.as_ref().and(art);
        Self {
            hero,
            art,
            playing_track_id,
        }
    }
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
