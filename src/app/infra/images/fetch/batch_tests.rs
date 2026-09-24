use super::level_artists::{bucket_tracks_by_album, level_artists_from_items, vote_album_artist};
use crate::app::tests::make_item;
use rstest::rstest;
fn track(
    parent: &str,
    path: &str,
    album_artist: Option<&str>,
    artists: &[&str],
) -> serde_json::Value {
    let mut t = serde_json::json!({ "ParentId": parent, "Path": path });
    if let Some(a) = album_artist {
        t["AlbumArtist"] = serde_json::json!(a);
    }
    t["Artists"] = serde_json::json!(artists);
    t
}

fn album(id: &str, path: &str) -> mbv_core::api::EmbyItem {
    let mut a = make_item(id, "Folder");
    a.id = id.into();
    a.path = path.into();
    a.is_folder = true;
    a
}

#[rstest]
#[case("majority_win", &["A", "B", "A", "A", "B"], Some("A"))]
#[case("album_artist_present_but_empty_skipped", &["", ""], None)]
#[case("empty_candidates_skipped", &["", "", "C", "C"], Some("C"))]
#[case("first_seen_wins_tie", &["B", "A", "A", "B"], Some("B"))]
#[case("sample_capped_at_five", &["A", "A", "A", "A", "A", "B", "B", "B"], Some("A"))]
#[case("no_candidates", &["", ""], None)]
fn vote_cases(#[case] _name: &str, #[case] album_artists: &[&str], #[case] expected: Option<&str>) {
    let tracks: Vec<serde_json::Value> = album_artists
        .iter()
        .map(|a| track("alb-1", "/m/a/1.flac", Some(a), &["Fallback"]))
        .collect();
    assert_eq!(
        vote_album_artist(&tracks).as_deref(),
        expected,
        "case {_name}"
    );
}

#[test]
fn artists0_fallback_used_when_album_artist_absent() {
    let tracks = vec![
        track("alb-1", "/m/a/1.flac", None, &["X"]),
        track("alb-1", "/m/a/2.flac", None, &["X"]),
        track("alb-1", "/m/a/3.flac", None, &["Y"]),
    ];
    assert_eq!(vote_album_artist(&tracks).as_deref(), Some("X"));
}

#[test]
fn empty_album_artist_does_not_fall_through_to_artists() {
    let tracks = vec![track("alb-1", "/m/a/1.flac", Some(""), &["X"])];
    assert_eq!(vote_album_artist(&tracks), None);
}

#[test]
fn buckets_one_to_one_by_parent_id() {
    let tracks = vec![
        track("alb-1", "/m/a/1.flac", None, &["A"]),
        track("alb-2", "/m/b/1.flac", None, &["B"]),
        track("alb-1", "/m/a/2.flac", None, &["A"]),
    ];
    let albums = vec![album("alb-1", "/m/a"), album("alb-2", "/m/b")];
    let buckets = bucket_tracks_by_album(&tracks, &albums);
    assert_eq!(buckets.len(), 2);
    let a = &buckets["alb-1"];
    assert_eq!(a.len(), 2);
    assert_eq!(a[0]["Path"], "/m/a/1.flac");
    assert_eq!(a[1]["Path"], "/m/a/2.flac");
    assert_eq!(buckets["alb-2"].len(), 1);
}

#[test]
fn multi_disc_orphan_bucket_attributed_via_path() {
    let tracks = vec![
        track("disc-1", "/m/a/Disc 1/1.flac", None, &["A"]),
        track("disc-1", "/m/a/Disc 1/2.flac", None, &["A"]),
        track("disc-2", "/m/a/Disc 2/1.flac", None, &["A"]),
    ];
    let albums = vec![album("alb-1", "/m/a")];
    let buckets = bucket_tracks_by_album(&tracks, &albums);
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets["alb-1"].len(), 3);
}

#[test]
fn fill_requested_from_page_one_covers_later_page_albums() {
    // Page-starvation regression: the fill is requested with only the
    // page-1 album in hand; bucketing by `ParentId` verbatim still
    // yields the page-2 album's artist from its tracks, so the one
    // whole-level fill covers every album in the level no matter which
    // page was listed when it was requested.
    let tracks = vec![
        track("alb-1", "/m/a/1.flac", Some("Alpha"), &["Alpha"]),
        track("alb-2", "/m/b/1.flac", Some("Beta"), &["Beta"]),
    ];
    let albums = vec![album("alb-1", "/m/a")];
    let artists = level_artists_from_items(&tracks, &albums);
    assert_eq!(
        artists,
        vec![
            ("alb-1".to_string(), "Alpha".to_string()),
            ("alb-2".to_string(), "Beta".to_string()),
        ]
    );
}

#[test]
fn unmatched_orphan_key_stays_as_inert_bucket() {
    // An orphan whose `Path` matches no in-hand album keeps its own
    // key: an inert cache row nothing looks up (a Service reset clears
    // it); the album itself resolves via the settle/fallback path.
    let tracks = vec![
        track("disc-9", "/m/other/x/1.flac", None, &["A"]),
        track("alb-1", "/m/a/1.flac", None, &["A"]),
    ];
    let albums = vec![album("alb-1", "/m/a")];
    let buckets = bucket_tracks_by_album(&tracks, &albums);
    assert_eq!(buckets["alb-1"].len(), 1);
    assert_eq!(buckets["disc-9"].len(), 1);
}

#[test]
fn nested_album_path_claims_its_orphan_tracks() {
    // A nested album Path (Deluxe edition one level deeper) must win the
    // longest-prefix attribution over its outer album.
    let tracks = vec![
        track("disc-1", "/m/a/Deluxe/1.flac", None, &["A"]),
        track("disc-9", "/m/a/1.flac", None, &["A"]),
    ];
    let albums = vec![
        album("alb-outer", "/m/a"),
        album("alb-deluxe", "/m/a/Deluxe"),
    ];
    let buckets = bucket_tracks_by_album(&tracks, &albums);
    assert_eq!(buckets["alb-deluxe"].len(), 1);
    assert_eq!(buckets["alb-deluxe"][0]["Path"], "/m/a/Deluxe/1.flac");
    assert_eq!(buckets["alb-outer"].len(), 1);
    assert_eq!(buckets["alb-outer"][0]["Path"], "/m/a/1.flac");
}

#[test]
fn level_pipeline_buckets_votes_and_orders_deterministically() {
    // Mock JSON shaped like the level request's `Items` array: two
    // 1:1 album buckets plus a multi-disc orphan bucket, exercising the
    // full parse+bucket+vote pipeline without a live server.
    let tracks = vec![
        track("alb-b", "/m/b/2.flac", Some("Beta"), &["Beta"]),
        track("disc-1", "/m/a/Disc 1/1.flac", Some("Alpha"), &["Alpha"]),
        track("disc-2", "/m/a/Disc 2/1.flac", Some("Wrong"), &["Wrong"]),
        track("disc-2", "/m/a/Disc 2/2.flac", Some("Alpha"), &["Alpha"]),
        track("alb-b", "/m/b/1.flac", Some("Alpha"), &["Beta"]),
        track("alb-c", "/m/c/1.flac", Some(""), &[""]),
    ];
    let albums = vec![album("alb-a", "/m/a"), album("alb-b", "/m/b")];
    let artists = level_artists_from_items(&tracks, &albums);
    // `alb-c`'s bucket yields no candidate and is omitted entirely;
    // `alb-a`'s orphan bucket majority-votes to Alpha despite the outlier.
    assert_eq!(
        artists,
        vec![
            ("alb-a".to_string(), "Alpha".to_string()),
            ("alb-b".to_string(), "Beta".to_string()),
        ]
    );
}

#[test]
fn level_pipeline_empty_tracks_yield_empty_artists() {
    // HTTP failure surfaces as an empty `Items` array upstream; the
    // pipeline must return no artists so the level marks `Failed`.
    let albums = vec![album("alb-a", "/m/a")];
    assert!(level_artists_from_items(&[], &albums).is_empty());
}

#[test]
fn orphan_merge_order_follows_request_order_so_tie_break_is_stable() {
    // Two disc subfolders re-attributing to one album with a tied vote:
    // the winner must be the artist first seen in request order, no
    // matter how `HashMap` enumerates the orphan bucket keys. Two track
    // lists with different orphan key names (same first-appearance
    // ordering) must yield identical merged-bucket order and winner.
    let albums = vec![album("alb-1", "/m/a")];
    for (k1, k2) in [("disc-1", "disc-2"), ("cd-b", "cd-a")] {
        let tracks = vec![
            track(k1, "/m/a/Disc 1/1.flac", Some("Zed"), &["Zed"]),
            track(k2, "/m/a/Disc 2/1.flac", Some("Abc"), &["Abc"]),
        ];
        let buckets = bucket_tracks_by_album(&tracks, &albums);
        assert_eq!(buckets["alb-1"].len(), 2, "keys {k1}/{k2}");
        assert_eq!(buckets["alb-1"][0]["Path"], "/m/a/Disc 1/1.flac");
        assert_eq!(buckets["alb-1"][1]["Path"], "/m/a/Disc 2/1.flac");
        let artists = level_artists_from_items(&tracks, &albums);
        assert_eq!(
            artists,
            vec![("alb-1".to_string(), "Zed".to_string())],
            "keys {k1}/{k2}"
        );
    }
}

#[test]
fn sibling_prefix_does_not_catch_unrelated_orphan() {
    // Component-aligned matching: `/m/ab` must not attribute to `/m/a`.
    // The unmatched orphan keeps its own (inert) key instead.
    let tracks = vec![track("disc-1", "/m/ab/1.flac", None, &["A"])];
    let albums = vec![album("alb-1", "/m/a")];
    let buckets = bucket_tracks_by_album(&tracks, &albums);
    assert!(!buckets.contains_key("alb-1"));
    assert_eq!(buckets["disc-1"].len(), 1);
}
