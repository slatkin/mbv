//! Home sync/render methods for the shell `Model` (design D2/D9, task 3.4).
//!
//! `HomeComponent` mounts for the session (active destination on the Home
//! tab); content/focus/nerd-fonts are pushed event-driven at their writers
//! (`push_home_content`, task 5.3d); cursor/section/scroll stay local.

use std::time::Instant;

use super::components::msg::HomeHitRegion;
use super::components::{ComponentId, HomeComponent, ShellRequest};
use super::shell::Model;
use super::{PanelFocus, TabSelection};
use mbv_core::playback_queue::QueueItem;

impl Model {
    /// Route the Home typed effects (task 5.3d, Home typed-effect prep) to
    /// their `App` handlers with the component-provided target index.
    /// `HomeComponent` owns the cursor; the effect must act on the requested
    /// target directly — never by copying it into a (now deleted) App flat
    /// cursor and re-reading that field. `HomeToggleWatched` carries no index: it
    /// targets the Continue Watching column's own cursor (`continue_cursor`),
    /// matching the legacy `cw_toggle_watched` (preserved, not fixed).
    pub(super) fn handle_home_request(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::HomePlay(cursor) => self.app.home_play(cursor),
            ShellRequest::HomeEnqueue(cursor) => self.app.home_enqueue(cursor),
            // Delete / watched-toggle refetch Home: re-project (5.3d).
            ShellRequest::HomeDelete(cursor) => {
                self.app.home_delete(cursor);
                self.push_home_content();
            }
            ShellRequest::HomeToggleWatched => {
                self.app.cw_toggle_watched();
                self.push_home_content();
            }
            ShellRequest::HomeSectionSelected(section) => {
                self.select_home_section_from_component(section)
            }
            _ => {}
        }
    }

    /// Resolve a selected section's semantic source from the mounted
    /// `HomeComponent` and persist it (task 5.3d, numeric Home section
    /// deletion). The component has already moved its own numeric section by
    /// the time a keyboard `[`/`]` / `HomeSectionSelected` or a pill click /
    /// `HomeClick::Pill` reaches the shell; the shell never copies that
    /// numeric index back into App — it maps the requested section to its
    /// `HomeLatestSource` via the component and stores that in the shell-
    /// owned semantic preference, then persists through the unchanged
    /// `App::save_prefs`. Continue Watching (section 0) resolves to `None`
    /// (the empty-string persistence sentinel). A missing component is a
    /// defensive no-op (Home is mounted for the whole session).
    fn select_home_section_from_component(&mut self, section: usize) {
        let Some(source) = self
            .application
            .get_component(&ComponentId::Home)
            .and_then(|c| c.as_any().downcast_ref::<HomeComponent>())
            .map(|home| home.source_for_section(section))
        else {
            return;
        };
        self.app.home_section_pref_semantic = source;
        // Persist the selection so the pill is restored on the next launch.
        self.app.save_prefs();
    }

    /// The authoritative "is Continue Watching selected?" fact for the
    /// context-menu builder, resolved at the Model boundary from the mounted
    /// `HomeComponent` (task 5.3d, Home context-menu section decoupling).
    /// Reading `HomeComponent::section() == 0` here replaces the deleted
    /// numeric `App.home.section == 0` read; the
    /// value is passed into the App-owned builder and never copied into a
    /// new App field or boolean mirror. With no mounted Home component the
    /// fact defaults to `false` (the component is mounted for the whole
    /// session, so this is only a defensive fallback).
    pub(super) fn home_continue_watching_selected(&self) -> bool {
        self.application
            .get_component(&ComponentId::Home)
            .and_then(|c| c.as_any().downcast_ref::<HomeComponent>())
            .map(HomeComponent::section)
            .map(|section| section == 0)
            .unwrap_or(false)
    }

    /// Route the Home click family (task 5.3d, Home mouse-click handoff) at
    /// the Model boundary. `HomeComponent` owns the hit geometry and has
    /// already moved its local cursor/section before emitting the region;
    /// the shell finishes only the cross-boundary side of each gesture:
    ///
    /// - `Pill` — persist the section preference through the existing
    ///   section-selection boundary (`select_home_section_from_component`,
    ///   which maps the clicked pill to its `HomeLatestSource` in the
    ///   component and calls `save_prefs`) and mark the click timestamp so a
    ///   follow-up row click isn't misread.
    /// - `Row` single click — focus the Library panel, as a Home click does
    ///   today. The clicked row is **not** copied into an App flat cursor;
    ///   the component owns the cursor, and App's independent Continue
    ///   Watching / per-latest cursors are left untouched.
    /// - `Row` double click — focus the Library panel and activate the
    ///   component-provided flat target directly via `home_play(target)`.
    /// - `ContextMenu` — focus the Library panel and open the menu at the
    ///   pointer, preserving the existing eligibility, menu actions, and
    ///   `continue_cursor` target semantics (the Home menu target is not the
    ///   clicked flat row, so no cursor copy is required).
    ///
    /// Double-click/scroll timing stays `App`-owned
    /// (`note_browse_double_click` against `last_click_time`/`last_click_pos`
    /// and the wheel throttle), so the 400ms window is preserved.
    pub(super) fn handle_home_click(&mut self, region: HomeHitRegion, col: u16, row: u16) {
        match region {
            HomeHitRegion::Pill(target) => {
                self.app.last_click_time = Instant::now();
                self.app.last_click_pos = (col, row);
                self.select_home_section_from_component(target);
            }
            HomeHitRegion::ContextMenu(_target) => {
                self.app.set_panel_focus(PanelFocus::Library);
                self.app
                    .open_context_menu_at(col, row, self.home_continue_watching_selected());
                // Right/row clicks focus Library: re-project (5.3d).
                self.push_home_content();
            }
            HomeHitRegion::Row(target) => {
                self.app.set_panel_focus(PanelFocus::Library);
                if self.app.note_browse_double_click(col, row) {
                    self.app.home_play(target);
                }
                self.push_home_content();
            }
        }
    }

    /// Route the Home wheel scroll (task 5.3d, Home wheel-scroll ownership)
    /// at the Model boundary, keeping the legacy gate order and quirk:
    /// `App`'s 30ms wheel throttle (`note_browse_scroll`) and browse-
    /// readiness gate (`browse_mouse_ready`) accept the event first; only
    /// then does the mounted `HomeComponent` move its section-local cursor
    /// with the same clamped semantics as its keyboard navigation
    /// (`move_local_cursor`), and the independent Continue Watching column's
    /// `continue_cursor` follows through `App::cw_move_cursor` — the
    /// pre-existing quirk the migration preserves rather than fixes.
    pub(super) fn handle_home_scroll(&mut self, delta: i64) {
        if !self.app.note_browse_scroll() {
            return;
        }
        if !self.app.browse_mouse_ready() {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&ComponentId::Home) {
            if let Some(home) = comp.as_any_mut().downcast_mut::<HomeComponent>() {
                home.move_local_cursor(delta);
            }
        }
        // Preserve the Continue Watching column quirk: the legacy wheel scroll
        // also moved the column's independent `continue_cursor`, which is not
        // the Home component's flat cursor mirror.
        self.app.cw_move_cursor(delta);
    }

    /// Mount `HomeComponent` for the session. Called once from `Model::new`;
    /// never unmounted (Home is always available, matching `App.tab`
    /// defaulting to `TabSelection::Home`).
    pub(super) fn mount_home(&mut self) {
        self.application
            .mount(ComponentId::Home, Box::new(HomeComponent::new()), vec![])
            .expect("mount Home");
    }

    /// Event-scoped projection replacing the deleted per-frame `sync_home`
    /// (task 5.3d): runs only at the writers of Home's projected inputs —
    /// content/`home_loading` and panel-focus writers; deterministic in
    /// `App` state, so duplicate pushes are idempotent. Applies the one-time
    /// persisted-pill restore (`home_section_pending` via `restore_section`
    /// once a matching section exists), then reconciles
    /// `home_section_pref_semantic` post-clamp (`[`/`]`/pill selection
    /// already reaches the shell as typed `HomeSectionSelected`).
    pub(super) fn push_home_content(&mut self) {
        let continue_items: Vec<QueueItem> = self
            .app
            .home
            .continue_items
            .iter()
            .cloned()
            .map(|item| QueueItem::Emby(Box::new(item)))
            .collect();
        let latest = self
            .app
            .home
            .latest
            .iter()
            .map(|(title, source, items, _cursor)| (title.clone(), source.clone(), items.clone()))
            .collect();
        let focused = !matches!(self.app.effective_panel_focus(), PanelFocus::Queue);
        let use_nerd_fonts = self.app.use_nerd_fonts;
        // Snapshot the pending persisted-pill restore before the component
        // borrow so the source identity is stable; arriving sources are
        // applied by `restore_section` only once a matching section exists.
        let pending = self.app.home_section_pending.clone();
        if let Some(comp) = self.application.get_component_mut(&ComponentId::Home) {
            if let Some(home) = comp.as_any_mut().downcast_mut::<HomeComponent>() {
                home.set_content(continue_items, latest, self.app.home_loading);
                home.set_focused(focused);
                home.set_use_nerd_fonts(use_nerd_fonts);
                if let Some(pending_source) = &pending {
                    if home.restore_section(pending_source) {
                        // Successful restore retains the pending source and
                        // clears the marker; the semantic preference is then
                        // reconciled from the component below.
                        self.app.home_section_pending = None;
                    }
                }
            }
        }
        // Reconcile the shell-owned semantic persistence identity from the
        // component only while no one-time restore remains pending: with the
        // numeric section owned by the component, this keeps
        // `home_section_pref_semantic` tracking the clamped selection across
        // async content rebuilds — but never while a pending absent source
        // must be retained (that would clear it to Continue Watching before
        // restoration). A successful restore above clears pending first, so
        // the reconcile then records the restored source.
        if self.app.home_section_pending.is_none() {
            let source = self
                .application
                .get_component(&ComponentId::Home)
                .and_then(|c| c.as_any().downcast_ref::<HomeComponent>())
                .map(|home| home.source_for_section(home.section()))
                .unwrap_or_else(|| self.app.home_section_pref_semantic.clone());
            self.app.home_section_pref_semantic = source;
        }
    }

    /// Paint `HomeComponent`'s `view()` over the legacy frame when Home is
    /// the active destination, then paint the cover image it computed but
    /// couldn't paint itself (no image-cache authority in the component).
    pub(super) fn render_home_component(&mut self, f: &mut ratatui::Frame) {
        if !matches!(self.app.tab, TabSelection::Home) {
            return;
        }
        let area = self.app.layout.main.home_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(&ComponentId::Home, f, area);
        let image_paint = self
            .application
            .get_component_mut(&ComponentId::Home)
            .and_then(|comp| comp.as_any_mut().downcast_mut::<HomeComponent>())
            .and_then(|home| home.take_image_paint());
        self.app.paint_home_image(f, image_paint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::msg::{LegacyTerminalEvent, Msg};
    use crate::app::tests::{make_app_stub, make_item, make_items};
    use std::time::Duration;
    use tuirealm::component::AppComponent;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    /// Task 5.3d, Home legacy underpaint removal + numeric section deletion:
    /// the one-time persisted-pill restore that used to run in the deleted
    /// legacy `App::render_home_list` now runs on the shell's `push_home_content`
    /// path. It restores the section via `HomeComponent::restore_section` only
    /// once a section with the pending source identity exists (sections
    /// arrive asynchronously), keeps the preference pending until then (an
    /// unrelated save must retain it), clears the pending when applied, and
    /// reconciles the shell-owned semantic preference from the component. No
    /// numeric section is mirrored back into App.
    #[test]
    fn shell_push_home_restores_persisted_home_section_and_clears_pending() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut model = Model::new(make_app_stub());
        // Simulate real startup: both the semantic preference and the pending
        // marker carry the saved source identity.
        model.app.home_section_pref_semantic =
            Some(crate::app::types_playback::HomeLatestSource::Audiobookshelf("books".into()));
        model.app.home_section_pending =
            Some(crate::app::types_playback::HomeLatestSource::Audiobookshelf("books".into()));

        // No matching source yet: the preference stays pending, the semantic
        // identity is retained (not clobbered to Continue Watching), and the
        // component section stays at its default (Continue Watching).
        model.push_home_content();
        {
            let home = model
                .application
                .get_component(&ComponentId::Home)
                .expect("Home component mounted")
                .as_any()
                .downcast_ref::<HomeComponent>()
                .expect("Home component type");
            assert_eq!(
                home.section(),
                0,
                "pending must not apply before the section exists"
            );
        }
        assert_eq!(
            model.app.home_section_pending,
            Some(crate::app::types_playback::HomeLatestSource::Audiobookshelf("books".into())),
            "preference must stay pending while the matching section is absent"
        );
        assert_eq!(
            model.app.home_section_pref(),
            "abs:books",
            "an absent pending source must be retained for an unrelated save"
        );

        // The "books" section arrives; the next sync restores it into the
        // component (section 1), clears the pending, and the reconcile
        // records the restored source in the semantic preference.
        model.app.home.latest = vec![(
            "Books".into(),
            crate::app::types_playback::HomeLatestSource::Audiobookshelf("books".into()),
            vec![],
            0,
        )];
        model.push_home_content();
        {
            let home = model
                .application
                .get_component(&ComponentId::Home)
                .expect("Home component mounted")
                .as_any()
                .downcast_ref::<HomeComponent>()
                .expect("Home component type");
            assert_eq!(
                home.section(),
                1,
                "restored section must land in the component"
            );
        }
        assert_eq!(
            model.app.home_section_pref(),
            "abs:books",
            "restored identity must be reflected in the persisted semantic preference"
        );
        assert_eq!(
            model.app.home_section_pending, None,
            "pending must be cleared once restored"
        );
    }

    #[test]
    fn shell_sync_keeps_home_component_cursor_local() {
        let mut model = Model::new(make_app_stub());
        model.app.home.continue_items = vec![
            crate::app::tests::make_item("one", "Movie"),
            crate::app::tests::make_item("two", "Movie"),
        ];
        model.push_home_content();

        let message = {
            let component = model
                .application
                .get_component_mut(&ComponentId::Home)
                .expect("Home component mounted")
                .as_any_mut()
                .downcast_mut::<HomeComponent>()
                .expect("Home component type");
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }))
        };
        assert_eq!(message, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));

        model.push_home_content();

        let component = model
            .application
            .get_component(&ComponentId::Home)
            .expect("Home component mounted")
            .as_any()
            .downcast_ref::<HomeComponent>()
            .expect("Home component type");
        assert_eq!(component.cursor(), 1);
    }

    /// Task 5.3d, Home typed-effect prep + cursor deletion: the shell routes
    /// each typed Home effect to its `App` handler with the component's
    /// target, and the effect acts on that supplied target even when App's
    /// remaining state (`continue_cursor`) points elsewhere. Enqueue is
    /// proven by the queued item's id; the section preference by the semantic
    /// preference persisted at the Model boundary. The emby-gated
    /// effects prove their target by acting at all: absent a live Emby
    /// service they flash "Emby is unavailable", while a non-CW flat target
    /// (play on the folder, delete past the CW range) would skip silently.
    #[test]
    fn shell_home_effects_honor_component_target() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut model = Model::new(make_app_stub());
        // Three Continue Watching rows (ids id0..id2) plus one latest pill
        // holding a folder, so a flat target past the CW range makes
        // `home_play` return early via the folder guard — a clean "wrong
        // target" signal. `continue_cursor` is parked on a different CW
        // column row so the effects must act on their explicit target, not
        // that parked state.
        model.app.home.continue_items = make_items(3);
        let mut folder = make_item("folder", "CollectionFolder");
        folder.is_folder = true;
        model.app.home.latest = vec![(
            "Folder".into(),
            crate::app::types_playback::HomeLatestSource::Emby("lib".into()),
            vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(folder))],
            0,
        )];
        model.push_home_content();

        // HomeEnqueue: the requested CW row (id2) is queued, not row 0.
        model.handle_home_request(ShellRequest::HomeEnqueue(2));
        let queued = model.app.player_tab.emby_items();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, "id2");

        // HomePlay: the requested CW row (id0) is resumed — the emby-gated
        // resume flashes "Emby is unavailable" — while a folder-flat target
        // would return early (folder guard).
        model.app.status.clear();
        model.handle_home_request(ShellRequest::HomePlay(0));
        assert_eq!(
            model.app.status, "Emby is unavailable",
            "play must act on the supplied CW target, not skip on the parked section state"
        );

        // HomeDelete: the requested CW row (0, in range) is removed — again
        // the emby-gated removal flashes — while an out-of-range flat target
        // would be skipped by the delete guard.
        model.app.status.clear();
        model.handle_home_request(ShellRequest::HomeDelete(0));
        assert_eq!(
            model.app.status, "Emby is unavailable",
            "delete must act on the supplied CW target, not skip on an out-of-range target"
        );

        // HomeToggleWatched: carries no index; it targets the Continue
        // Watching column's own cursor. Park `continue_cursor` on row 1 while
        // the emby-gated effect still acts (flashes unavailable) rather than
        // skipping.
        model.app.status.clear();
        model.app.home.continue_cursor = 1;
        model.handle_home_request(ShellRequest::HomeToggleWatched);
        assert_eq!(
            model.app.status, "Emby is unavailable",
            "toggle must act on the continue_cursor target, not skip on parked state"
        );

        // HomeSectionSelected: the requested pill index is mapped to its
        // semantic source in the component and persisted, even though it is
        // supplied explicitly (App holds no numeric section to read).
        model.handle_home_request(ShellRequest::HomeSectionSelected(1));
        assert_eq!(
            model.app.home_section_pref(),
            "emby:lib",
            "section preference must be the requested pill's source"
        );
    }

    /// Task 5.3d, numeric Home section deletion: explicit pill selection at
    /// the Model boundary persists the selected section's semantic
    /// `HomeLatestSource` (or `None`/empty for Continue Watching section 0),
    /// never a numeric index — resolved through the mounted component's
    /// `source_for_section`, driven via `HomeSectionSelected`.
    #[test]
    fn shell_home_section_selection_persists_semantic_source() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut model = Model::new(make_app_stub());
        model.app.home.latest = vec![
            (
                "Movies".into(),
                crate::app::types_playback::HomeLatestSource::Emby("lib-movies".into()),
                vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(
                    make_item("Movie one", "Movie"),
                ))],
                0,
            ),
            (
                "Podcasts".into(),
                crate::app::types_playback::HomeLatestSource::Audiobookshelf("abs-pod".into()),
                vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(
                    make_item("Episode one", "Episode"),
                ))],
                0,
            ),
        ];
        model.push_home_content();

        // Continue Watching (section 0) persists as the empty sentinel, never
        // as a `latest` pill's key.
        model.handle_home_request(ShellRequest::HomeSectionSelected(0));
        assert!(
            model.app.home_section_pref().is_empty(),
            "Continue Watching persists as no section key"
        );

        // Real pills persist their own keys (off-by-one: section 1 == latest[0]).
        model.handle_home_request(ShellRequest::HomeSectionSelected(1));
        assert_eq!(model.app.home_section_pref(), "emby:lib-movies");
        model.handle_home_request(ShellRequest::HomeSectionSelected(2));
        assert_eq!(model.app.home_section_pref(), "abs:abs-pod");
    }

    /// Task 5.3d, numeric Home section deletion: after a Home source is
    /// selected through the real component/Model-boundary path, an unrelated
    /// `save_prefs()` retains that semantic identity on disk — there is no
    /// numeric App section to clobber it (the selection wrote
    /// `home_section_pref_semantic`).
    #[test]
    fn shell_home_unrelated_save_retains_selected_source() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut model = Model::new(make_app_stub());
        model.app.home.latest = vec![(
            "Movies".into(),
            crate::app::types_playback::HomeLatestSource::Emby("lib-movies".into()),
            vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(
                make_item("Movie one", "Movie"),
            ))],
            0,
        )];
        model.push_home_content();
        model.handle_home_request(ShellRequest::HomeSectionSelected(1));

        // An unrelated preference save persists the retained semantic source.
        model.app.save_prefs();
        let saved = crate::config::prefs_path();
        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(saved).expect("prefs written")).unwrap();
        assert_eq!(
            parsed["home_section"], "emby:lib-movies",
            "unrelated save must keep the selected Home source identity"
        );
    }

    /// Task 5.3d, Home mouse-click handoff: the shell routes each typed
    /// `HomeClick` region at the Model boundary. A pill persists the section
    /// through the existing section-selection mechanism; a single row click
    /// focuses the Library panel but does **not** mutate App's independent
    /// Continue Watching `continue_cursor` or the per-latest pill cursors
    /// (the component owns the flat cursor); a double click additionally
    /// activates the component-provided flat target; a right-click focuses
    /// Library and opens a Pointer-anchored context menu whose target stays
    /// the Continue Watching `continue_cursor` item — not the clicked row.
    #[test]
    fn shell_home_click_family_routes_at_model_boundary() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut model = Model::new(make_app_stub());
        // Two Continue Watching rows (movies, ids id0/id1) plus one latest
        // folder so a double-click on a CW row provably differs from the
        // non-CW folder target (play on the folder skips silently via the
        // folder guard).
        model.app.home.continue_items = make_items(2);
        let mut folder = make_item("folder", "CollectionFolder");
        folder.is_folder = true;
        model.app.home.latest = vec![(
            "Folder".into(),
            crate::app::types_playback::HomeLatestSource::Emby("lib".into()),
            vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(folder))],
            0,
        )];
        model.push_home_content();

        // Single click on CW row 0: focuses the Library panel, but does not
        // mutate App's independent Continue Watching column cursor or the
        // per-latest pill cursor, and does not activate.
        model.app.home.continue_cursor = 1;
        model.app.home.latest[0].3 = 7;
        model.app.status.clear();
        model.handle_home_click(HomeHitRegion::Row(0), 5, 5);
        assert_eq!(
            model.app.effective_panel_focus(),
            PanelFocus::Library,
            "single click must focus the Library panel"
        );
        assert_eq!(
            model.app.home.continue_cursor, 1,
            "single click must not mutate the Continue Watching column cursor"
        );
        assert_eq!(
            model.app.home.latest[0].3, 7,
            "single click must not mutate the per-latest pill cursor"
        );
        assert!(
            model.app.status.is_empty(),
            "single click must not activate"
        );

        // Double click on CW row 0 (same coords within the App-owned 400ms
        // window): activates the flat target via home_play, which flashes on
        // the missing Emby service — proving it acted on the clicked CW
        // target, not the non-CW folder.
        model.handle_home_click(HomeHitRegion::Row(0), 5, 5);
        assert_eq!(
            model.app.status, "Emby is unavailable",
            "double click must activate the clicked flat target"
        );

        // Pill click: the clicked pill's semantic source is persisted via the
        // Model-boundary selection (`select_home_section_from_component`).
        model.handle_home_click(HomeHitRegion::Pill(1), 6, 6);
        assert_eq!(
            model.app.home_section_pref(),
            "emby:lib",
            "pill click must persist the clicked pill's source"
        );

        // Right-click: focuses Library and opens a Pointer-anchored context
        // menu whose entries resolve from the Continue Watching
        // `continue_cursor` item — a CW movie, not the folder. (The Home menu
        // target is continue_cursor, never the clicked row, so preserving it
        // needs no cursor copy.)
        model.app.home.continue_cursor = 0;
        model.handle_home_click(HomeHitRegion::ContextMenu(0), 70, 20);
        let Some(crate::app::types_overlay::OverlayRequest::ContextMenu(ref menu)) =
            model.app.pending_overlay
        else {
            panic!("right-click must open a context menu");
        };
        assert_eq!(
            menu.anchor,
            crate::app::types_context_menu::ContextMenuAnchor::Pointer { x: 70, y: 20 },
            "right-click must keep the pointer anchor"
        );
        assert!(
            menu.entries
                .iter()
                .any(|e| e.label == "Remove from Continue Watching"),
            "menu target must be the Continue Watching item, not the clicked folder"
        );

        // Task 5.3d, Home context-menu section decoupling: the authoritative
        // "is Continue Watching selected?" fact comes from the mounted
        // `HomeComponent` at the Model boundary (`home_continue_watching_selected`),
        // and it is load-bearing in the odd Queue-focus coupling — the queue
        // context menu shows "Remove from Continue Watching" iff the Home
        // component has Continue Watching selected. There is no App numeric
        // section to fall back on, so the menu follows the component.
        model.push_home_content();
        {
            let home = model
                .application
                .get_component_mut(&ComponentId::Home)
                .expect("Home component mounted")
                .as_any_mut()
                .downcast_mut::<HomeComponent>()
                .expect("Home component type");
            assert!(
                home.restore_section(&crate::app::types_playback::HomeLatestSource::Emby(
                    "lib".into()
                )),
                "restore to the Folder section must succeed"
            );
        }
        assert!(
            !model.home_continue_watching_selected(),
            "resolver must report the component's non-CW section"
        );

        // Keyboard '.' path under Queue panel focus while Home is the active
        // Tab selection: the shared `handle_global_view_key` front door
        // (reached through the CONTEXT_STACK from the Model-boundary
        // `handle_key_with_home_context`) opens the queue menu, and the odd
        // coupling entry is present iff the mounted Home component has
        // Continue Watching selected. With the component on a non-CW section
        // the entry must be absent — the menu follows the component.
        model.app.player_tab.set_queue_items(
            vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(
                make_item("Queued", "Movie"),
            ))],
            0,
        );
        model.app.panel_focus = PanelFocus::Queue;
        model.app.pending_overlay = None;
        assert!(
            !model.app.handle_key_with_home_context(
                crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char('.'),
                    crossterm::event::KeyModifiers::NONE,
                ),
                model.home_continue_watching_selected(),
            ),
            "'.' must not quit"
        );
        let Some(crate::app::types_overlay::OverlayRequest::ContextMenu(ref menu_non_cw)) =
            model.app.pending_overlay
        else {
            panic!("keyboard '.' must open a context menu under Queue focus");
        };
        assert!(
            !menu_non_cw
                .entries
                .iter()
                .any(|e| e.label == "Remove from Continue Watching"),
            "component on a non-CW section must drop the keyboard-menu CW entry"
        );
        assert!(
            matches!(model.app.effective_panel_focus(), PanelFocus::Queue),
            "keyboard '.' must not change the Queue panel focus"
        );

        // Put the component back on Continue Watching (empty latest clamps the
        // component section back to section 0): the same keyboard '.' path
        // shows the entry again.
        {
            let home = model
                .application
                .get_component_mut(&ComponentId::Home)
                .expect("Home component mounted")
                .as_any_mut()
                .downcast_mut::<HomeComponent>()
                .expect("Home component type");
            home.set_content(vec![], vec![], false);
        }
        assert!(
            model.home_continue_watching_selected(),
            "resolver must report CW when the component is back on section 0"
        );
        model.app.pending_overlay = None;
        assert!(
            !model.app.handle_key_with_home_context(
                crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char('.'),
                    crossterm::event::KeyModifiers::NONE,
                ),
                model.home_continue_watching_selected(),
            ),
            "'.' must not quit"
        );
        let Some(crate::app::types_overlay::OverlayRequest::ContextMenu(ref menu_cw)) =
            model.app.pending_overlay
        else {
            panic!("keyboard '.' must open a context menu under Queue focus");
        };
        assert!(
            menu_cw
                .entries
                .iter()
                .any(|e| e.label == "Remove from Continue Watching"),
            "component back on CW must show the keyboard-menu CW entry"
        );
    }

    fn home_component_cursor(model: &Model) -> usize {
        model
            .application
            .get_component(&ComponentId::Home)
            .expect("Home component mounted")
            .as_any()
            .downcast_ref::<HomeComponent>()
            .expect("Home component type")
            .cursor()
    }

    /// Task 5.3d, Home wheel-scroll ownership: an accepted `HomeScroll`
    /// (App's 30ms wheel throttle and browse-readiness gate both pass) moves
    /// the mounted component's section-local cursor *and* the independent
    /// Continue Watching column cursor (`continue_cursor`, the preserved
    /// legacy quirk); a throttled event — the next wheel inside the App-owned
    /// 30ms window — moves neither.
    #[test]
    fn shell_home_wheel_moves_component_and_continue_cursor() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut model = Model::new(make_app_stub());
        model.app.home.continue_items = make_items(3);
        model.push_home_content();

        // Accepted scroll: both the component-local cursor and the Continue
        // Watching column's independent cursor advance by the delta. Pin the
        // wheel throttle far in the past so acceptance is deterministic
        // regardless of wall-clock execution time.
        model.app.last_scroll_at = Instant::now() - Duration::from_secs(1);
        model.handle_home_scroll(1);
        assert_eq!(
            model.app.home.continue_cursor, 1,
            "accepted wheel must move the Continue Watching column cursor"
        );
        assert_eq!(
            home_component_cursor(&model),
            1,
            "accepted wheel must move the component-local cursor"
        );

        // Throttled scroll: pin the wheel throttle into the future so the
        // next event is deterministically blocked inside App's 30ms window —
        // `duration_since` saturates to zero even if this thread is
        // descheduled past the window before the call — rather than relying
        // on the two calls landing <30ms apart; neither cursor moves.
        model.app.last_scroll_at = Instant::now() + Duration::from_secs(1);
        model.handle_home_scroll(1);
        assert_eq!(
            model.app.home.continue_cursor, 1,
            "throttled wheel must not move the Continue Watching column cursor"
        );
        assert_eq!(
            home_component_cursor(&model),
            1,
            "throttled wheel must not move the component-local cursor"
        );
    }
}
