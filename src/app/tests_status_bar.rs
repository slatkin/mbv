use super::*;
use crate::app::tests::*;

#[test]
fn home_refresh_preserves_cursor_by_lib_id() {
    // Simulate what init_home does: old_cursors keyed by lib_id.
    let old_latest: Vec<(String, String, Vec<MediaItem>, usize)> = vec![
        (
            "Latest Movies".into(),
            "lib-movies".into(),
            make_items(10),
            7,
        ),
        ("Latest TV".into(), "lib-tv".into(), make_items(5), 3),
    ];
    let old_cursors: std::collections::HashMap<String, usize> = old_latest
        .iter()
        .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
        .collect();

    // New fetch returns same libs but with fresh items.
    let new_items_movies = make_items(12);
    let new_items_tv = make_items(4);

    let cursor_movies = old_cursors
        .get("lib-movies")
        .copied()
        .unwrap_or(0)
        .min(new_items_movies.len().saturating_sub(1));
    let cursor_tv = old_cursors
        .get("lib-tv")
        .copied()
        .unwrap_or(0)
        .min(new_items_tv.len().saturating_sub(1));

    assert_eq!(cursor_movies, 7, "cursor preserved when within bounds");
    assert_eq!(cursor_tv, 3, "cursor preserved when within bounds");
}

#[test]
fn home_refresh_clamps_cursor_when_new_list_is_shorter() {
    let old_latest: Vec<(String, String, Vec<MediaItem>, usize)> = vec![(
        "Latest Movies".into(),
        "lib-movies".into(),
        make_items(10),
        9,
    )];
    let old_cursors: std::collections::HashMap<String, usize> = old_latest
        .iter()
        .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
        .collect();

    let new_items = make_items(5); // shorter than before
    let cursor = old_cursors
        .get("lib-movies")
        .copied()
        .unwrap_or(0)
        .min(new_items.len().saturating_sub(1));

    assert_eq!(cursor, 4, "cursor clamped to new last index");
}

#[test]
fn home_refresh_cursor_defaults_zero_for_new_library() {
    let old_cursors: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let new_items = make_items(8);
    let cursor = old_cursors
        .get("brand-new-lib")
        .copied()
        .unwrap_or(0)
        .min(new_items.len().saturating_sub(1));
    assert_eq!(cursor, 0);
}

#[test]
fn home_section_clamped_after_refresh_removes_sections() {
    let mut app = make_app_stub();
    app.home.latest = sections(4); // 5 total
    app.home.section = 4;

    // Simulate refresh that returns fewer sections.
    app.home.latest = sections(1); // now only 2 total
    let n = 1 + app.home.latest.len();
    if app.home.section >= n {
        app.home.section = n.saturating_sub(1);
    }
    assert_eq!(app.home.section, 1);
}

#[test]
fn home_section_cycle_includes_continue_watching_in_both_directions() {
    let mut app = make_app_stub();
    app.home.continue_items = make_items(1);
    app.home.latest = sections(2);

    app.home.section = 2;
    app.power_home_move_section(1);
    assert_eq!(app.home.section, 0);

    app.power_home_move_section(-1);
    assert_eq!(app.home.section, 2);
}
