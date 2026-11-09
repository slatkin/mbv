//! The GUI Library's Music projection (add-gui-library-panel design D5): the
//! group pills, the union of the selected groups' artists by `ArtistKey` in
//! artist sort-name order, their albums with the cached tracklists, and the
//! Hero's facts for the picked album. The projection reads `App` data only, so
//! the TUI's own projections and paint do not change.

use std::collections::{HashMap, HashSet};

use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_ui_model::music_grouping::ArtistKey;
use mbv_ui_model::sort_filter::strip_article;
use mbv_ui_model::ui_util::fmt_duration_short;

use super::GuiGroupAlbums;
use crate::app::App;

/// One group pill of the Music destination (design D5, D6): a Grouped Music
/// library's collections are its groups, so every Music pill is a collection
/// pill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiMusicPill {
    /// The group's stable Service id: the set-collections command and the
    /// collection store key it.
    pub group_id: String,
    /// The group's display name, the pill's label.
    pub name: String,
    /// Whether the core's selection holds the group.
    pub selected: bool,
}

/// One artist row of the Music list (design D5, D7): the union's artist with
/// its albums, in artist sort-name order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiMusicArtist {
    /// The stable artist identity the union groups by (design D5). It never
    /// comes from a display position.
    pub artist_key: ArtistKey,
    /// The artist's display name, the row's label.
    pub name: String,
    /// The letter heading and rail letter above this row (design D7): the
    /// first letter of the article-stripped sort name, or `#`.
    pub sort_letter: char,
    /// The artist's albums in the union's order.
    pub albums: Vec<GuiMusicAlbum>,
}

/// One album row of the Music list, and the Hero's picked album (design D5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiMusicAlbum {
    /// The album's stable Service id: a command names it, and the track
    /// fetch and the artwork are keyed by it.
    pub album_id: String,
    /// The album's display name (the folder-name parse when Emby names the
    /// item by its release folder).
    pub name: String,
    /// The album's release year, empty when the Service holds none.
    pub year: String,
    /// The cached tracklist in the cache's disc and track order; `None`
    /// until the album node's open lands `App::fetch_album_tracks`.
    pub tracks: Option<Vec<GuiMusicTrack>>,
    /// The album's track count and total time, present exactly when `tracks`
    /// is (the tracklist header's facts).
    pub total: Option<GuiMusicTrackTotal>,
}

/// One tracklist row of the album block (design D5, D7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiMusicTrack {
    /// The track's stable Service id, the identity a Play next, an Add to
    /// queue, or a pick sends.
    pub track_id: String,
    /// The track's display name.
    pub name: String,
    /// The track number the row paints: the Service's index number, or the
    /// track's position when it holds none.
    pub number: i64,
    /// The track's runtime, `M:SS` or `H:MM:SS`; empty when the Service holds
    /// none.
    pub duration: String,
}

/// A track count with its total time, as the Hero's artist header and album
/// block paint them (design D5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiMusicTrackTotal {
    /// How many tracks the count covers.
    pub count: usize,
    /// Their summed runtime, `M:SS` or `H:MM:SS`.
    pub duration: String,
}

/// The Hero's facts for the picked album (design D5): the artist header, the
/// picked album block, and its tracklist. The artwork is not here: the frame
/// carries it from the image cache (row 3.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiMusicHero {
    /// The picked album's artist.
    pub artist: String,
    /// The selected collection names in pill order, the header's
    /// `ARTIST · <collections>` line (design D6).
    pub collections: Vec<String>,
    /// The artist's album count in the union.
    pub album_count: usize,
    /// The artist's track count and total time, present once every one of the
    /// artist's union albums holds a cached tracklist (design D5).
    pub artist_tracks: Option<GuiMusicTrackTotal>,
    /// The picked album, with its tracklist when the cache holds it.
    pub album: GuiMusicAlbum,
}

/// The Music projection the GUI Library panel paints (design D5): the group
/// pills with their selected state, the artist union, and the Hero's facts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiMusicProjection {
    /// The group pills in the group level's order (design D5, D6).
    pub pills: Vec<GuiMusicPill>,
    /// The selected collection names in pill order (design D6): the one
    /// derivation of the names the frame carries, so the list header, the
    /// Status bar location, and the Hero header cannot disagree.
    pub collection_names: Vec<String>,
    /// Whether one of the selected groups' album lists has not landed yet.
    pub loading: bool,
    /// The failure line naming every selected group whose album list failed
    /// to load; the groups that loaded keep their albums (design D5). The
    /// store's failure carries no Service message, so the line is worded here.
    pub failure: Option<String>,
    /// The union of the selected groups' artists, one row per `ArtistKey`, in
    /// artist sort-name order (design D5).
    pub artists: Vec<GuiMusicArtist>,
    /// The Hero's facts for the picked album; `None` until the window content
    /// picks one of the union's albums (design D2, D5).
    pub hero: Option<GuiMusicHero>,
}

/// The de-duplicated union of the selected groups' albums, with each album's
/// `(artist, year, name)` facts and stable `ArtistKey`, in pill order (design
/// D5), plus the groups that failed or are still loading.
#[derive(Default)]
struct GuiMusicUnion<'a> {
    albums: Vec<&'a EmbyItem>,
    facts: Vec<(String, String, String)>,
    keys: Vec<ArtistKey>,
    failed: Vec<String>,
    loading: bool,
}

impl App {
    /// The active destination's Grouped Music library index (design D2, D5):
    /// the destination whose Music content the GUI Library panel paints.
    /// `None` for every other destination, which presents no Music section.
    #[must_use]
    pub fn gui_music_destination(&self) -> Option<usize> {
        let mbv_ui_model::tab_selection::TabSelection::EmbyLibrary(lib_idx) = self.tab else {
            return None;
        };
        (lib_idx < self.libs.len() && self.is_grouped_music_library(lib_idx)).then_some(lib_idx)
    }

    /// The Music projection the GUI Library panel paints (design D5): the
    /// Grouped Music library's pills, the union of the selected groups'
    /// artists with their albums and cached tracklists, and the Hero's facts
    /// for the picked album.
    #[must_use]
    pub fn gui_music_projection(&self, lib_idx: usize) -> GuiMusicProjection {
        let pills = self.gui_music_pills(lib_idx);
        let collection_names = selected_collection_names(&pills);
        let union = self.gui_music_union(lib_idx, &pills);
        let artists = self.gui_music_artists(&union);
        let hero = self.gui_music_hero(&artists, &collection_names);
        GuiMusicProjection {
            pills,
            collection_names,
            loading: union.loading,
            failure: failure_line(&union.failed),
            artists,
            hero,
        }
    }

    /// The Grouped Music library's group pills in the group level's order
    /// (design D5): the group level is the library's own level in
    /// `nav_stack`, and the pills paint the core's stored selection.
    fn gui_music_pills(&self, lib_idx: usize) -> Vec<GuiMusicPill> {
        if lib_idx >= self.libs.len() || !self.is_grouped_music_library(lib_idx) {
            return Vec::new();
        }
        let lib = &self.libs[lib_idx];
        let Some(group_level) = lib
            .nav_stack
            .iter()
            .find(|level| level.parent_id == lib.library.id)
        else {
            return Vec::new();
        };
        let selected = self.gui_selected_group(lib_idx);
        group_level
            .items
            .iter()
            .map(|group| GuiMusicPill {
                group_id: group.id.clone(),
                name: group.name.clone(),
                selected: selected.as_deref() == Some(group.id.as_str()),
            })
            .collect()
    }

    /// The group the Music destination shows (design D6): the stored choice
    /// while the library still holds it, else the first group in pill order.
    /// One group is always selected, as in the TUI. `None` while the group
    /// list has not landed or the library holds no group.
    #[must_use]
    pub(in crate::app::gui_library) fn gui_selected_group(&self, lib_idx: usize) -> Option<String> {
        let groups = &self.gui_music_group_level(lib_idx)?.items;
        match self.gui_library_groups.get(&lib_idx) {
            Some(stored) if groups.iter().any(|group| group.id == *stored) => Some(stored.clone()),
            _ => groups.first().map(|group| group.id.clone()),
        }
    }

    /// The albums of the selected group (design D5): its whole album list,
    /// with the `(artist, year, name)` facts and stable `ArtistKey` that
    /// `mbv_render::group_album_plan` derives from `album_artist_cache`. One
    /// group is always selected (design D6), so the union is that one list; a
    /// failed fetch names the group in the failure line.
    fn gui_music_union(&self, lib_idx: usize, pills: &[GuiMusicPill]) -> GuiMusicUnion<'_> {
        let mut union = GuiMusicUnion::default();
        let mut seen: HashSet<&str> = HashSet::new();
        for pill in pills.iter().filter(|pill| pill.selected) {
            let Some(state) = self.gui_music_albums.get(&(lib_idx, pill.group_id.clone())) else {
                union.loading = true;
                continue;
            };
            match state {
                GuiGroupAlbums::Loaded(albums) => {
                    self.extend_gui_music_union(&mut union, &mut seen, albums);
                }
                GuiGroupAlbums::Failed => union.failed.push(pill.name.clone()),
                GuiGroupAlbums::Loading => union.loading = true,
            }
        }
        union
    }

    /// Append one loaded group's albums to the union, de-duplicated by album
    /// id, with the facts and stable identity `mbv_render::group_album_plan`
    /// derives from `album_artist_cache` (design D5).
    fn extend_gui_music_union<'a>(
        &'a self,
        union: &mut GuiMusicUnion<'a>,
        seen: &mut HashSet<&'a str>,
        albums: &'a [EmbyItem],
    ) {
        let (facts, keys) = mbv_render::group_album_plan(&self.album_artist_cache, albums, None);
        for ((facts, key), album) in facts.into_iter().zip(keys).zip(albums) {
            if seen.insert(album.id.as_str()) {
                union.albums.push(album);
                union.facts.push(facts);
                union.keys.push(key);
            }
        }
    }

    /// The union's artist rows in artist sort-name order (design D5): the
    /// `mbv_render::sorted_group_album_order` order of the union's facts,
    /// grouped into one row per `ArtistKey` at its first occurrence.
    fn gui_music_artists(&self, union: &GuiMusicUnion<'_>) -> Vec<GuiMusicArtist> {
        let order = mbv_render::sorted_group_album_order(&union.facts);
        let mut artists: Vec<GuiMusicArtist> = Vec::new();
        let mut positions: HashMap<&ArtistKey, usize> = HashMap::new();
        for &index in &order {
            let (Some(facts), Some(album), Some(key)) = (
                union.facts.get(index),
                union.albums.get(index),
                union.keys.get(index),
            ) else {
                continue;
            };
            let (artist, year, name) = facts;
            let position = if let Some(position) = positions.get(key).copied() {
                position
            } else {
                artists.push(GuiMusicArtist {
                    artist_key: key.clone(),
                    name: artist.clone(),
                    sort_letter: sort_letter(artist),
                    albums: Vec::new(),
                });
                let position = artists.len() - 1;
                positions.insert(key, position);
                position
            };
            artists[position]
                .albums
                .push(self.gui_music_album(album, year, name));
        }
        artists
    }

    /// One album row of the union, with its cached tracklist when
    /// `album_tracks_cache` holds it (design D5). An opened album's fetch
    /// fills that cache, and its drain sorts it into disc and track order.
    fn gui_music_album(&self, album: &EmbyItem, year: &str, name: &str) -> GuiMusicAlbum {
        let tracks = self.album_tracks_cache.get(&album.id);
        GuiMusicAlbum {
            album_id: album.id.clone(),
            name: name.to_string(),
            year: year.to_string(),
            tracks: tracks.map(|tracks| gui_music_tracks(tracks)),
            total: tracks.map(|tracks| track_total(tracks)),
        }
    }

    /// The Hero's facts for the picked album (design D5): the artist header
    /// from the album's union row, and the album block with its tracklist.
    /// The picked album is the one the window content showed the detail of;
    /// an album outside the selected collections' union has no Hero, because
    /// its artist header's facts are the union's.
    fn gui_music_hero(
        &self,
        artists: &[GuiMusicArtist],
        collection_names: &[String],
    ) -> Option<GuiMusicHero> {
        let picked = self.gui_library_detail.as_ref()?.id.as_str();
        let (artist, album) = artists.iter().find_map(|artist| {
            artist
                .albums
                .iter()
                .find(|album| album.album_id == picked)
                .map(|album| (artist, album))
        })?;
        Some(GuiMusicHero {
            artist: artist.name.clone(),
            collections: collection_names.to_vec(),
            album_count: artist.albums.len(),
            artist_tracks: self.gui_artist_tracks(artist),
            album: album.clone(),
        })
    }

    /// The picked artist's track count and total time, once every one of its
    /// union albums holds a cached tracklist (design D5).
    fn gui_artist_tracks(&self, artist: &GuiMusicArtist) -> Option<GuiMusicTrackTotal> {
        let mut count = 0;
        let mut ticks = 0;
        for album in &artist.albums {
            let tracks = self.album_tracks_cache.get(&album.album_id)?;
            count += tracks.len();
            ticks += tracks.iter().map(|track| track.runtime_ticks).sum::<i64>();
        }
        Some(GuiMusicTrackTotal {
            count,
            duration: fmt_duration_short(ticks / TICKS_PER_SECOND),
        })
    }
}

/// The selected pills' names in pill order (design D6): the one derivation
/// of the names the projection carries, so every painter that names the
/// selected collections reads the same fact.
fn selected_collection_names(pills: &[GuiMusicPill]) -> Vec<String> {
    pills
        .iter()
        .filter(|pill| pill.selected)
        .map(|pill| pill.name.clone())
        .collect()
}

/// The album's tracklist rows (design D5, D7).
fn gui_music_tracks(tracks: &[EmbyItem]) -> Vec<GuiMusicTrack> {
    tracks
        .iter()
        .enumerate()
        .map(|(position, track)| GuiMusicTrack {
            track_id: track.id.clone(),
            name: track.name.clone(),
            number: track_number(track, position),
            duration: if track.runtime_ticks > 0 {
                fmt_duration_short(track.runtime_ticks / TICKS_PER_SECOND)
            } else {
                String::new()
            },
        })
        .collect()
}

/// The track number a row paints: the Service's index number, or the track's
/// position when the Service holds none (the TUI's row rule).
fn track_number(track: &EmbyItem, position: usize) -> i64 {
    if track.index_number > 0 {
        track.index_number
    } else {
        i64::try_from(position)
            .unwrap_or(i64::MAX)
            .saturating_add(1)
    }
}

/// An album's track count and total time.
fn track_total(tracks: &[EmbyItem]) -> GuiMusicTrackTotal {
    let ticks: i64 = tracks.iter().map(|track| track.runtime_ticks).sum();
    GuiMusicTrackTotal {
        count: tracks.len(),
        duration: fmt_duration_short(ticks / TICKS_PER_SECOND),
    }
}

/// The failure line for the selected groups whose album list failed to load
/// (design D5). The store's `Failed` carries no message, so the groups are
/// named and the line is worded here.
fn failure_line(failed: &[String]) -> Option<String> {
    if failed.is_empty() {
        None
    } else {
        Some(format!("Couldn't load albums: {}", failed.join(", ")))
    }
}

/// An artist row's letter heading and rail letter (design D7): the first
/// letter of the article-stripped sort name that the artist order uses,
/// uppercased, or `#` for a name that starts with a digit or a non-ASCII
/// letter.
fn sort_letter(artist: &str) -> char {
    match strip_article(artist).chars().next() {
        Some(first) if first.is_ascii_alphabetic() => first.to_ascii_uppercase(),
        Some(_) | None => '#',
    }
}
