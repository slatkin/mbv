//! #745 integration regression: the global UI-state reset owns the
//! active/inactive owner, default-presentation, saved-state and
//! protected-state orchestration in one hermetic shell pass. Owner-local
//! reset behaviour is owned by `mbv-components`; the launch-restore
//! cancellation is owned by `tests/lifecycle/lifecycle_launch_restore.rs`.

use super::*;
use crate::app::state::playback::DestinationLatestSource;
use crate::app::tests::render_fixtures::{make_movie_app, make_queue_app};
use crate::app::{BrowseEvent, BrowseLevel, LibEvent, LibraryTab, TabSelection};
use mbv_audiobookshelf::AudiobookshelfLibrary;
use mbv_components::QueueCursorUpdate;
use mbv_components::emby_library_content::EmbyLibraryContent;
use mbv_emby_model::test_support::make_item;
use mbv_queue::ServiceKind;
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState;
use mbv_ui_model::browse::ServerRows;
use mbv_ui_model::library::{LibraryKey, LibraryKind};
use mbv_ui_model::sort_filter::{LetterFilter, LetterFilterKind};

fn movies_key(library_id: &str) -> LibraryKey {
    LibraryKey::Service {
        service: ServiceKind::Emby,
        library_id: library_id.into(),
        kind: LibraryKind::Movies,
    }
}

fn second_movies_library() -> LibraryTab {
    let mut library = make_item("Second Movies", "CollectionFolder");
    library.id = "lib-movies-2".into();
    library.is_folder = true;
    library.collection_type = "movies".into();
    let mut first = make_item("Second Focused", "Movie");
    first.id = "movie-2-focused".into();
    let mut second = make_item("Second Other", "Movie");
    second.id = "movie-2-other".into();
    LibraryTab {
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(2),
            parent_id: "lib-movies-2".into(),
            title: "Second Movies".into(),
            items: vec![first, second],
            resting: BrowseResting::new(0, 0),
            item_types: None,
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
    }
}

fn deep_level() -> BrowseLevel {
    let mut first = make_item("Focused Movie", "Movie");
    first.id = "movie-focused".into();
    let mut second = make_item("Second Movie", "Movie");
    second.id = "movie-second".into();
    BrowseLevel {
        rows: ServerRows::new(2),
        parent_id: "movie-focused".into(),
        title: "Focused Movie".into(),
        items: vec![first, second],
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
    }
}

fn queue_component_mut(model: &mut Model) -> &mut QueueComponent {
    model
        .application
        .get_component_mut(&ComponentId::Queue)
        .expect("mounted Queue")
        .as_any_mut()
        .downcast_mut::<QueueComponent>()
        .expect("Queue component")
}

fn queue_component(model: &Model) -> &QueueComponent {
    model
        .application
        .get_component(&ComponentId::Queue)
        .expect("mounted Queue")
        .as_any()
        .downcast_ref::<QueueComponent>()
        .expect("Queue component")
}

fn owner_cursor(model: &Model, key: &LibraryKey) -> usize {
    model
        .library_owner::<EmbyLibraryContent>(key)
        .expect("installed owner")
        .cursor()
}

fn seeded_reset_model() -> (
    Model,
    LibraryKey,
    LibraryKey,
    DestinationLatestSource,
    usize,
) {
    let mut app = make_queue_app(3);
    app.ui_volume = 37;
    app.mute_on = true;
    app.panel_mode = PanelMode::LibraryOnly;
    app.queue_column_width = 62;
    app.list_pane_width = Some(54);
    app.visual_slot_hidden = true;
    app.tab_scroll = 3;
    app.local_view.set_cursor(2);
    app.libs.push(second_movies_library());
    // A committed show pill whose owner reset returns to the state pill: the
    // shell-owned fan-out scope must follow (#745, design D5).
    let abs_library = AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut abs_state = AudiobookshelfBrowseState::new(abs_library.clone());
    abs_state.committed_show_pill = Some("show-a".into());
    app.audiobookshelf_libraries.push(abs_library);
    app.audiobookshelf_browse.push(abs_state);
    // Non-default root presentation plus a drilled-in level.
    app.libs[0].nav_stack[0].letter_filter = Some(LetterFilter::default_filter_for_kind(
        LetterFilterKind::from_collection_type("movies"),
    ));
    app.libs[0].nav_stack[0].unplayed_only = true;
    app.libs[0].nav_stack[0].sort_by = "DateCreated".into();
    app.libs[0].nav_stack[0].resting = BrowseResting::new(1, 4);
    app.libs[0].nav_stack.push(deep_level());
    // Saved presentation state that the reset must forget without deleting
    // unrelated values.
    std::fs::write(
        mbv_config::prefs_path(),
        serde_json::json!({
            "panel_focus": "queue_side",
            "ui_volume": 37,
            "unrelated_future_key": "keep",
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        mbv_config::tui_launch_state_path(),
        r#"{"version":1,"tab":{"type":"home"}}"#,
    )
    .unwrap();

    let mut model = Model::new(app);
    let acknowledged = DestinationLatestSource::Emby("lib-movies".into());
    model
        .acknowledged_home_latest_sources
        .insert(acknowledged.clone());
    model.sync_mounted_surfaces();

    let key1 = movies_key("lib-movies");
    let key2 = movies_key("lib-movies-2");
    model.push_library_owner(
        key2.clone(),
        Box::new(EmbyLibraryContent::new(LibraryKind::Movies)),
    );
    model.push_emby_library_owner_content(1, &key2, LibraryKind::Movies);
    model
        .library_owner_mut::<EmbyLibraryContent>(&key2)
        .expect("second owner")
        .apply_position(1, 2);
    model
        .library_owner_mut::<EmbyLibraryContent>(&key1)
        .expect("active owner")
        .apply_position(1, 3);
    queue_component_mut(&mut model).set_cursor(&QueueCursorUpdate::Set(2));
    let queue_len_before = model.app.local_view.total_queue_len();
    assert_eq!(owner_cursor(&model, &key1), 1);
    assert_eq!(owner_cursor(&model, &key2), 1);
    assert_eq!(queue_component(&model).test_cursor(), 2);
    (model, key1, key2, acknowledged, queue_len_before)
}

fn assert_default_presentation(model: &Model, key1: &LibraryKey, key2: &LibraryKey) {
    assert_eq!(model.app.tab, TabSelection::Home);
    assert_eq!(model.app.launch_restore, LaunchRestore::Done);
    assert_eq!(model.app.panel_mode, PanelMode::Both);
    assert_eq!(model.app.panel_focus, PanelFocus::Library);
    assert_eq!(model.app.mini_view_focus, PanelFocus::Queue);
    assert_eq!(model.app.queue_column_width, LEFT_WIDTH_DEFAULT);
    assert!(model.app.list_pane_width.is_none());
    assert!(!model.app.visual_slot_hidden);
    assert_eq!(model.app.tab_scroll, 0);
    assert_eq!(model.app.settings_destination, SettingsDestination::Main);
    // Browse roots: deep levels dropped, root fields back to defaults,
    // fetched content retained.
    assert_eq!(model.app.libs[0].nav_stack.len(), 1);
    assert_eq!(model.app.libs[1].nav_stack.len(), 1);
    let root = &model.app.libs[0].nav_stack[0];
    assert_eq!(root.sort_by, "SortName");
    assert_eq!(root.sort_order, "Ascending");
    assert!(!root.unplayed_only);
    assert!(root.letter_filter.is_none());
    assert_eq!(root.resting.cursor(), 0);
    assert_eq!(root.resting.scroll(), 0);
    assert_eq!(root.items.len(), 2, "root content retained");
    // Active and inactive owners both reset; Queue-local presentation reset.
    assert_eq!(owner_cursor(model, key1), 0);
    assert_eq!(owner_cursor(model, key2), 0);
    assert_eq!(queue_component(model).test_cursor(), 0);
    // The podcast owner's default state pill aligns the shell fan-out scope.
    assert!(
        model.app.audiobookshelf_browse[0]
            .committed_show_pill
            .is_none()
    );
}

fn assert_protected_state(
    model: &Model,
    acknowledged: &DestinationLatestSource,
    queue_len_before: usize,
) {
    assert_eq!(model.app.ui_volume, 37);
    assert!(model.app.mute_on);
    assert_eq!(model.app.local_view.total_queue_len(), queue_len_before);
    assert_eq!(model.app.local_view.cursor(), 2);
    assert!(
        model
            .acknowledged_home_latest_sources
            .contains(acknowledged)
    );
}

fn assert_saved_presentation_cleared(model: &Model) {
    assert!(!mbv_config::tui_launch_state_path().exists());
    let prefs: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(mbv_config::prefs_path()).unwrap()).unwrap();
    assert!(prefs.get("panel_focus").is_none());
    assert_eq!(prefs["ui_volume"], serde_json::json!(37));
    assert_eq!(prefs["unrelated_future_key"], serde_json::json!("keep"));
    assert_eq!(
        model.app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Success
    );
}

#[test]
fn reset_ui_state_restores_defaults_and_preserves_protected_state() {
    let (mut model, key1, key2, acknowledged, queue_len_before) = seeded_reset_model();

    model.reset_ui_state();

    assert_default_presentation(&model, &key1, &key2);
    assert_protected_state(&model, &acknowledged, queue_len_before);
    assert_saved_presentation_cleared(&model);
}

fn stale_root_level() -> BrowseLevel {
    let mut stale = make_item("Stale Movie", "Movie");
    stale.id = "movie-stale".into();
    BrowseLevel {
        rows: ServerRows::new(1),
        parent_id: "lib-movies".into(),
        title: "Movies".into(),
        items: vec![stale],
        resting: BrowseResting::new(0, 0),
        item_types: Some("Movie".into()),
        unplayed_only: true,
        sort_by: "DateCreated".into(),
        sort_order: "Descending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

/// #745: a root-level browse load issued before the reset carries the
/// discarded fetch fields. Its parent id matches the reset root, so the
/// existing parent check cannot reject it; the fetch-key guard must drop it.
#[test]
fn late_root_browse_completion_after_reset_keeps_reset_defaults() {
    let mut app = make_movie_app();
    // A root load in flight when the reset runs, carrying pre-reset intent.
    app.libs[0].nav_stack[0].loading = true;
    app.libs[0].nav_stack[0].sort_by = "DateCreated".into();
    app.libs[0].nav_stack[0].sort_order = "Descending".into();
    app.libs[0].nav_stack[0].unplayed_only = true;
    let mut model = Model::new(app);

    model.reset_ui_state();
    model
        .app
        .handle_lib_event(LibEvent::Browse(BrowseEvent::Loaded {
            lib_idx: 0,
            parent_id: "lib-movies".into(),
            level: Box::new(stale_root_level()),
        }));

    let root = &model.app.libs[0].nav_stack[0];
    assert_eq!(root.sort_by, "SortName");
    assert_eq!(root.sort_order, "Ascending");
    assert!(!root.unplayed_only);
    assert_eq!(
        root.items.len(),
        2,
        "the stale completion must not replace the reset root"
    );
}
