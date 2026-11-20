//! Design D6: the Emby `Row` request selects the one image kind the item
//! declares a tag for, and keys the request by that tag so changed art loads
//! again; an item that declares no tag requests the chain untagged.

use mbv_emby_model::EmbyImageTags;
use mbv_images::row::{emby_row_primary_request, emby_row_request_for};
use mbv_images::{ImageRendition, RowEmbyImage, RowRendition, abs_row_cache_key};

#[test]
fn a_declared_tag_selects_its_kind_and_keys_the_request_by_the_tag() {
    let tags = EmbyImageTags {
        thumb: "thumb-1".into(),
        ..Default::default()
    };

    let request = emby_row_request_for("item-1", "", &tags, false);

    assert_eq!(request.cache_key, "row:item-1:Thumb:thumb-1");
    assert_eq!(request.types, ["Thumb"]);
    assert_eq!(
        request.rendition,
        ImageRendition::Row(RowRendition {
            square: false,
            emby: Some(RowEmbyImage {
                kind: "Thumb".into(),
                tag: "thumb-1".into(),
            }),
        })
    );
}

#[test]
fn an_item_with_no_declared_tag_requests_the_chain_untagged() {
    let request = emby_row_request_for("item-1", "", &EmbyImageTags::default(), true);

    assert_eq!(request.cache_key, "row:item-1:untagged");
    assert_eq!(request.types, ["Thumb", "Backdrop", "Logo", "Primary"]);
    assert_eq!(
        request.rendition,
        ImageRendition::Row(RowRendition {
            square: true,
            emby: None,
        })
    );
}

#[test]
fn a_series_addressed_row_takes_the_series_tag_not_the_items_own() {
    let tags = EmbyImageTags {
        thumb: "own-thumb".into(),
        series_thumb: "series-thumb".into(),
        ..Default::default()
    };

    let request = emby_row_request_for("episode-1", "series-1", &tags, false);

    assert_eq!(request.cache_key, "row:episode-1:Thumb:series-thumb");
    assert_eq!(request.series_id, "series-1");
}

#[test]
fn an_abs_row_key_changes_with_the_library_items_update_time() {
    let first = abs_row_cache_key("https://abs.example", "book-1", 1_700_000_000_000);
    let unchanged = abs_row_cache_key("https://abs.example", "book-1", 1_700_000_000_000);
    let changed_art = abs_row_cache_key("https://abs.example", "book-1", 1_700_000_000_001);

    assert_eq!(first, unchanged);
    assert_ne!(first, changed_art);
    assert!(
        first.starts_with("row:abs:"),
        "a row key lives in the row namespace"
    );
    assert!(
        first.ends_with(":book-1:1700000000000"),
        "the key names the library item and its update time"
    );
}

/// A Music artist row wants the artist's picture: the request asks for `Primary`
/// alone, not the chain whose `Logo` or `Backdrop` would win over it.
#[test]
fn a_primary_request_asks_for_the_primary_kind_alone() {
    let request = emby_row_primary_request("artist-1");

    assert_eq!(request.types, ["Primary"]);
}
