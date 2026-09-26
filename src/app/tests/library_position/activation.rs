use super::*;

#[test]
fn ensure_lib_loaded_for_uses_saved_position_loading_state_without_root_flash() {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab::new(library));
    app.library_position_state.libraries.insert(
        "lib-movies".into(),
        crate::config::LibraryPosition {
            levels: vec![
                crate::config::LibraryPositionLevel {
                    fetched_rows: None,
                    parent_id: "lib-movies".into(),
                    title: "Movies".into(),
                    focused_item_id: Some("folder-b".into()),
                    cursor_index: 1,
                    item_types: Some("Movie".into()),
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    letter_filter_index: Some(2),
                    tv_content_mode: None,
                    library_total: Some(673),
                },
                crate::config::LibraryPositionLevel {
                    fetched_rows: None,
                    parent_id: "folder-b".into(),
                    title: "Folder B".into(),
                    focused_item_id: Some("leaf-1".into()),
                    cursor_index: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    letter_filter_index: None,
                    tv_content_mode: None,
                    library_total: None,
                },
            ],
            ..Default::default()
        },
    );

    app.ensure_lib_loaded_for(0);

    assert_eq!(app.libs[0].nav_stack.len(), 1);
    let level = &app.libs[0].nav_stack[0];
    assert_eq!(level.parent_id, "lib-movies");
    assert_eq!(level.title, "Movies");
    assert!(level.loading);
    assert!(level.items.is_empty());
    assert_eq!(level.item_types.as_deref(), Some("Movie"));
    assert_eq!(app.libs[0].library_total, Some(673));
    assert_eq!(
        level.letter_filter.as_ref().map(|filter| filter.index),
        Some(2)
    );
}

// #361: `set_tab` (the Standard tab-switch entry point) is gone; the
// scope-isolation premise this test exercised ("default scope survives
// a power-scope write") no longer applies -- there is one saved
// position per library now (see `save_default_library_position_persists_focused_item`).
#[test]
fn library_tab_next_activates_saved_placeholder() {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab::new(library));
    app.library_position_state.libraries.insert(
        "lib-movies".into(),
        crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                fetched_rows: None,
                parent_id: "lib-movies".into(),
                title: "Saved".into(),
                focused_item_id: Some("id1".into()),
                cursor_index: 1,
                item_types: None,
                unplayed_only: false,
                sort_by: "DateCreated".into(),
                sort_order: "Descending".into(),
                letter_filter_index: None,
                tv_content_mode: None,
                library_total: None,
            }],
            ..Default::default()
        },
    );
    app.tab = TabSelection::Home;

    app.library_tab_next();

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(app.libs[0].nav_stack[0].title, "Saved");
    assert!(app.libs[0].nav_stack[0].loading);
}
