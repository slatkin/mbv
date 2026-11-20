//! The GUI Library's Music projection (add-gui-library-panel design D5): the
//! group collection selector, the union of the selected groups' artists by
//! `ArtistKey` in artist sort-name order, their albums with the cached
//! tracklists, and the Hero's facts for the picked album. The selection is All
//! or a set of checked groups (gui-collection-dropdown design D1, D2, D11).
//! The projection reads `App` data only, so the TUI's own projections and paint
//! do not change.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_ui_model::music_grouping::ArtistKey;
use mbv_ui_model::sort_filter::strip_article;
use mbv_ui_model::ui_util::fmt_duration_short;

use super::artwork::{GuiRowArt, GuiRowMediaKind};
use super::collection::{GuiCollectionSet, GuiCollectionTarget, GuiCollections};
use super::collection_command::GuiCollectionSite;
use super::{GuiGroupAlbums, GuiMusicCollectionSite};
use crate::app::App;

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
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArt,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKind,
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
/// collection selector, the artist union, and the Hero's facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiMusicProjection {
    /// The destination's collection selector (design D6): All, every group in
    /// the group level's order with its checked flag, the Groups noun, and no
    /// date order — Music sorts by name. The bridge's collections frame rides
    /// here.
    pub collections: GuiCollections,
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
    /// Grouped Music library's pills, the union of the checked groups'
    /// artists with their albums and cached tracklists, and the Hero's facts
    /// for the picked album (gui-collection-dropdown design D1, D2, D11).
    #[must_use]
    pub fn gui_music_projection(&mut self, lib_idx: usize) -> GuiMusicProjection {
        // The one effective-set read the frame takes: the helpers read the
        // set it borrows, so the frame never recomputes the retention.
        let set = self.gui_effective_music_set(lib_idx);
        let collections = self.gui_collections(GuiCollectionTarget::Music(lib_idx));
        let selected_groups = self.gui_selected_groups(lib_idx, set.as_ref());
        let union = self.gui_music_union(lib_idx, &selected_groups);
        let artists = self.gui_music_artists(&union);
        let hero = self.gui_music_hero(lib_idx, set.as_ref(), &artists, &collections);
        let projection = GuiMusicProjection {
            collections,
            loading: union.loading,
            failure: failure_line(&union.failed),
            artists,
            hero,
        };
        self.warm_gui_music_hero_neighbours(&projection);
        projection
    }

    /// Warm the Hero artwork of the album leaves neighbouring the picked one
    /// in the painted tree order (artists in order, each artist's albums in
    /// order): the TUI's grouped-Music neighbour window over the GUI's own
    /// projection, through the shared navigation-idle gate. The pick's own art
    /// is the frame's fetch, so the picked album is never warmed here.
    fn warm_gui_music_hero_neighbours(&mut self, projection: &GuiMusicProjection) {
        let Some(picked) = projection.hero.as_ref() else {
            return;
        };
        let albums: Vec<&str> = projection
            .artists
            .iter()
            .flat_map(|artist| &artist.albums)
            .map(|album| album.album_id.as_str())
            .collect();
        let Some(picked_index) = albums
            .iter()
            .position(|album_id| *album_id == picked.album.album_id)
        else {
            return;
        };
        self.warm_gui_hero_neighbours(&albums, picked_index, |_app, album_id| {
            Some(super::artwork::music_album_source(album_id))
        });
    }

    /// The Grouped Music library's groups as the collection selector's catalog
    /// in the group level's order (design D5, D6): each group's stable Service
    /// id and its display name. The group level is the library's own level in
    /// `nav_stack`.
    pub(in crate::app::gui_library) fn gui_music_groups(
        &self,
        lib_idx: usize,
    ) -> Vec<(String, String)> {
        if lib_idx >= self.libs.len() || !self.is_grouped_music_library(lib_idx) {
            return Vec::new();
        }
        let Some(group_level) = self.gui_music_group_level(lib_idx) else {
            return Vec::new();
        };
        group_level
            .items
            .iter()
            .map(|group| (group.id.clone(), group.name.clone()))
            .collect()
    }

    /// The Grouped Music library's effective selection (design D1): the
    /// stored set, or All when none is stored. A checked set whose keys the
    /// library no longer holds falls back to All (the spec's vanish rule); a
    /// group list that has not landed admits the stored set as it is, since
    /// the list is what would name its keys stale. A set nothing drops is
    /// returned borrowed, so a per-frame reader clones nothing.
    pub(in crate::app) fn gui_effective_music_set(
        &self,
        lib_idx: usize,
    ) -> Cow<'_, GuiCollectionSet> {
        GuiMusicCollectionSite(lib_idx).effective_set(self)
    }

    /// The groups the Music destination shows, in pill order (design D1, D2):
    /// every group of the library for All, and the checked groups for a
    /// checked set. `None`'s case — the group list has not landed or the
    /// library holds no group — shows no group, so the frame path starts no
    /// fetch until the list lands. The ids are borrowed from the group
    /// level, so the union reads them without cloning.
    pub(in crate::app::gui_library) fn gui_selected_groups(
        &self,
        lib_idx: usize,
        set: &GuiCollectionSet,
    ) -> Vec<&str> {
        let Some(group_level) = self.gui_music_group_level(lib_idx) else {
            return Vec::new();
        };
        match set {
            GuiCollectionSet::All => group_level
                .items
                .iter()
                .map(|group| group.id.as_str())
                .collect(),
            GuiCollectionSet::Checked(keys) => group_level
                .items
                .iter()
                .filter(|group| keys.contains(&group.id))
                .map(|group| group.id.as_str())
                .collect(),
        }
    }

    /// The albums of the checked groups (design D1, D2): each checked
    /// group's whole album list, with the `(artist, year, name)` facts and
    /// stable `ArtistKey` that `mbv_render::group_album_plan` derives from
    /// `album_artist_cache`. All is every group's list; a failed fetch names
    /// the group in the failure line.
    fn gui_music_union(&self, lib_idx: usize, group_ids: &[&str]) -> GuiMusicUnion<'_> {
        let mut union = GuiMusicUnion::default();
        let mut seen: HashSet<&str> = HashSet::new();
        let groups = self.gui_music_albums.get(&lib_idx);
        for group_id in group_ids {
            let Some(state) = groups.and_then(|groups| groups.get(*group_id)) else {
                union.loading = true;
                continue;
            };
            match state {
                GuiGroupAlbums::Loaded(albums) => {
                    self.extend_gui_music_union(&mut union, &mut seen, albums);
                }
                GuiGroupAlbums::Failed => union.failed.push(self.gui_group_name(lib_idx, group_id)),
                GuiGroupAlbums::Loading => union.loading = true,
            }
        }
        union
    }

    /// The display name the group's failure line paints (design D5): the
    /// group's name in the group level, or the id when the level holds none.
    fn gui_group_name(&self, lib_idx: usize, group_id: &str) -> String {
        self.gui_music_group_level(lib_idx)
            .and_then(|level| {
                level
                    .items
                    .iter()
                    .find(|group| group.id == group_id)
                    .map(|group| group.name.clone())
            })
            .unwrap_or_else(|| group_id.to_string())
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
                    art: GuiRowArt::NoSource,
                    media: GuiRowMediaKind::Audio,
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

    /// The picked album id under the selection (gui-collection-dropdown
    /// design D11): one checked group's kept pick while the union holds it;
    /// All or several groups' session union pick while the union holds it;
    /// else the union's first album in list order. `None` while the union
    /// holds no album. The group's pick is read through its borrowed id, so
    /// the per-frame Hero clones nothing for the lookup.
    fn gui_picked_album_id(
        &self,
        lib_idx: usize,
        set: &GuiCollectionSet,
        artists: &[GuiMusicArtist],
    ) -> Option<String> {
        let holds = |album_id: &str| {
            artists
                .iter()
                .any(|artist| artist.albums.iter().any(|album| album.album_id == album_id))
        };
        let single_group = match set {
            GuiCollectionSet::Checked(keys) if keys.len() == 1 => keys.first().map(String::as_str),
            _ => None,
        };
        if let Some(group_id) = single_group
            && let Some(pick) = self.gui_library_group_pick(lib_idx, group_id)
            && holds(pick.as_str())
        {
            return Some(pick.clone());
        }
        if single_group.is_none()
            && let Some(pick) = self.gui_music_union_picks.get(&lib_idx)
            && holds(pick.as_str())
        {
            return Some(pick.clone());
        }
        artists
            .iter()
            .find_map(|artist| artist.albums.first())
            .map(|album| album.album_id.clone())
    }

    /// The kept pick of one checked group (design D11): the pick the launch
    /// file saves, that the group's album list keeps. The pick map is keyed
    /// by the owned id pair, so the lookup scans its few entries against the
    /// borrowed id instead of cloning the id for a key.
    fn gui_library_group_pick(&self, lib_idx: usize, group_id: &str) -> Option<&String> {
        self.gui_library_picks
            .iter()
            .find(|((idx, id), _)| *idx == lib_idx && id == group_id)
            .map(|(_, pick)| pick)
    }

    /// The Hero's facts for the picked album (design D5, D13): the artist header
    /// from the album's union row, and the album block with its tracklist. The
    /// picked album is the selected group's pick, or its first album, so the
    /// Hero is never blank while the group's albums are loaded.
    fn gui_music_hero(
        &self,
        lib_idx: usize,
        set: &GuiCollectionSet,
        artists: &[GuiMusicArtist],
        collections: &GuiCollections,
    ) -> Option<GuiMusicHero> {
        let picked = self.gui_picked_album_id(lib_idx, set, artists)?;
        let (artist, album) = artists.iter().find_map(|artist| {
            artist
                .albums
                .iter()
                .find(|album| album.album_id == picked)
                .map(|album| (artist, album))
        })?;
        Some(GuiMusicHero {
            artist: artist.name.clone(),
            collections: collections.selected_names(),
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
/// letter. Shared with the Shows rows' rail letters (design D12), which use
/// the same rule over the item's sort name.
pub(in crate::app::gui_library) fn sort_letter(name: &str) -> char {
    match strip_article(name).chars().next() {
        Some(first) if first.is_ascii_alphabetic() => first.to_ascii_uppercase(),
        Some(_) | None => '#',
    }
}
