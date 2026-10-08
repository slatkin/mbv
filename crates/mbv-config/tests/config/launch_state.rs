use mbv_config::AudiobookshelfBookBucket;

#[test]
fn tui_launch_state_audiobookshelf_book_bucket_from_index_matches_fixed_table() {
    let expected = vec![
        AudiobookshelfBookBucket::AToC,
        AudiobookshelfBookBucket::DToF,
        AudiobookshelfBookBucket::GToI,
        AudiobookshelfBookBucket::JToL,
        AudiobookshelfBookBucket::MToO,
        AudiobookshelfBookBucket::PToR,
        AudiobookshelfBookBucket::SToU,
        AudiobookshelfBookBucket::VToZ,
    ];
    let actual: Vec<_> = (0..8)
        .map(|index| AudiobookshelfBookBucket::from_bucket_index(index).unwrap())
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(AudiobookshelfBookBucket::from_bucket_index(8), None);
}
