/// Track rows per album that feed the majority vote (design D2): the
/// per-album request fetched `Limit=5`, so the batched level request samples
/// the same first ≤5 rows per bucket.
const ALBUM_ARTIST_VOTE_SAMPLE: usize = 5;

/// One track's artist candidate: `AlbumArtist`, falling back to the track's
/// first listed artist. `None` when neither field carries a usable name —
/// an `AlbumArtist` present but empty does *not* fall through to `Artists`,
/// matching the per-album vote this extracts.
fn track_artist_candidate(track: &serde_json::Value) -> Option<String> {
    let candidate = track["AlbumArtist"]
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            track["Artists"]
                .get(0)
                .and_then(|a| a.as_str())
                .map(str::to_string)
        })?;
    (!candidate.is_empty()).then_some(candidate)
}

/// Majority vote over an album's first `ALBUM_ARTIST_VOTE_SAMPLE` track rows
/// (design D2), extracted verbatim from the per-album fetch so one
/// outlier/mistagged track can't poison the whole album's displayed artist.
/// `tracks` must already be in request order
/// (`SortBy=ParentIndexNumber,IndexNumber`). Empty candidates are skipped and
/// the first-seen artist wins ties. `None` when no sampled track yields a
/// candidate.
pub(super) fn vote_album_artist<'a>(
    tracks: impl IntoIterator<Item = &'a serde_json::Value>,
) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for track in tracks.into_iter().take(ALBUM_ARTIST_VOTE_SAMPLE) {
        let Some(candidate) = track_artist_candidate(track) else {
            continue;
        };
        match counts.iter_mut().find(|(c, _)| c == &candidate) {
            Some(entry) => entry.1 += 1,
            None => counts.push((candidate, 1)),
        }
    }
    // `max_by_key` breaks ties by keeping the *last* max; we want the
    // *first*-seen artist to win ties, since it corresponds to the
    // earliest track in the sample (closest to "read the first track").
    counts
        .into_iter()
        .enumerate()
        .max_by_key(|(i, (_, n))| (*n, std::cmp::Reverse(*i)))
        .map(|(_, (c, _))| c)
}

/// True when `path` is `root` itself or lies underneath it — a
/// component-aligned prefix, so `/a/ab/1.flac` never attributes to `/a/a`.
fn path_within(path: &str, root: &str) -> bool {
    std::path::Path::new(path).starts_with(root)
}

/// Groups a level's Audio rows by the album they belong to (design D1/D3).
/// Every track with a `ParentId` is bucketed by it verbatim: bucket keys
/// are album ids by Emby semantics, and a key need not be an album in the
/// in-hand `albums` snapshot — level listings paginate, so a whole-level
/// fill must also cover albums listed on pages after the one in hand when
/// the fill was requested (they are cached under their own id and looked
/// up when those albums' candidates run). `albums` exist here only to
/// attribute orphan tracks: a bucket keyed by an id that is not an in-hand
/// album (e.g. a multi-disc set's nested disc subfolder) is re-attributed
/// to the in-hand album whose `Path` prefixes its tracks' `Path`s, longest
/// match winning; tracks that match nothing keep their original key — an
/// inert cache row that nothing looks up and a Service reset clears.
/// Tracks with no `ParentId` go through the same `Path`-prefix map and are
/// dropped when nothing matches — the album then resolves through the
/// existing settle/fallback path. `tracks` must be in request order; each
/// bucket preserves that order. Orphan re-attribution is processed in each
/// orphan bucket's first-appearance order in `tracks` (not HashMap order),
/// so merged buckets append in request order and `vote_album_artist`'s
/// first-seen tie-break is stable across runs.
pub(super) fn bucket_tracks_by_album<'a>(
    tracks: &'a [serde_json::Value],
    albums: &[mbv_core::api::EmbyItem],
) -> std::collections::HashMap<String, Vec<&'a serde_json::Value>> {
    let album_ids: std::collections::HashSet<&str> = albums.iter().map(|a| a.id.as_str()).collect();
    let album_paths: Vec<(&str, &str)> = albums
        .iter()
        .filter(|a| !a.path.is_empty())
        .map(|a| (a.id.as_str(), a.path.as_str()))
        .collect();

    let mut buckets: std::collections::HashMap<String, Vec<&'a serde_json::Value>> =
        std::collections::HashMap::new();
    // First-appearance index of each `ParentId` bucket in the request-order
    // `tracks` slice, so orphan re-attribution below can run in request
    // order instead of `HashMap` enumeration order.
    let mut bucket_first_seen: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (index, track) in tracks.iter().enumerate() {
        let parent = track["ParentId"].as_str().unwrap_or("");
        if !parent.is_empty() {
            buckets.entry(parent.to_string()).or_default().push(track);
            bucket_first_seen.entry(parent.to_string()).or_insert(index);
            continue;
        }
        // Track without a `ParentId`: attribute by `Path` prefix against
        // the in-hand album listing (design D3), else drop.
        attribute_by_path(track, &album_paths, &mut buckets);
    }

    // Reattribute buckets keyed by an id that is not an in-hand album (a
    // nested disc subfolder): their tracks belong to the in-hand album
    // whose `Path` prefixes theirs, longest match winning so a nested
    // album folder claims its own disc tracks instead of donating them to
    // the outer album. Design D3's drop rule predates f4e9c44e: a bucket key
    // is a track `ParentId` (1:1 with albums except rare nested disc folders)
    // and `albums` is one page, so an unmatched key may be an album listed on
    // a later page. Dropping it loses whole-level coverage and makes the
    // empty-album-list warm-up resolve nothing; retain it as an inert cache
    // row nothing looks up. Buckets are processed in first-appearance order
    // in `tracks` so merged orphan tracks preserve request order and the
    // vote's first-seen tie-break does not depend on `HashMap` order.
    let mut orphan_keys: Vec<(String, usize)> = bucket_first_seen
        .into_iter()
        .filter(|(key, _)| !album_ids.contains(key.as_str()))
        .collect();
    orphan_keys.sort_by_key(|(_, index)| *index);
    for (key, _) in orphan_keys {
        let bucket = buckets.remove(&key).unwrap_or_default();
        let mut unattributed = Vec::new();
        for track in bucket {
            if !attribute_by_path(track, &album_paths, &mut buckets) {
                unattributed.push(track);
            }
        }
        if !unattributed.is_empty() {
            buckets.insert(key, unattributed);
        }
    }
    buckets
}

/// Attributes `track` to the in-hand album whose `Path` prefixes the
/// track's `Path` (design D3), longest match winning so a nested album
/// Path (e.g. a Deluxe-edition folder one level deeper) claims its own
/// disc tracks instead of donating them to the outer album. Returns
/// `false` when nothing matches. A multi-disc set surfaces as several
/// orphan tracks that all merge into their album's bucket.
fn attribute_by_path<'a>(
    track: &'a serde_json::Value,
    album_paths: &[(&str, &str)],
    buckets: &mut std::collections::HashMap<String, Vec<&'a serde_json::Value>>,
) -> bool {
    let track_path = track["Path"].as_str().unwrap_or("");
    if let Some((album_id, _)) = album_paths
        .iter()
        .filter(|(_, album_path)| path_within(track_path, album_path))
        .max_by_key(|(_, album_path)| album_path.len())
    {
        buckets
            .entry((*album_id).to_string())
            .or_default()
            .push(track);
        true
    } else {
        false
    }
}

/// Pure parse+bucket+vote pipeline behind `spawn_level_artist_fetch`
/// (design D1–D3): buckets the level's Audio rows per album (by track
/// `ParentId` verbatim) and majority-votes each bucket's artist.
/// `albums` (the listing items already in hand) drive orphan-`Path`
/// attribution only, so the result covers every album in the level —
/// including ones listed on pages after the page in hand when the fill
/// was requested. Albums with no resolvable artist are omitted — their
/// slots stay free for the settle/fallback path. Deterministic: results
/// are ordered by album id. `tracks` must be in request order.
pub(super) fn level_artists_from_items(
    tracks: &[serde_json::Value],
    albums: &[mbv_core::api::EmbyItem],
) -> Vec<(String, String)> {
    let mut artists: Vec<(String, String)> = bucket_tracks_by_album(tracks, albums)
        .into_iter()
        .filter_map(|(album_id, bucket)| {
            vote_album_artist(bucket.iter().copied()).map(|artist| (album_id, artist))
        })
        .collect();
    artists.sort_by(|a, b| a.0.cmp(&b.0));
    artists
}
