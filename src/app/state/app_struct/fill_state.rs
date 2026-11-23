//! The browse-cache fill lifecycle `App` carries (the music level fills and
//! the TV Upcoming snapshot fetch) and the bounded-cache insertion helper,
//! split out of `app_struct.rs` along their own seam.

use indexmap::IndexMap;

/// Lifecycle of the one background album-artist request per music level.
/// `Failed` levels are
/// not terminal across candidates: the next candidate creation for the level
/// retries, and unresolved albums settle via `SETTLE_WINDOW` regardless.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum LevelFillState {
    /// A level fill is in flight. Set by `spawn_level_artist_fetch`.
    /// `orphan_risk` is true for a warm-up fill that lacked the level's album
    /// paths, so a later browse can perform one path-aware upgrade.
    Loading { orphan_risk: bool },
    /// The level's albums were bulk-filled into `album_artist_cache`.
    /// Warm-up fills carry orphan risk because they had no album paths for
    /// attributing nested-disc track buckets.
    Filled { orphan_risk: bool },
    /// The level fill failed (HTTP error or no tracks); albums resolve via
    /// the settle/fallback path.
    Failed,
}

/// Entry caps for the two largest browse caches (`album_tracks_cache`,
/// `series_detail_cache`, issue #917). Neither cache had a bound or an
/// eviction pass, so a long browse session kept every fetched payload
/// reachable for the life of the process. Each insert of a fresh id evicts
/// the oldest entry once the cap is reached; revisiting an evicted entry
/// refetches it.
pub(in crate::app) const MAX_ALBUM_TRACKS_CACHE: usize = 128;
pub(in crate::app) const MAX_SERIES_DETAIL_CACHE: usize = 128;

/// Inserts `value` as the newest entry of `map`, evicting the oldest entry
/// past `cap`. The `shift_remove` gives a re-fetched key a fresh eviction age.
pub(in crate::app) fn insert_capped<V>(
    map: &mut IndexMap<String, V>,
    cap: usize,
    key: String,
    value: V,
) {
    map.shift_remove(&key);
    map.insert(key, value);
    if map.len() > cap {
        map.shift_remove_index(0);
    }
}

/// The single "should a level fill start?" decision both fill entry points
/// consume — `start_or_supersede_music_grouping`'s dedupe and
/// `spawn_level_artist_fetch`'s guard — so the two interpretations of a
/// `LevelFillState` can never diverge again.
pub(in crate::app) enum LevelFillAction {
    /// A fill is already in flight or has completed for this level.
    NoWork,
    /// No fill has run for this level, or the last one failed.
    Request,
}

impl LevelFillState {
    /// Unified interpretation of a level-fill state:
    /// `Loading`/`Filled` levels do no work; a fresh or `Failed` level
    /// (re)starts the fill.
    pub(in crate::app) fn action_for(state: Option<&LevelFillState>) -> LevelFillAction {
        match state {
            Some(LevelFillState::Loading { .. } | LevelFillState::Filled { .. }) => {
                LevelFillAction::NoWork
            }
            Some(LevelFillState::Failed) | None => LevelFillAction::Request,
        }
    }
}

/// The one fetch-lifecycle state of a TV library's eager Upcoming snapshot
/// (mirrors `LevelFillState`'s Loading/Failed shape). `InFlight` spans the
/// fetch's spaced retries; `Failed` is sticky until a new request or a
/// success replaces it, so the panel can name the failure and a reselect
/// can retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum TvUpcomingFetchState {
    InFlight,
    Failed,
}

impl TvUpcomingFetchState {
    /// Whether a fetch is under way for `state`: only `InFlight` blocks a
    /// new request; `Failed` and an absent entry both allow one.
    #[must_use]
    pub(in crate::app) fn blocks_request(state: Option<&Self>) -> bool {
        matches!(state, Some(Self::InFlight))
    }
}
