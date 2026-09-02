use super::components::{BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::{PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    /// Route TuiRealm's native LIFO focus to the active destination's child
    /// component, or back to `UiRoot` when the destination has no mounted
    /// surface component (e.g. a narrow non-wide grouped-Music Emby
    /// library). Idempotent: `active()` on the already-active component is a
    /// no-op. The destination child is derived directly from `App.tab` via
    /// `library_child_id`; there is no Library-parent component or routing
    /// mirror.
    ///
    /// Short-circuits when Queue owns panel focus (and no blocking overlay
    /// is up): `sync_queue` already activated `ComponentId::Queue` a few
    /// lines earlier in the same tick, and we must not stomp it by
    /// re-activating the Library child or `UiRoot` on top. Without this
    /// guard Queue falls back to legacy key routing (issue #610, blocks
    /// the #607 acceptance gate). Mirrors the exact condition
    /// `sync_queue` uses to claim focus.
    pub(super) fn sync_active_destination(&mut self) {
        if self.overlay_holds_focus() {
            return;
        }
        let queue_owns_focus = matches!(self.app.effective_panel_focus(), PanelFocus::Queue)
            && !self.blocking_overlay_active();
        if queue_owns_focus {
            return;
        }
        let target = self
            .library_child_id()
            .filter(|child| self.application.mounted(child))
            .unwrap_or(ComponentId::UiRoot);
        if self.application.mounted(&target) {
            self.application
                .active(&target)
                .expect("activate Library child");
        }
    }

    fn library_child_id(&self) -> Option<ComponentId> {
        match self.app.tab {
            TabSelection::Home => Some(ComponentId::Home),
            TabSelection::Feeds => Some(ComponentId::Feeds),
            TabSelection::EmbyLibrary(index) => self.emby_library_child_id(index),
            TabSelection::AudiobookshelfLibrary(index) => self.abs_library_child_id(index),
        }
    }

    fn emby_library_child_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.libs.get(index)?;
        if let Some(id) = self.inline_search_component_id(index) {
            return Some(id);
        }
        let kind = BrowserKind::from_collection_type(&library.library.collection_type);
        // Wide TV focuses `TvWorkspaceComponent` under its distinct
        // `ComponentId::TvWorkspace`; narrow TV focuses the mounted
        // `BrowserComponent` under `ComponentId::Browser` (D4). The two
        // mount gates share `is_wide_tv_active()`, so mirror that split here.
        if kind == BrowserKind::TvShows && self.app.layout.main.is_wide_tv_active() {
            return Some(ComponentId::TvWorkspace(BrowserKey {
                service: ServiceKind::Emby,
                library_id: library.library.id.clone(),
                kind,
            }));
        }
        let mounted_surface = match kind {
            BrowserKind::Generic | BrowserKind::Movies | BrowserKind::HomeVideos => true,
            // Narrow TV focuses the mounted BrowserComponent (D4), matching
            // `emby_browser_component_id`.
            BrowserKind::TvShows => true,
            BrowserKind::Music => {
                // Music mounts one component type at all widths (no TV-style
                // split), so narrow Music is focusable too — the mount gate is
                // already width-agnostic; only this focus gate was wide-only.
                self.app.is_music_group_view(index) && self.app.is_viewing_album_folders(index)
            }
            BrowserKind::AudiobookshelfPodcast | BrowserKind::AudiobookshelfBook => false,
        };
        mounted_surface.then_some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind,
        }))
    }

    fn abs_library_child_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.audiobookshelf_libraries.get(index)?;
        let kind = match self.app.audiobookshelf_kind_at(index)? {
            AudiobookshelfBrowseKind::Podcast => BrowserKind::AudiobookshelfPodcast,
            AudiobookshelfBrowseKind::Book => BrowserKind::AudiobookshelfBook,
        };
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Audiobookshelf,
            library_id: library.id.clone(),
            kind,
        }))
    }

    /// Whether any sidebar, modal, or popup overlay is mounted. Every
    /// focus-management sync pass (`sync_active_destination`, `sync_queue`)
    /// consults this before re-activating its own surface, so a mounted
    /// overlay that just took focus is never stolen back on the next tick.
    pub(super) fn overlay_holds_focus(&self) -> bool {
        [
            ComponentId::Overlay(super::components::OverlayId::Search),
            ComponentId::Overlay(super::components::OverlayId::Settings),
            ComponentId::Overlay(super::components::OverlayId::Sessions),
            ComponentId::Overlay(super::components::OverlayId::Playlists),
            ComponentId::Overlay(super::components::OverlayId::Help),
            ComponentId::Overlay(super::components::OverlayId::ContextMenu),
            ComponentId::Overlay(super::components::OverlayId::SelectionModal),
            ComponentId::Modal(super::components::ModalId::Confirm),
            ComponentId::Modal(super::components::ModalId::DaemonLost),
            ComponentId::Modal(super::components::ModalId::RemoteReanchor),
            ComponentId::Modal(super::components::ModalId::SavePlaylist),
            ComponentId::Popup(super::components::PopupId::Multiselect),
            ComponentId::Popup(super::components::PopupId::LibraryRoutes),
            ComponentId::Popup(super::components::PopupId::FeedManage),
        ]
        .iter()
        .any(|id| self.application.mounted(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::BrowserComponent;
    use crate::app::components::OverlayId;
    use crate::app::render::make_movie_app;
    use crate::app::tests::make_item;
    use crate::app::types_browse::BrowseResting;
    use crate::app::{
        BrowseLevel, FeedHomeVideoGroup, FeedHomeVideoState, PanelFocus, PanelMode, TabSelection,
    };

    #[test]
    fn shell_routes_focus_to_the_active_destination_child() {
        let mut model = Model::new(make_movie_app());
        model.app.tab = TabSelection::EmbyLibrary(0);
        model.app.panel_focus = PanelFocus::Library;
        model.app.panel_mode = PanelMode::Both;
        model.sync_emby_browser();
        model.sync_active_destination();

        let child = model
            .emby_browser_id
            .clone()
            .expect("generic browser mounted");
        assert_eq!(model.application.focus(), Some(&child));
        assert!(model
            .application
            .get_component(&child)
            .unwrap()
            .as_any()
            .downcast_ref::<BrowserComponent>()
            .is_some());
    }

    /// migrate-narrow-browse-to-components task 2.2 (D4): a narrow Emby TV
    /// library routes to the mounted `BrowserComponent` (flat series list);
    /// a wide one still routes to `TvWorkspaceComponent`. The two are never
    /// both `Some` for the same library at any width.
    #[test]
    fn narrow_tv_library_routes_to_browser_component_wide_to_tv_workspace() {
        use crate::app::components::TvWorkspaceComponent;
        use ratatui::layout::Rect;

        let build = |wide: bool| {
            let mut app = make_movie_app();
            app.libs[0].library.collection_type = "tvshows".into();
            app.tab = TabSelection::EmbyLibrary(0);
            app.panel_focus = PanelFocus::Library;
            app.panel_mode = PanelMode::Both;
            app.layout.main.tv_wide_right_area = if wide {
                Rect::new(40, 0, 60, 20)
            } else {
                Rect::default()
            };
            let mut model = Model::new(app);
            model.sync_tv_workspace();
            model.sync_emby_browser();
            model.sync_active_destination();
            model
        };

        // Narrow: BrowserComponent owns the surface and focus.
        let narrow = build(false);
        assert_eq!(narrow.tv_workspace_id, None);
        let browser_id = narrow.emby_browser_id.clone().expect("narrow TV browser");
        assert!(matches!(browser_id, ComponentId::Browser(_)));
        assert_eq!(narrow.application.focus(), Some(&browser_id));
        assert!(narrow
            .application
            .get_component(&browser_id)
            .unwrap()
            .as_any()
            .downcast_ref::<BrowserComponent>()
            .is_some());

        // Wide: TvWorkspaceComponent owns the surface and focus.
        let wide = build(true);
        assert_eq!(wide.emby_browser_id, None);
        let tv_id = wide.tv_workspace_id.clone().expect("wide TV workspace");
        assert_eq!(wide.application.focus(), Some(&tv_id));
        assert!(wide
            .application
            .get_component(&tv_id)
            .unwrap()
            .as_any()
            .downcast_ref::<TvWorkspaceComponent>()
            .is_some());
    }

    /// migrate-narrow-browse-to-components task 2.2: every
    /// `is_feed_home_video_group_view` Emby library — podcast channels and
    /// configured home-video feed-view libraries alike — is owned by the
    /// mounted `BrowserComponent` at every width. Both resolve
    /// `Some(ComponentId::Browser(..))` and take TuiRealm focus, narrow and
    /// wide.
    #[test]
    fn feed_group_picker_libraries_route_to_browser_component_at_every_width() {
        let build = |podcast: bool, wide: bool| {
            let mut app = make_movie_app();
            let lib = &mut app.libs[0];
            lib.library.name = "Feed".into();
            if podcast {
                lib.library.item_type = "Channel".into();
            } else {
                lib.library.collection_type = "homevideos".into();
            }
            let mut folder = make_item("Channel A", "Folder");
            folder.id = "folder-a".into();
            folder.is_folder = true;
            let mut v1 = make_item("V1", "Episode");
            v1.id = "v1".into();
            let mut v2 = make_item("V2", "Episode");
            v2.id = "v2".into();
            lib.nav_stack = vec![BrowseLevel {
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
            app.layout.main.movies_wide_area = if wide {
                ratatui::layout::Rect::new(0, 0, 200, 50)
            } else {
                ratatui::layout::Rect::default()
            };
            let mut model = Model::new(app);
            assert!(model.app.is_feed_home_video_group_view(0));
            model.sync_emby_browser();
            model.sync_active_destination();
            model
        };

        for podcast in [false, true] {
            for wide in [false, true] {
                let model = build(podcast, wide);
                let id = model
                    .emby_browser_id
                    .clone()
                    .unwrap_or_else(|| panic!("podcast={podcast} wide={wide}: browser id"));
                assert!(matches!(id, ComponentId::Browser(_)));
                assert_eq!(model.application.focus(), Some(&id));
                assert!(model
                    .application
                    .get_component(&id)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<BrowserComponent>()
                    .is_some());
            }
        }
    }

    /// migrate-narrow-browse-to-components task 2.2 (reviewer block): drive a
    /// TV library through wide -> narrow -> wide via `sync_mounted_surfaces()`
    /// in production order (not the individual `sync_*` out of order). Because
    /// wide TV now mounts under `ComponentId::TvWorkspace` and narrow TV under
    /// `ComponentId::Browser`, both components can stay mounted across the
    /// flips and the active-destination pointer alone gates render/focus. At
    /// each step the mounted component *type* under the resolved id and the
    /// focus target must match the width.
    #[test]
    fn tv_library_wide_narrow_wide_transition_routes_and_focuses_correctly() {
        use crate::app::components::TvWorkspaceComponent;

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
        // recomputes `tv_wide_*` before the mount gates read it.
        let widen = |model: &mut Model, wide: bool| {
            model.app.terminal_width = if wide { 160 } else { 80 };
        };

        // Wide: TvWorkspaceComponent owns the surface and focus.
        widen(&mut model, true);
        model.sync_mounted_surfaces();
        let tv_id = model.tv_workspace_id.clone().expect("wide TV workspace id");
        assert!(matches!(tv_id, ComponentId::TvWorkspace(_)));
        assert_eq!(model.emby_browser_id, None);
        assert_eq!(model.application.focus(), Some(&tv_id));
        assert!(model
            .application
            .get_component(&tv_id)
            .unwrap()
            .as_any()
            .downcast_ref::<TvWorkspaceComponent>()
            .is_some());

        // Narrow: BrowserComponent owns the surface and focus; the TV
        // workspace stays mounted (keep-mounted) but is no longer the pointer.
        widen(&mut model, false);
        model.sync_mounted_surfaces();
        let browser_id = model.emby_browser_id.clone().expect("narrow TV browser id");
        assert!(matches!(browser_id, ComponentId::Browser(_)));
        assert_eq!(model.tv_workspace_id, None);
        assert!(
            model.application.mounted(&tv_id),
            "the wide TV workspace stays mounted across the narrow flip"
        );
        assert_eq!(model.application.focus(), Some(&browser_id));
        assert!(model
            .application
            .get_component(&browser_id)
            .unwrap()
            .as_any()
            .downcast_ref::<BrowserComponent>()
            .is_some());

        // Wide again: the same TvWorkspaceComponent is re-pointed and focused.
        widen(&mut model, true);
        model.sync_mounted_surfaces();
        assert_eq!(model.tv_workspace_id.as_ref(), Some(&tv_id));
        assert_eq!(model.emby_browser_id, None);
        assert_eq!(model.application.focus(), Some(&tv_id));
        assert!(model
            .application
            .get_component(&tv_id)
            .unwrap()
            .as_any()
            .downcast_ref::<TvWorkspaceComponent>()
            .is_some());
    }

    #[test]
    fn shell_routes_focus_back_to_ui_root_without_a_mounted_child() {
        let mut model = Model::new(make_movie_app());
        // A narrow (non-wide) grouped-Music library has no surface component
        // yet; the destination falls back to UiRoot (whose terminal
        // translation owns the remaining legacy key dispatch for those
        // surfaces). Podcast / feed-group libraries now route to the mounted
        // BrowserComponent (migrate-narrow-browse task 2.2).
        model.app.libs[0].library.collection_type = "music".into();
        model.app.tab = TabSelection::EmbyLibrary(0);
        model.app.panel_focus = PanelFocus::Library;
        model.app.panel_mode = PanelMode::Both;
        model.sync_active_destination();

        assert_eq!(model.application.focus(), Some(&ComponentId::UiRoot));
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
    /// `sync_active_destination` in `shell_run.rs`) must leave
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

        // Mirror the production call order at shell_run.rs:427-433.
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
        model.sync_emby_browser();
        model.sync_active_destination();
        let child = model
            .emby_browser_id
            .clone()
            .expect("generic browser mounted");
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

        // Mirror the first tick: mount the destination, then the single focus
        // pass routes to the child.
        model.sync_emby_browser();
        model.sync_active_destination();

        let child = model
            .emby_browser_id
            .clone()
            .expect("generic browser mounted");
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
        model.sync_emby_browser();
        model.sync_active_destination();
        let child = model
            .emby_browser_id
            .clone()
            .expect("generic browser mounted");
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

    /// keep-destination-components-mounted task 4.3 (D4): a mounted-but-
    /// inactive destination component paints nothing. `render_emby_browser_`
    /// `component` early-returns when the `*_id` pointer is `None` (narrow /
    /// drilled away / inactive), so the mounted instance never paints over
    /// the legacy frame. Deterministic proof by frame diff over COMPLETE
    /// cell state: with the pointer `Some`, the component adds its content
    /// (symbols, styles, modifiers) to the frame; with the pointer `None`
    /// (component still mounted), the frame is identical to the App-only
    /// frame (zero cells contributed).
    #[test]
    fn mounted_but_inactive_destination_paints_nothing() {
        fn frame_cells(model: &mut Model, render_component: bool) -> String {
            let backend = ratatui::backend::TestBackend::new(120, 40);
            let mut term = ratatui::Terminal::new(backend).unwrap();
            term.draw(|f| {
                model.app.compose_base_frame(f, None);
                if render_component {
                    model.render_emby_browser_component(f);
                }
            })
            .unwrap();
            let buffer = term.backend().buffer();
            let mut out = String::new();
            for y in 0..buffer.area().height {
                for x in 0..buffer.area().width {
                    let cell = &buffer[(x, y)];
                    // Complete cell state: symbol + style (which carries the
                    // full style including any add/remove modifiers), so a
                    // render that changed only styling/attributes is caught.
                    out.push_str(cell.symbol());
                    out.push('|');
                    out.push_str(&format!("{:?}", cell.style()));
                    out.push(';');
                }
            }
            out
        }

        let mut model = Model::new(make_movie_app());
        model.app.tab = TabSelection::EmbyLibrary(0);
        model.app.panel_focus = PanelFocus::Library;
        model.app.panel_mode = PanelMode::Both;
        model.sync_emby_browser();
        let id = model.emby_browser_id.clone().expect("browser mounted");
        assert!(model.application.mounted(&id));

        // Baseline: the App-only frame (the component is not rendered).
        let app_only = frame_cells(&mut model, false);
        // Active: the component paints its content over the frame. This must
        // differ from app-only — otherwise the test could not discriminate a
        // broken gate.
        let active = frame_cells(&mut model, true);
        assert_ne!(
            active, app_only,
            "the active browser must add content to the frame (gate sanity)"
        );

        // Inactive: clear the pointer (narrow/drill transition). The
        // component stays mounted (keep-mounted) but the render gate must
        // suppress it, so the frame is identical to App-only across the full
        // cell state.
        model.emby_browser_id = None;
        let inactive = frame_cells(&mut model, true);
        assert_eq!(
            inactive, app_only,
            "a mounted-but-inactive destination must paint nothing over the frame"
        );
        assert_eq!(model.emby_browser_id, None);
        assert!(
            model.application.mounted(&id),
            "the component stays mounted but inactive"
        );
    }
}
