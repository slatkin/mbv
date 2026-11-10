//! Tests of the GUI Library's Search data path (add-gui-library-panel design
//! D12, row 12.1): a landed result fills the App's search store and projects
//! its rows, a stale result changes nothing, an activate navigates and leaves
//! no overlay request behind, an empty query closes the results, and a failed
//! result records the failure.
//!
//! The result boundary is the App's own search channel, so a test owns every
//! row it asserts and no worker thread or network runs behind it. The TUI's
//! own search drain and its component tests are untouched.

use super::*;
use mbv_emby::EmbyError;

/// A searchable test row: an Emby item of `item_type` with `id` and `name`.
fn searchable(id: &str, name: &str, item_type: &str) -> EmbyItem {
    let mut item = make_item(name, item_type);
    item.id = id.to_string();
    item
}

/// A stub `App` whose Search field has sent `query`. The stub has no Emby
/// Service attached, so the query stores and starts no worker.
fn queried_app(query: &str) -> App {
    let mut app = make_app_stub();
    app.gui_search_query(query.to_string());
    app
}

/// A stub `App` whose live query has landed `items` through the GUI tick
/// host's drain.
fn landed_app(query: &str, items: Vec<EmbyItem>) -> App {
    let mut app = queried_app(query);
    app.send_gui_search_result_for_test(query, items);
    assert!(app.drain_gui_search_results(), "the result applied");
    app
}

/// Contract: the drain applies a landed result for the live query to the
/// store, and the projection carries its rows in the provider's order with the
/// item identity the activate command needs (row 12.1).
#[test]
fn a_landed_result_for_the_live_query_projects_its_rows() {
    let app = landed_app(
        "blue",
        vec![
            searchable("m-1", "Blue Train", "Movie"),
            searchable("a-1", "Blue Album", "MusicAlbum"),
        ],
    );

    let projection = app.gui_search_projection();
    assert_eq!(projection.query, "blue");
    assert!(!projection.loading);
    assert!(!projection.failed);
    assert_eq!(
        projection
            .rows
            .iter()
            .map(|row| (row.id.as_str(), row.title.as_str(), row.item_type.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("m-1", "Blue Train", "Movie"),
            ("a-1", "Blue Album", "MusicAlbum"),
        ]
    );
}

/// Contract: the store keeps only the types the TUI's search sidebar shows, so
/// the two Clients list the same results (row 12.1).
#[test]
fn a_landed_result_keeps_only_navigable_rows() {
    let app = landed_app(
        "blue",
        vec![
            searchable("m-1", "Blue Train", "Movie"),
            searchable("c-1", "Blue Folder", "CollectionFolder"),
        ],
    );

    let projection = app.gui_search_projection();
    assert_eq!(projection.rows.len(), 1);
    assert_eq!(projection.rows[0].id, "m-1");
}

/// Contract: a result answering an older query is discarded, as the TUI's
/// stale-query guard discards it, and the store keeps no row (row 12.1).
#[test]
fn a_stale_result_changes_nothing() {
    let mut app = queried_app("jazz");
    app.send_gui_search_result_for_test("blues", vec![searchable("m-9", "Rocks", "Movie")]);

    assert!(!app.drain_gui_search_results(), "the stale result applied");

    let projection = app.gui_search_projection();
    assert_eq!(projection.query, "jazz");
    assert_eq!(projection.rows, vec![]);
}

/// Contract: a result applies only once, because the drain takes what the
/// channel holds and reports nothing after it (row 12.1).
#[test]
fn the_drain_reports_no_change_when_the_channel_is_empty() {
    let mut app = landed_app("blue", vec![searchable("m-1", "Blue Train", "Movie")]);

    assert!(!app.drain_gui_search_results(), "the channel held nothing");
    assert_eq!(app.gui_search_projection().rows.len(), 1);
}

/// Contract: an activate navigates from the stored row and leaves no overlay
/// request, because the GUI fronts its own results and mounts no sidebar (row
/// 12.1, design D2).
#[test]
fn activating_a_result_navigates_and_leaves_the_overlay_request_unset() {
    let mut app = landed_app("blue", vec![searchable("m-1", "Blue Train", "Movie")]);
    assert!(app.pending_overlay.is_none(), "no overlay request before");

    app.gui_search_activate("m-1".to_string(), "Movie".to_string());

    assert!(app.pending_overlay.is_none(), "no overlay request after");
    assert_eq!(
        app.gui_search_projection(),
        GuiSearchProjection::default(),
        "the results close, as the TUI's dismiss unmounts them"
    );
}

/// Contract: an activate of a row the store does not carry is a stale frame's
/// row — it navigates nowhere and leaves the results (row 12.1).
#[test]
fn activating_an_unknown_row_changes_nothing() {
    let mut app = landed_app("blue", vec![searchable("m-1", "Blue Train", "Movie")]);

    app.gui_search_activate("m-404".to_string(), "Movie".to_string());

    assert!(app.pending_overlay.is_none(), "no overlay request");
    assert_eq!(app.gui_search_projection().rows.len(), 1, "the rows stay");
}

/// Contract: an empty query closes the results, and a close clears the live
/// query's rows (row 12.1).
#[test]
fn an_empty_query_and_a_close_both_clear_the_store() {
    let mut app = landed_app("blue", vec![searchable("m-1", "Blue Train", "Movie")]);
    app.gui_search_query(String::new());
    assert_eq!(app.gui_search_projection(), GuiSearchProjection::default());

    let mut app = landed_app("blue", vec![searchable("m-1", "Blue Train", "Movie")]);
    app.gui_search_close();
    assert_eq!(app.gui_search_projection(), GuiSearchProjection::default());
}

/// Contract: a failed result ends the load, records the failure, and keeps no
/// row, so the frame never paints a stale list behind a failure (row 12.1).
#[test]
fn a_failed_result_records_the_failure_and_keeps_no_row() {
    let mut app = landed_app("blue", vec![searchable("m-1", "Blue Train", "Movie")]);
    app.send_gui_search_failure_for_test("blue", EmbyError::resolve("search failed"));
    assert!(app.drain_gui_search_results(), "the failure applied");

    let projection = app.gui_search_projection();
    assert!(projection.failed);
    assert!(!projection.loading);
    assert_eq!(projection.rows, vec![]);
}
