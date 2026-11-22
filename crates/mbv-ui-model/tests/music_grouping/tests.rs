use mbv_ui_model::music_grouping::sorted_group_album_order;

/// Regression for the 2026-11-18 GUI Music bug: the `McCoy Tyner` albums painted
/// 1967, 1972, 1972, 1968 because the server's `SortName` order was kept
/// inside the artist group. One year also carries a hidden tag character.
#[test]
fn albums_order_by_the_displayed_release_year() {
    let albums = [
        (
            "McCoy Tyner".to_string(),
            "1967".to_string(),
            "The Real McCoy".to_string(),
        ),
        (
            "McCoy Tyner".to_string(),
            "1972".to_string(),
            "Extensions".to_string(),
        ),
        (
            "McCoy Tyner".to_string(),
            "1972".to_string(),
            "Sahara".to_string(),
        ),
        (
            "McCoy Tyner".to_string(),
            "19\u{200b}68".to_string(),
            "Tender Moments".to_string(),
        ),
    ];

    let rows: Vec<(&str, &str)> = sorted_group_album_order(&albums)
        .into_iter()
        .map(|index| (albums[index].2.as_str(), albums[index].1.as_str()))
        .collect();

    assert_eq!(
        rows,
        [
            ("The Real McCoy", "1967"),
            ("Tender Moments", "19\u{200b}68"),
            ("Extensions", "1972"),
            ("Sahara", "1972"),
        ]
    );
}
