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
#[case::majority_win(&["A", "B", "A", "A", "B"], Some("A"))]
#[case::first_seen_wins_tie(&["B", "A", "A", "B"], Some("B"))]
fn vote_cases(#[case] album_artists: &[&str], #[case] expected: Option<&str>) {
    let tracks: Vec<serde_json::Value> = album_artists
        .iter()
        .map(|artist| track("alb-1", "/m/a/1.flac", Some(artist), &["Fallback"]))
        .collect();
    assert_eq!(vote_album_artist(&tracks).as_deref(), expected);
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
