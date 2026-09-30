use crate::app::{BrowseLevel, LibraryTab};
use mbv_ui_model::browse::BrowseResting;
use mbv_ui_model::browse::ServerRows;

#[test]
fn handle_loaded_level_replaces_the_matching_loading_level() {
    let mut app = crate::app::tests::make_app_stub();
    let mut library = mbv_emby_model::test_support::make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::complete(0),
            parent_id: "parent".into(),
            title: "Loading".into(),
            items: vec![],

            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    let level = BrowseLevel {
        rows: ServerRows::new(2),
        parent_id: "parent".into(),
        title: "Loaded".into(),
        items: crate::app::tests::make_items(2),

        resting: BrowseResting::new(1, 3),
        item_types: None,
        unplayed_only: false,
        sort_by: "DateCreated".into(),
        sort_order: "Descending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    };

    app.handle_loaded_level(0, "parent", level);

    let last = app.libs[0].nav_stack.last().unwrap();
    assert_eq!(last.title, "Loaded");
    assert_eq!(last.items.len(), 2);
    assert_eq!(last.rows.total(), 2);
    assert_eq!(last.resting().cursor(), 1);
    assert_eq!(last.sort_by, "DateCreated");
    assert_eq!(last.sort_order, "Descending");
    assert!(!last.loading);
}

#[test]
fn normalize_current_browse_level_items_sorts_episode_lists() {
    let mut app = crate::app::tests::make_app_stub();
    let mut second = mbv_emby_model::test_support::make_item("Episode 2", "Episode");
    second.index_number = 2;
    let mut first = mbv_emby_model::test_support::make_item("Episode 1", "Episode");
    first.index_number = 1;
    let mut library = mbv_emby_model::test_support::make_item("TV", "CollectionFolder");
    library.id = "lib-tv".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(2),
            parent_id: "series".into(),
            title: "Season 1".into(),
            items: vec![second, first],

            resting: BrowseResting::new(0, 0),
            item_types: Some("Episode".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    app.normalize_current_browse_level_items(0);

    let last = app.libs[0].nav_stack.last().unwrap();
    let names: Vec<&str> = last.items.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, vec!["Episode 1", "Episode 2"]);
}
