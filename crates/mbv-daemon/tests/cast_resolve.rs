//! cast-owner-output task 4.1: the Owner's Cast source resolver classifies an
//! uncastable item with the item's name and the reason (design D6, scenario
//! "Item is uncastable"). Both classifications happen before any client or
//! session is touched, so the resolve context stays empty and the test is
//! hermetic.

use mbv_daemon::{CastResolveContext, prepare_cast_source};
use mbv_queue::{AudiobookshelfBookQueueItem, AudiobookshelfItem, FeedEntry, QueueItem};
use rstest::rstest;

fn book_item() -> QueueItem {
    QueueItem::Audiobookshelf(AudiobookshelfItem::Book(AudiobookshelfBookQueueItem {
        library_item_id: "book-1".into(),
        title: "A Long Book".into(),
        author: None,
        duration_ticks: Some(1000),
        position_ticks: 0,
        played: false,
        is_finished: false,
        cover_path: None,
    }))
}

fn feed_item_without_url() -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: "ep-1".into(),
        title: "An Episode".into(),
        enclosure_url: None,
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: Some("feed".into()),
        position_ticks: 0,
        played: false,
    })
}

#[rstest]
#[case::abs_book(book_item(), "A Long Book", "multi-file audiobook")]
#[case::feed_without_media_url(feed_item_without_url(), "An Episode", "no media URL to cast")]
fn an_uncastable_item_is_classified_with_its_name_and_reason(
    #[case] item: QueueItem,
    #[case] name: &str,
    #[case] reason_fragment: &str,
) {
    let uncastable = prepare_cast_source(&item, &CastResolveContext::default()).unwrap_err();

    assert_eq!(uncastable.item, name, "the classification names the item");
    assert!(
        uncastable.reason.contains(reason_fragment),
        "reason: {}",
        uncastable.reason
    );
}
