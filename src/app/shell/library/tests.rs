use super::*;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::OverlayId;
use crate::app::render::make_movie_app;
use crate::app::{PanelMode, TabSelection};

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
/// migrate-narrow-browse-to-components task 2.2, converted by task 6.1
/// (design D2): every `is_feed_home_video_group_view` Emby library — a
/// configured home-video feed-view library — is a migrated kind whose
/// surface routes through the mounted `LibraryPanel` at every width. Both
/// take TuiRealm focus on `ComponentId::Library`, narrow and wide, with the
/// embedded `EmbyLibraryContent` owner installed.
/// unify-screens-under-panel-components task 8.4 (design D2): drive a TV
/// library through wide -> narrow -> wide via `sync_mounted_surfaces()` in
/// production order. The panel-hosted owner stays installed and focused
/// across every flip -- no second id is ever mounted.
/// Production-style acceptance test for #610 / #607: when Queue owns
/// panel focus, the per-tick sync sequence (`sync_queue` followed by
/// `sync_active_destination` in `shell/run.rs`) must leave
/// `ComponentId::Queue` as the active TuiRealm component. Without the
/// Queue-owner guard in `sync_active_destination`, the destination
/// sync re-activates the Library child (or `UiRoot`) on top of Queue,
/// and Queue falls back to legacy key routing.
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
