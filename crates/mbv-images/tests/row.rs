//! Design D6: the Emby `Row` request selects the one image kind the item
//! declares a tag for, and keys the request by that tag so changed art loads
//! again; an item that declares no tag requests the chain untagged.

use mbv_emby_model::EmbyImageTags;
use mbv_images::row::emby_row_request_for;
use mbv_images::{ImageRendition, RowEmbyImage, RowRendition};

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
