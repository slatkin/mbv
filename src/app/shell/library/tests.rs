use super::*;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::OverlayId;
use crate::app::render::make_movie_app;
use crate::app::state::types::browse::BrowseResting;
use crate::app::tests::make_item;
use crate::app::{
    BrowseLevel, FeedHomeVideoGroup, FeedHomeVideoState, PanelFocus, PanelMode, TabSelection,
};

// ADR 0024 D2 (task 2.3): the three-rung mouse-eligibility ladder.

fn eligibility_model() -> Model {
    let mut model = Model::new(make_movie_app());
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.app.panel_focus = PanelFocus::Library;
    model.app.panel_mode = PanelMode::Both;
    // Task 6.1: the full sync pass mounts the migrated owner into the panel,
    // points the panel at it (`sync_library_panel`), and routes focus
    // (`sync_active_destination`); the two-call pair would leave the panel's
    // active-owner pointer unset.
    model.sync_mounted_surfaces();
    model
}

#[test]
fn mouse_eligibility_rung3_is_painted_destination_plus_playback() {
    let model = eligibility_model();
    // Task 6.1: the migrated Movies surface is the mounted `LibraryPanel`
    // (its embedded owner is never a component), so the painted destination
    // child is the panel.
    let child = ComponentId::Library;
    let eligible: std::collections::HashSet<_> = model.mouse_eligible_ids().into_iter().collect();
    assert!(eligible.contains(&child));
    // The strip mounts only where `RootFrame` places it (task 4.1): in the
    // two-panel layout its `library_playback` placement is absent and the
    // `LibraryPlaybackPanel` is unmounted, so it is not mouse-eligible.
    assert!(!eligible.contains(&ComponentId::LibraryPlaybackPanel));
    assert!(
        !eligible
            .iter()
            .any(|id| matches!(id, ComponentId::Overlay(_) | ComponentId::Modal(_))),
        "rung 3 carries no overlay"
    );
}

#[test]
fn mouse_eligibility_rung1_blocking_overlay_is_exclusive() {
    use crate::app::components::{ConfirmComponent, ModalId};
    let mut model = eligibility_model();
    model
        .application
        .mount(
            ComponentId::Modal(ModalId::Confirm),
            Box::new(ConfirmComponent::new()),
            vec![],
        )
        .unwrap();
    assert_eq!(
        model.mouse_eligible_ids(),
        vec![ComponentId::Modal(ModalId::Confirm)]
    );
}

#[test]
fn mouse_eligibility_rung2_topmost_panel_overlay_is_exclusive() {
    let mut model = Model::new(make_movie_app());
    model.app.tab = TabSelection::Home;
    model.mount_help();
    assert_eq!(
        model.mouse_eligible_ids(),
        vec![ComponentId::Overlay(OverlayId::Help)]
    );
}

/// Task 2.9(a): the eligible set follows what is painted across a
/// wide/narrow breakpoint change and an overlay mount/unmount — it is
/// derived off `active_surface_id()`, never a second "did I paint" ledger.
#[test]
fn mouse_eligibility_follows_breakpoint_and_overlay_lifecycle() {
    let tv_child = |wide: bool| {
        let mut app = make_movie_app();
        app.libs[0].library.collection_type = "tvshows".into();
        for item in &mut app.libs[0].nav_stack[0].items {
            item.item_type = "Series".into();
        }
        app.tab = TabSelection::EmbyLibrary(0);
        app.panel_focus = PanelFocus::Library;
        app.panel_mode = PanelMode::Both;
        // Wide breakpoint is now driven synchronously by terminal size
        // (`wide_tv_library_area`), not this previous-frame paint rect.
        if wide {
            app.terminal_width = 160;
            app.terminal_height = 40;
        } else {
            app.terminal_width = 80;
            app.terminal_height = 24;
        }
        let mut model = Model::new(app);
        model.sync_tv_content();
        model.sync_mounted_surfaces();
        model.sync_active_destination();
        let eligible: std::collections::HashSet<_> =
            model.mouse_eligible_ids().into_iter().collect();
        (model, eligible)
    };

    // Task 8.4 (design D2): the TV surface is the mounted `LibraryPanel`
    // under `ComponentId::Library` at every breakpoint; no `Browser` id is
    // ever mounted for TV.
    let (_wide, wide_eligible) = tv_child(true);
    assert!(wide_eligible.contains(&ComponentId::Library));

    let (_narrow, narrow_eligible) = tv_child(false);
    assert!(narrow_eligible.contains(&ComponentId::Library));

    let mut model = eligibility_model();
    // Task 6.1: the migrated Movies surface is the mounted `LibraryPanel`.
    let child = ComponentId::Library;
    assert!(model.mouse_eligible_ids().contains(&child));
    model.mount_help();
    assert_eq!(
        model.mouse_eligible_ids(),
        vec![ComponentId::Overlay(OverlayId::Help)]
    );
    model
        .application
        .umount(&ComponentId::Overlay(OverlayId::Help))
        .unwrap();
    assert!(model.mouse_eligible_ids().contains(&child));
}

// Task 5.2 (ADR 0024 D2): the five already-handling surfaces ride the
// same eligibility ladder — each sidebar / inline search is eligible
// alone while it is the painted overlay or destination child, and every
// one is ineligible while a blocking overlay covers it.
#[test]
fn phase4_surfaces_are_eligible_alone_and_ineligible_under_a_blocking_overlay() {
    use crate::app::components::{ConfirmComponent, ModalId};
    use crate::app::SidebarId;

    let confirm_id = ComponentId::Modal(ModalId::Confirm);
    let mount_confirm = |model: &mut Model| {
        model
            .application
            .mount(
                confirm_id.clone(),
                Box::new(ConfirmComponent::new()),
                vec![],
            )
            .unwrap();
    };

    // The three overlay sidebars: the topmost mounted overlay is
    // eligible alone; the blocking modal takes exclusivity from all.
    for (sidebar, id) in [
        (
            SidebarId::Settings,
            ComponentId::Overlay(OverlayId::Settings),
        ),
        (
            SidebarId::Sessions,
            ComponentId::Overlay(OverlayId::Sessions),
        ),
        (
            SidebarId::Playlists,
            ComponentId::Overlay(OverlayId::Playlists),
        ),
    ] {
        let mut model = eligibility_model();
        model.mount_sidebar(sidebar);
        model.sync_mouse_subscriptions();
        assert!(model.mouse_subscribed.contains(&id), "{id:?}");
        mount_confirm(&mut model);
        model.sync_mouse_subscriptions();
        assert!(!model.mouse_subscribed.contains(&id), "{id:?}");
        assert_eq!(
            model.mouse_subscribed,
            std::iter::once(confirm_id.clone()).collect()
        );
    }

    // Help mounts through its own path but the same rung applies.
    let mut model = eligibility_model();
    let help_id = ComponentId::Overlay(OverlayId::Help);
    model.mount_help();
    model.sync_mouse_subscriptions();
    assert!(model.mouse_subscribed.contains(&help_id));
    mount_confirm(&mut model);
    model.sync_mouse_subscriptions();
    assert!(!model.mouse_subscribed.contains(&help_id));
}

#[test]
fn sync_mouse_subscriptions_tracks_and_wipes_the_eligible_set() {
    use crate::app::components::{ConfirmComponent, ModalId};
    let mut model = eligibility_model();
    model.sync_mouse_subscriptions();
    // Task 6.1: the migrated Movies surface is the mounted `LibraryPanel`.
    let child = ComponentId::Library;
    assert!(model.mouse_subscribed.contains(&child));
    // The strip is not mounted in the two-panel layout (no
    // `library_playback` placement), so it draws no subscription.
    assert!(!model
        .mouse_subscribed
        .contains(&ComponentId::LibraryPlaybackPanel));

    model
        .application
        .mount(
            ComponentId::Modal(ModalId::Confirm),
            Box::new(ConfirmComponent::new()),
            vec![],
        )
        .unwrap();
    model.sync_mouse_subscriptions();
    assert_eq!(
        model.mouse_subscribed,
        std::iter::once(ComponentId::Modal(ModalId::Confirm)).collect()
    );
}

#[test]
fn shell_routes_focus_to_the_active_destination_child() {
    let mut model = Model::new(make_movie_app());
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.app.panel_focus = PanelFocus::Library;
    model.app.panel_mode = PanelMode::Both;
    model.sync_mounted_surfaces();

    // Task 6.1: the migrated Movies surface routes through the mounted
    // `LibraryPanel`; the embedded owner is never a component.
    let child = ComponentId::Library;
    assert_eq!(model.application.focus(), Some(&child));
    assert!(model
        .application
        .get_component(&child)
        .unwrap()
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .is_some());
}

/// unify-screens-under-panel-components task 8.4 (design D2): the TV owner is
/// registered under `LibraryKey::Service(TvShows)` inside the mounted
/// `LibraryPanel` at every breakpoint; no standalone `Browser` or TV-specific
/// component id is ever mounted for a `tvshows` library.
#[test]
fn narrow_and_wide_tv_library_both_route_to_the_library_panel() {
    let build = |wide: bool| {
        let mut app = make_movie_app();
        app.libs[0].library.collection_type = "tvshows".into();
        for item in &mut app.libs[0].nav_stack[0].items {
            item.item_type = "Series".into();
        }
        app.tab = TabSelection::EmbyLibrary(0);
        app.panel_focus = PanelFocus::Library;
        app.panel_mode = PanelMode::Both;
        // Wide breakpoint is now driven synchronously by terminal size
        // (`wide_tv_library_area`), not this previous-frame paint rect.
        if wide {
            app.terminal_width = 160;
            app.terminal_height = 40;
        } else {
            app.terminal_width = 80;
            app.terminal_height = 24;
        }
        let mut model = Model::new(app);
        model.sync_tv_content();
        model.sync_mounted_surfaces();
        model.sync_active_destination();
        model
    };

    for wide in [false, true] {
        let model = build(wide);
        assert_eq!(model.application.focus(), Some(&ComponentId::Library));
        assert!(model.library_panel_has_owner(&model.test_tv_owner_key()));
    }
}

/// migrate-narrow-browse-to-components task 2.2, converted by task 6.1
/// (design D2): every `is_feed_home_video_group_view` Emby library — a
/// configured home-video feed-view library — is a migrated kind whose
/// surface routes through the mounted `LibraryPanel` at every width. Both
/// take TuiRealm focus on `ComponentId::Library`, narrow and wide, with the
/// embedded `EmbyLibraryContent` owner installed.
#[test]
fn feed_group_picker_libraries_route_to_the_library_panel_at_every_width() {
    let build = |wide: bool| {
        let mut app = make_movie_app();
        let lib = &mut app.libs[0];
        lib.library.name = "Feed".into();
        lib.library.collection_type = "homevideos".into();
        let mut folder = make_item("Channel A", "Folder");
        folder.id = "folder-a".into();
        folder.is_folder = true;
        let mut v1 = make_item("V1", "Episode");
        v1.id = "v1".into();
        let mut v2 = make_item("V2", "Episode");
        v2.id = "v2".into();
        lib.nav_stack = vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-movies".into(),
            title: "Feed".into(),
            items: vec![folder.clone()],
            total_count: 1,
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
        }];
        lib.feed_home_video = Some(FeedHomeVideoState {
            all_items: vec![v1, v2.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: vec![v2],
            }],
            loading: false,
            ..FeedHomeVideoState::default()
        });
        app.config.lock().unwrap().feed_view_libraries = vec!["feed".into()];
        app.tab = TabSelection::EmbyLibrary(0);
        app.panel_focus = PanelFocus::Library;
        app.panel_mode = PanelMode::Both;
        app.terminal_width = if wide { 160 } else { 80 };
        app.terminal_height = 40;
        let mut model = Model::new(app);
        assert!(model.app.is_feed_home_video_group_view(0));
        model.sync_mounted_surfaces();
        model
    };

    for wide in [false, true] {
        let model = build(wide);
        assert!(
            model.active_emby_library_owner().is_some(),
            "wide={wide}: the feed-group library is a migrated kind"
        );
        assert_eq!(
            model.application.focus(),
            Some(&ComponentId::Library),
            "wide={wide}: focus lands on the mounted Library panel"
        );
        assert!(model
            .application
            .get_component(&ComponentId::Library)
            .unwrap()
            .as_any()
            .downcast_ref::<LibraryPanel>()
            .is_some());
    }
}

/// unify-screens-under-panel-components task 8.4 (design D2): drive a TV
/// library through wide -> narrow -> wide via `sync_mounted_surfaces()` in
/// production order. The panel-hosted owner stays installed and focused
/// across every flip -- no second id is ever mounted.
#[test]
fn tv_library_wide_narrow_wide_transition_routes_and_focuses_correctly() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::Both;
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut model = Model::new(app);

    // The breakpoint is now driven synchronously by terminal size (the
    // flash fix): `prime_wide_tv_geometry` in `sync_mounted_surfaces`
    // derives the TV breakpoint before the mount gates read it.
    let widen = |model: &mut Model, wide: bool| {
        model.app.terminal_width = if wide { 160 } else { 80 };
    };

    let assert_tv_focused = |model: &Model| {
        assert_eq!(model.application.focus(), Some(&ComponentId::Library));
        assert!(model.library_panel_has_owner(&model.test_tv_owner_key()));
    };

    // Wide.
    widen(&mut model, true);
    model.sync_mounted_surfaces();
    assert_tv_focused(&model);

    // Narrow: the same owner stays installed and focused.
    widen(&mut model, false);
    model.sync_mounted_surfaces();
    assert_tv_focused(&model);

    // Wide again: still the same owner.
    widen(&mut model, true);
    model.sync_mounted_surfaces();
    assert_tv_focused(&model);
}

#[test]
fn shell_routes_focus_to_the_library_panel_for_a_music_library() {
    let mut model = Model::new(make_movie_app());
    // Every library destination routes through the mounted Library panel.
    model.app.libs[0].library.collection_type = "music".into();
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.app.panel_focus = PanelFocus::Library;
    model.app.panel_mode = PanelMode::Both;
    model.sync_active_destination();

    assert_eq!(model.application.focus(), Some(&ComponentId::Library));
}

#[test]
fn shell_skips_focus_routing_while_an_overlay_is_mounted() {
    let mut model = Model::new(make_movie_app());
    model.app.tab = TabSelection::Home;
    model.mount_help();
    model.sync_active_destination();

    assert_eq!(
        model.application.focus(),
        Some(&ComponentId::Overlay(OverlayId::Help))
    );
}

/// Production-style acceptance test for #610 / #607: when Queue owns
/// panel focus, the per-tick sync sequence (`sync_queue` followed by
/// `sync_active_destination` in `shell/run/mod.rs`) must leave
/// `ComponentId::Queue` as the active TuiRealm component. Without the
/// Queue-owner guard in `sync_active_destination`, the destination
/// sync re-activates the Library child (or `UiRoot`) on top of Queue,
/// and Queue falls back to legacy key routing.
#[test]
fn shell_preserves_queue_focus_across_destination_sync() {
    use crate::app::components::QueueComponent;
    let mut model = Model::new(make_movie_app());
    // Pretend a user action (Alt+Right, mouse click, etc.) just
    // moved panel focus to Queue. With no overlay mounted, this is
    // exactly the precondition the production main loop sees each
    // tick once `sync_queue` activates the component.
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.app.panel_focus = PanelFocus::Queue;
    model.app.panel_mode = PanelMode::Both;

    // Mirror the production call order at shell/run/mod.rs.
    model.sync_queue();
    model.sync_active_destination();

    assert!(
        model.application.mounted(&ComponentId::Queue),
        "sync_queue must mount Queue so it can claim focus"
    );
    assert_eq!(
        model.application.focus(),
        Some(&ComponentId::Queue),
        "Queue must remain the active TuiRealm component when it owns panel focus"
    );
    // The component is the Queue surface, not a re-claimed destination
    // or UiRoot fallback. A downcast succeeds iff focus is actually
    // on the Queue component (i.e., it's mounted and active).
    let component = model
        .application
        .get_component(&ComponentId::Queue)
        .expect("Queue mounted")
        .as_any()
        .downcast_ref::<QueueComponent>();
    assert!(
        component.is_some(),
        "active component must be QueueComponent when Queue owns panel focus"
    );
}

/// Symmetric regression guard under D3 (single focus pass): when a
/// blocking modal is up, `sync_active_destination` short-circuits on
/// `overlay_holds_focus()` — the modal owns native LIFO focus (D3
/// first-match-wins), never the destination child. `sync_queue` also
/// skips activation under blocking overlays. After the modal is
/// dismissed, the next `sync_active_destination` pass routes focus back
/// to the destination child.
#[test]
fn shell_blocking_overlay_owns_focus_and_dismiss_returns_to_destination() {
    use crate::app::components::{ConfirmComponent, ModalId};
    let mut model = Model::new(make_movie_app());
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.app.panel_focus = PanelFocus::Library;
    model.app.panel_mode = PanelMode::Both;
    // First: the destination child owns focus (the single focus pass).
    // Task 6.1: the migrated Movies surface is the mounted `LibraryPanel`.
    model.sync_mounted_surfaces();
    let child = ComponentId::Library;
    assert_eq!(model.application.focus(), Some(&child));

    // The production modal-open path mounts AND activates the blocking
    // modal; the destination pass must not stomp it (D3 first-match).
    model
        .application
        .mount(
            ComponentId::Modal(ModalId::Confirm),
            Box::new(ConfirmComponent::new()),
            vec![],
        )
        .expect("mount Confirm");
    model
        .application
        .active(&ComponentId::Modal(ModalId::Confirm))
        .expect("activate Confirm");
    model.sync_queue();
    model.sync_active_destination();
    assert_eq!(
        model.application.focus(),
        Some(&ComponentId::Modal(ModalId::Confirm)),
        "the blocking modal must own TuiRealm focus while mounted (D3 first-match)"
    );

    // Dismiss the modal; the next destination pass routes focus to the
    // destination child.
    model
        .application
        .umount(&ComponentId::Modal(ModalId::Confirm))
        .expect("dismiss Confirm");
    model.sync_active_destination();
    assert_eq!(
        model.application.focus(),
        Some(&child),
        "dismissing the blocking modal must return focus to the destination child"
    );
}

/// keep-destination-components-mounted task 4.2: with `active()` removed
/// from every `sync_*` (D1), the FIRST tick after startup must land
/// TuiRealm focus on the active destination child via the single
/// `sync_active_destination` pass (D3). `Model::new` activates UiRoot;
/// after the destination mounts, the focus pass must route to the child.
#[test]
fn shell_first_tick_focus_lands_on_the_active_destination_child() {
    let mut model = Model::new(make_movie_app());
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.app.panel_focus = PanelFocus::Library;
    model.app.panel_mode = PanelMode::Both;
    // Startup state: UiRoot is active (Model::new), no destination child
    // has been activated yet (no prior active() in sync_*).
    assert_eq!(model.application.focus(), Some(&ComponentId::UiRoot));

    // Mirror the first tick: the full sync pass mounts the destination
    // owner, then the single focus pass routes to the child. Task 6.1: the
    // migrated Movies surface is the mounted `LibraryPanel`.
    model.sync_mounted_surfaces();

    let child = ComponentId::Library;
    assert_eq!(
        model.application.focus(),
        Some(&child),
        "the first tick must land focus on the active destination child"
    );
}

/// keep-destination-components-mounted task 4.2: after dismissing an
/// overlay, TuiRealm's LIFO stack restores focus to the prior component;
/// the next `sync_active_destination` pass must re-route it to the active
/// destination child (not a stale lazily-mounted sibling or UiRoot).
#[test]
fn shell_overlay_dismiss_returns_focus_to_the_active_destination_child() {
    let mut model = Model::new(make_movie_app());
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.app.panel_focus = PanelFocus::Library;
    model.app.panel_mode = PanelMode::Both;
    // Task 6.1: the migrated Movies surface is the mounted `LibraryPanel`.
    model.sync_mounted_surfaces();
    let child = ComponentId::Library;
    assert_eq!(model.application.focus(), Some(&child));

    // Mount Help (overlay owns focus); the destination pass short-circuits.
    model.mount_help();
    assert_eq!(
        model.application.focus(),
        Some(&ComponentId::Overlay(OverlayId::Help))
    );
    model.sync_active_destination();
    assert_eq!(
        model.application.focus(),
        Some(&ComponentId::Overlay(OverlayId::Help)),
        "an overlay keeps focus while mounted"
    );

    // Dismiss the overlay: LIFO restores focus to the prior component;
    // the next destination pass must route it back to the destination
    // child (never a stale sibling or UiRoot).
    model.umount_help();
    model.sync_active_destination();
    assert_eq!(
        model.application.focus(),
        Some(&child),
        "overlay dismiss must return focus to the active destination child"
    );
}

/// keep-destination-components-mounted task 4.2 (D4), converted by task 6.1
/// (design D2): a catalog-retained but inactive owner paints nothing. The
/// panel's `view` paints only `owners.active_mut()`, and the shell's
/// transitional gate is per-active-tab, so with another tab active the
/// Movies owner stays installed (its cursor/scroll survive — the retention
/// rule) yet contributes no cell to the frame; re-activating the library
/// repaints it. Deterministic proof by buffer content, the panel-output
/// form of the old component's frame diff.
#[test]
fn mounted_but_inactive_library_owner_paints_nothing() {
    fn draw_text(model: &mut Model, width: u16, height: u16) -> String {
        model.app.terminal_width = width;
        model.app.terminal_height = height;
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| model.draw_frame(f, false, false)).unwrap();
        let buffer = term.backend().buffer();
        let mut out = String::new();
        for y in 0..buffer.area().height {
            for x in 0..buffer.area().width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    let mut model = Model::new(make_movie_app());
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.app.panel_focus = PanelFocus::Library;
    model.app.panel_mode = PanelMode::Both;
    model.sync_mounted_surfaces();
    let key = model
        .active_emby_library_owner()
        .map(|(_, key, _)| key)
        .expect("the Movies owner has migrated");

    // Active: the panel paints the owner's rows (gate sanity).
    let active = draw_text(&mut model, 120, 40);
    assert!(
        active.contains("Focused Movie") && active.contains("Second Movie"),
        "the active owner must add its rows to the frame:\n{active}"
    );

    // Switch to another migrated tab: the Movies owner is retained but
    // inactive, and the panel paints only the active owner — no Movies cell
    // reaches the frame.
    model.app.tab = TabSelection::Home;
    model.sync_mounted_surfaces();
    let inactive = draw_text(&mut model, 120, 40);
    assert!(
        !inactive.contains("Focused Movie") && !inactive.contains("Second Movie"),
        "a mounted-but-inactive owner must paint nothing:\n{inactive}"
    );
    assert!(
        model.library_panel_has_owner(&key),
        "the owner stays installed while its library is in the catalog"
    );

    // Re-activating the library repaints the retained owner.
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.sync_mounted_surfaces();
    let reactivated = draw_text(&mut model, 120, 40);
    assert!(
        reactivated.contains("Focused Movie") && reactivated.contains("Second Movie"),
        "re-activating the library repaints its retained owner:\n{reactivated}"
    );
}
