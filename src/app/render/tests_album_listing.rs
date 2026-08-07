use super::test_helpers::*;
use super::*;
use crate::app::LibSearch;

#[test]
fn searched_album_listing_does_not_duplicate_artist_row_in_plain_framing() {
    let mut app = make_music_group_app();
    let items = app.libs[0].nav_stack.last().unwrap().items.clone();
    app.libs[0].search = Some(LibSearch {
        query: "First Album".into(),
        items,
        results: vec![0],
        cursor: 0,
        scroll: 0,
        loading: false,
    });

    let out = render_library_to_string(&mut app, &mut LayoutMain::default());

    assert_eq!(
        out.lines().filter(|line| line.trim() == "Alpha").count(),
        1,
        "search-result album framing must not duplicate the artist name:\n{out}"
    );
}
