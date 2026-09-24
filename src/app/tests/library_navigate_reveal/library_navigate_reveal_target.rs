use super::*;
use crate::app::dispatch::library::browse::{resolve_reveal_target, RevealTarget};
use rstest::rstest;

fn ancestor(id: &str, item_type: &str) -> EmbyItem {
    let mut item = make_item(id, item_type);
    item.id = id.into();
    item
}

/// D1 reveal table (task 1.1): item_type + the item's own back-references +
/// the ancestor chain (nearest→root) → the single reveal target. An empty
/// ancestors row means the worker never needed the round trip.
#[rstest]
#[case::episode_series_id_decides_without_the_round_trip(
    "Episode", "ser1", "", &[],
    Ok(RevealTarget::Series("ser1".into()))
)]
#[case::episode_falls_back_to_the_series_ancestor(
    "Episode", "", "", &[("se1", "Season"), ("ser1", "Series"), ("folder1", "Folder"), ("root", "AggregateFolder")],
    Ok(RevealTarget::Series("ser1".into()))
)]
#[case::episode_without_a_series_ancestor_is_a_resolve_failure(
    "Episode", "", "", &[("folder1", "Folder"), ("root", "AggregateFolder")],
    Err("Could not resolve the item's series")
)]
#[case::season_resolves_like_an_episode(
    "Season", "ser1", "", &[],
    Ok(RevealTarget::Series("ser1".into()))
)]
#[case::track_album_id_decides("Audio", "", "alb1", &[], Ok(RevealTarget::Album("alb1".into())))]
#[case::track_falls_back_to_the_album_ancestor(
    "Audio", "", "", &[("alb1", "MusicAlbum"), ("folder1", "Folder"), ("root", "AggregateFolder")],
    Ok(RevealTarget::Album("alb1".into()))
)]
#[case::track_without_an_album_ancestor_is_a_resolve_failure(
    "Audio", "", "", &[("folder1", "Folder"), ("root", "AggregateFolder")],
    Err("Could not resolve the item's album")
)]
#[case::album_resolves_itself("MusicAlbum", "", "", &[], Ok(RevealTarget::Album("item1".into())))]
// D1: an artist does not land. It has no single owning album, and a plain
// artist browse chain does not render on a grouped Music surface (real-tick
// render check), so the kind resolves to the pre-U2 failure regardless of
// the ancestors it has.
#[case::artist_is_a_resolve_failure(
    "MusicArtist", "", "", &[],
    Err("Could not resolve the artist's album")
)]
#[case::artist_is_a_resolve_failure_even_with_an_album_ancestor(
    "MusicArtist", "", "",
    &[("alb1", "MusicAlbum"), ("folder1", "Folder"), ("root", "AggregateFolder")],
    Err("Could not resolve the artist's album")
)]
#[case::series_resolves_itself("Series", "", "", &[], Ok(RevealTarget::Series("item1".into())))]
#[case::movie_keeps_the_chain("Movie", "", "", &[], Ok(RevealTarget::Chain))]
#[case::generic_kind_keeps_the_chain("Folder", "", "", &[], Ok(RevealTarget::Chain))]
fn reveal_table(
    #[case] item_type: &str,
    #[case] series_id: &str,
    #[case] album_id: &str,
    #[case] ancestors: &[(&str, &str)],
    #[case] expected: Result<RevealTarget, &str>,
) {
    let mut item = make_item("Item", item_type);
    item.id = "item1".into();
    item.series_id = series_id.into();
    item.album_id = album_id.into();
    let chain: Vec<EmbyItem> = ancestors.iter().map(|(id, t)| ancestor(id, t)).collect();
    let got = resolve_reveal_target(
        item_type,
        &item,
        if chain.is_empty() { None } else { Some(&chain) },
    );
    match expected {
        Ok(expected) => assert_eq!(got, Ok(expected)),
        Err(msg) => assert_eq!(got.unwrap_err(), msg),
    }
}
