//! The queue card's artwork ownership: the chain and cache key a Movie and an
//! Episode take. Pure functions over an `EmbyItem`, so they need no `App`.

use super::{card_cache_key, card_image_types};

/// Guards 55c37ea71 (landscape key for movies and episodes) and its fix
/// e39b5a8f4 (episode keeps its own still, Movie-only key): a Movie takes the
/// landscape chain even with only a poster tag, while an episode carrying
/// series tags keeps its own poster-first chain so its card never shows
/// series art.
#[rstest::rstest]
#[case::movie_with_primary_tag(
    "Movie",
    Some("primary-tag"),
    None,
    "item:QB",
    &["Backdrop", "Primary"]
)]
#[case::episode_with_series_tags(
    "Episode",
    None,
    Some("series-thumb-tag"),
    "item:P",
    &["Primary", "Thumb", "Backdrop", "Logo"]
)]
fn artwork_ownership_keeps_movies_landscape_and_episodes_episode_owned(
    #[case] item_type: &str,
    #[case] primary_tag: Option<&str>,
    #[case] series_thumb_tag: Option<&str>,
    #[case] expected_key: &str,
    #[case] expected_chain: &[&str],
) {
    let mut item = mbv_emby_model::test_support::make_item("Item", item_type);
    item.id = "item".into();
    if let Some(tag) = primary_tag {
        item.image_tags.primary = tag.into();
    }
    if let Some(tag) = series_thumb_tag {
        item.image_tags.series_thumb = tag.into();
    }
    assert_eq!(card_cache_key(&item), expected_key);
    assert_eq!(card_image_types(&item), expected_chain);
}
