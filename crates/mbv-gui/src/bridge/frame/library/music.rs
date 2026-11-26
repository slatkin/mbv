//! The Music destination's Library content (add-gui-library-panel design D5,
//! D6, D7): the group collection selector, the artist and album tree, and the
//! Hero of the picked album. `gui-change-driven-sections` design D8 splits the destination
//! into a list part ([`GuiMusicList`]) and a detail part ([`GuiMusicDetail`]).
//! Both ride the `libraryList` and `libraryDetail` sections.

use crate::gui_core::{
    GuiCollections, GuiLibraryMenuItem, GuiMusicAlbum, GuiMusicArtist, GuiMusicArtistPage,
    GuiMusicHero, GuiMusicHeroSubject, GuiMusicProjection, GuiMusicTrack, GuiMusicTrackTotal,
};
use mbv_ui_model::music_grouping::ArtistKey;

use super::{GuiMusicMenuEntryFrame, GuiRowArtFrame, GuiRowMediaKindFrame};
use crate::art::GuiArtRef;

/// The Music destination's list part (design D8): the group selector, the
/// union of the selected groups' artists with their albums and cached
/// tracklists, and the load facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiMusicList {
    /// The Grouped Music library index the collection commands name
    /// (design D2, D6): the library whose groups these are.
    pub lib_idx: usize,
    /// The destination's collection selector (gui-collection-dropdown design
    /// D6): All, every group in the group level's order, and the Groups noun.
    /// The names the header's selection label, the filter's
    /// placeholder, and the Library Status bar location read derive from it
    /// ([`GuiCollections::selected_names`]), so they cannot disagree.
    pub collections: GuiCollections,
    /// Whether one of the selected groups' album lists has not landed yet.
    pub loading: bool,
    /// The failure line naming the selected groups whose album list failed.
    pub failure: Option<String>,
    /// The union of the selected groups' artists in artist sort-name order.
    pub artists: Vec<GuiMusicArtistFrame>,
}

/// The Music destination's detail part (design D8): the Hero of the picked
/// album, its square artwork, and the id of the playing track.
#[derive(Debug, Clone, PartialEq, Eq)]
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
            collections: projection.collections.clone(),
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

/// One artist row of the Music list (design D7).
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArtFrame,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKindFrame,
}

/// An artist's stable identity, mirroring `mbv_ui_model::ArtistKey`: the
/// Service-resolved identity, or the fallback name when the Service resolves
/// none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiArtistKey {
    Service(String),
    Fallback(String),
}

/// One album row of the Music list, and the Hero's picked album.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// A track count with its total time, mirroring `crate::gui_core::GuiMusicTrackTotal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiMusicTrackTotalFrame {
    pub count: usize,
    pub duration: String,
}

/// The Hero's subject (gui-music-artist-page design D1), mirroring
/// `crate::gui_core::GuiMusicHeroSubject`: the picked album, or the picked
/// artist's page while the union still holds the artist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiMusicHeroSubjectFrame {
    /// The picked album, with its tracklist when the cache holds it.
    Album(GuiMusicAlbumFrame),
    /// The picked artist's page.
    Artist(GuiMusicArtistPageFrame),
}

/// The artist page the Hero shows while an artist is picked
/// (gui-music-artist-page design D1), mirroring
/// `crate::gui_core::GuiMusicArtistPage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiMusicArtistPageFrame {
    /// The artist's stable identity, the row the page pick names.
    pub artist_key: GuiArtistKey,
    /// The union's albums of the artist, in the union's order, with their
    /// tracklists when the cache holds them.
    pub albums: Vec<GuiMusicAlbumFrame>,
    /// The first and last release year of the albums that name one, in the
    /// union's album order; `None` when no album names a year.
    pub years: Option<(String, String)>,
}

/// The Hero's facts for the picked subject (design D5; gui-music-artist-page
/// design D1): the artist header, and the subject — the picked album's block
/// and tracklist, or the artist page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiMusicHeroFrame {
    /// The subject's artist.
    pub artist: String,
    /// The selected collection names in selector order.
    pub collections: Vec<String>,
    /// The artist's album count in the union.
    pub album_count: usize,
    /// The artist's track count and total time, present once every one of the
    /// artist's union albums holds a cached tracklist.
    pub artist_tracks: Option<GuiMusicTrackTotalFrame>,
    /// The picked subject: the artist page while the union holds the page's
    /// artist, else the picked album (gui-music-artist-page design D1).
    pub subject: GuiMusicHeroSubjectFrame,
    /// The picked item's menu entries (design D8): the shared builder's entries
    /// for the album, which More actions paints. Empty when the item offers no
    /// menu — the artist page has no More actions.
    pub menu: Vec<GuiMusicMenuEntryFrame>,
}

impl GuiMusicHeroFrame {
    /// The subject's album; `None` while the subject is an artist page.
    #[must_use]
    pub fn album(&self) -> Option<&GuiMusicAlbumFrame> {
        match &self.subject {
            GuiMusicHeroSubjectFrame::Album(album) => Some(album),
            GuiMusicHeroSubjectFrame::Artist(_) => None,
        }
    }

    /// The picked row's name (gui-music-artist-page design D6): the album's
    /// name for an album subject, the artist's name for an artist page, so a
    /// reader never matches on the subject.
    #[must_use]
    pub fn picked_name(&self) -> String {
        match &self.subject {
            GuiMusicHeroSubjectFrame::Album(album) => album.name.clone(),
            GuiMusicHeroSubjectFrame::Artist(_) => self.artist.clone(),
        }
    }

    /// The picked row's id in the encodings `window/library/list.rs` paints
    /// (gui-music-artist-page design D6): the album row's id for an album
    /// subject, the artist row's id for an artist page, so a reader never
    /// matches on the subject.
    #[must_use]
    pub fn picked_row_id(&self) -> String {
        match &self.subject {
            GuiMusicHeroSubjectFrame::Album(album) => gui_album_row_id(&album.album_id),
            GuiMusicHeroSubjectFrame::Artist(page) => gui_artist_row_id(&page.artist_key),
        }
    }
}

/// An artist row's id in the window list's encoding: the artist's stable
/// identity, prefixed so an artist and an album of the same Service id cannot
/// collide. The list's rows and the Hero's picked row share this one encoding,
/// so a pick always names the row the list paints.
#[must_use]
pub fn gui_artist_row_id(artist_key: &GuiArtistKey) -> String {
    let (kind, id) = match artist_key {
        GuiArtistKey::Service(id) => ("service", id),
        GuiArtistKey::Fallback(id) => ("fallback", id),
    };
    format!("artist:{kind}:{id}")
}

/// An album row's id in the window list's encoding.
#[must_use]
pub fn gui_album_row_id(album_id: &str) -> String {
    format!("album:{album_id}")
}

impl From<&GuiMusicArtist> for GuiMusicArtistFrame {
    fn from(artist: &GuiMusicArtist) -> Self {
        Self {
            artist_key: GuiArtistKey::from(&artist.artist_key),
            name: artist.name.clone(),
            sort_letter: artist.sort_letter,
            albums: artist.albums.iter().map(GuiMusicAlbumFrame::from).collect(),
            art: GuiRowArtFrame::from(&artist.art),
            media: GuiRowMediaKindFrame::from(artist.media),
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
    fn from(track: &crate::gui_core::GuiMusicTrack) -> Self {
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
            subject: GuiMusicHeroSubjectFrame::from(&hero.subject),
            menu: Vec::new(),
        }
    }
}

impl From<&GuiMusicHeroSubject> for GuiMusicHeroSubjectFrame {
    fn from(subject: &GuiMusicHeroSubject) -> Self {
        match subject {
            GuiMusicHeroSubject::Album(album) => Self::Album(GuiMusicAlbumFrame::from(album)),
            GuiMusicHeroSubject::Artist(page) => Self::Artist(GuiMusicArtistPageFrame::from(page)),
        }
    }
}

impl From<&GuiMusicArtistPage> for GuiMusicArtistPageFrame {
    fn from(page: &GuiMusicArtistPage) -> Self {
        Self {
            artist_key: GuiArtistKey::from(&page.artist_key),
            albums: page.albums.iter().map(GuiMusicAlbumFrame::from).collect(),
            years: page.years.clone(),
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

impl From<GuiArtistKey> for ArtistKey {
    fn from(key: GuiArtistKey) -> Self {
        match key {
            GuiArtistKey::Service(id) => Self::Service(id),
            GuiArtistKey::Fallback(id) => Self::Fallback(id),
        }
    }
}

#[cfg(test)]
mod tests;
