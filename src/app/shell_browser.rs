use super::components::{BrowserComponent, BrowserKey, BrowserKind, ComponentId, ShellRequest};
use super::shell::Model;
use super::{ConfirmAction, ConfirmModal, PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    /// Route the generic Emby browser's selected-item typed effects (task
    /// 5.3d, Emby browser effect decoupling) to their `App` handlers with the
    /// component-resolved owned target. `BrowserComponent` resolves its own
    /// selected `EmbyItem` from its component-local cursor/content; the
    /// effect acts on that supplied item directly — never by copying the
    /// component cursor into a `BrowseLevel.cursor` and re-reading it. The
    /// active library index is derived from the shell's own tab state (the
    /// browser is mounted only for the active generic/Movies/home-video
    /// `EmbyLibrary` tab, same derivation as the `BrowserClick` mouse arms).
    /// A missing library index is a defensive no-op.
    pub(super) fn handle_browser_request(&mut self, request: ShellRequest) {
        let Some(lib_idx) = self.app.tab.emby_library_index() else {
            return;
        };
        match request {
            ShellRequest::BrowserActivate { item } => self.app.select_item(lib_idx, item),
            ShellRequest::BrowserPlay { item } => self.app.play_or_activate_lib_item(lib_idx, item),
            ShellRequest::BrowserEnqueue { item } => self.app.enqueue_lib_item(lib_idx, item),
            ShellRequest::BrowserToggleWatched { item } => {
                self.app.toggle_watched_item(lib_idx, item)
            }
            // '.' raises the context menu for the supplied item via the
            // existing item-targeted seam; the non-folder/mark-watched menu
            // content derives from the shell's own tab state (`lib_idx` just
            // guards that this is an EmbyLibrary tab), never a `BrowseLevel`
            // cursor re-read.
            ShellRequest::BrowserContextMenu { item } => self.app.open_context_menu_for(item),
            // Ctrl+S shuffles the supplied item with the preserved
            // `shuffle_play` tail: a folder item shuffles the folder itself;
            // a non-folder item shuffles the current browse level's parent
            // (falling back to the library id). The folder target comes from
            // the component-resolved item, never a `BrowseLevel.cursor`
            // re-read.
            ShellRequest::BrowserShuffle { item } => self.app.shuffle_play_selected(lib_idx, item),
            // Bare or Alt+`r` refreshes the active Emby library (task 5.3d,
            // Emby browser refresh): the shell derives the active library
            // index from its own tab state and runs `App::refresh_lib` on it,
            // the same call the legacy `handle_lib_key` `Char('r')` arm made.
            ShellRequest::BrowserRefresh => self.app.refresh_lib(lib_idx),
            // Ctrl+`r` raises the Rescan Library confirmation (task 5.3d,
            // Emby browser rescan): same title/message/hint and
            // `ConfirmAction::RescanLibrary(lib_idx)` as the legacy
            // `handle_lib_key` CONTROL arm, derived from the shell's own tab
            // state (the library name comes from the active library).
            ShellRequest::BrowserRescan => {
                let name = self.app.libs[lib_idx].library.name.clone();
                self.app.ask_confirm(ConfirmModal {
                    title: " Rescan Library ".into(),
                    message: format!("Rescan '{name}'?"),
                    hint: "[y] Confirm    [Esc] Cancel".into(),
                    on_confirm: ConfirmAction::RescanLibrary(lib_idx),
                });
            }
            // Esc/Backspace go back through the browse history (task 5.3d,
            // Emby browser back): the shell derives the active Emby library
            // index from its own tab state and runs `App::go_back` on it, the
            // same call the legacy `handle_lib_key` `Esc | Backspace` arm
            // made — preserving synthetic-group/root guards, parent-cursor
            // restoration, season-level skip, persistence, and stale-index
            // behavior.
            ShellRequest::BrowserBack => self.app.go_back(lib_idx),
            // `[`/`]` cycle the letter-range pill row (task 5.3d, Emby
            // browser selector cycling): the shell derives the active Emby
            // library index from its own tab state and runs
            // `App::cycle_letter_pill` on it, the same call the legacy
            // `handle_key_emby_library` arm made — preserving the
            // `should_show_letter_pills` no-op guard and the existing
            // wrap/select behavior (the component's mount gate has already
            // excluded the Music and feed-home-video group branches).
            ShellRequest::BrowserCycleLetterPill { delta } => {
                self.app.cycle_letter_pill(lib_idx, delta)
            }
            // Up/Down/k/j/PageUp/PageDown move the App cursor by display
            // rows (task 5.3d, Emby browser local navigation): the component
            // reports the row deltas it already applied to its own cursor
            // (Up/k -1, Down/j 1, PageUp/PageDown ±page_rows), and the
            // shell derives the active Emby library index from its own tab
            // state and runs `App::move_lib_cursor_rows` — the same call
            // the legacy `handle_lib_key` movement arms made — applying the
            // App's own painted column stride. Calling the App method, never
            // a raw cursor-field write, preserves `save_default_library_position` /
            // `mark_library_navigation` / `maybe_fetch_next_page` /
            // `last_nav_at` idle side effects byte-for-byte. The legacy
            // season-grid branch is unreachable here (the Browser mount
            // gate excludes TV).
            ShellRequest::BrowserMoveRows { rows } => self.app.move_lib_cursor_rows(lib_idx, rows),
            // Left/Right/h/l move the App cursor within a row on a
            // multi-column list (task 5.3d, Emby browser local navigation):
            // the component reports the column delta it already applied, and
            // the shell runs `App::move_lib_cursor` — the same call the
            // legacy `handle_lib_key` column arms made, with the same
            // navigation side effects as `BrowserMoveRows`.
            ShellRequest::BrowserMoveColumn { delta } => self.app.move_lib_cursor(lib_idx, delta),
            // Home/End jump the App cursor to the first/last item (task
            // 5.3d, Emby browser local navigation): the component reports
            // the jump direction it already applied, and the shell runs
            // `App::jump_lib_cursor` — the same call the legacy
            // `handle_lib_key` Home/End arms made, with the same navigation
            // side effects as `BrowserMoveRows`.
            ShellRequest::BrowserJumpCursor { to_end } => self.app.jump_lib_cursor(lib_idx, to_end),
            _ => {}
        }
    }
    fn emby_browser_component_id(&self) -> Option<ComponentId> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        if self.app.is_podcast_library(index) || self.app.is_feed_home_video_group_view(index) {
            return None;
        }
        let kind = BrowserKind::from_collection_type(&library.library.collection_type);
        if !matches!(
            kind,
            BrowserKind::Generic | BrowserKind::Movies | BrowserKind::HomeVideos
        ) {
            return None;
        }
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind,
        }))
    }

    /// Reconcile the mounted Emby browser against the currently-active Emby
    /// library tab (task 5.3d.15/M1 extraction from `sync_emby_browser`),
    /// idempotently. If the active id matches the gate (`emby_browser_component_id`)
    /// it does nothing. Mount lifecycle only; content projection and layout
    /// adapters stay in `sync_emby_browser`.
    pub(super) fn mount_emby_browser(&mut self) {
        let next_id = self.emby_browser_component_id();
        if self.emby_browser_id == next_id {
            return;
        }
        if let Some(id) = self.emby_browser_id.take() {
            let _ = self.application.umount(&id);
        }
        if let Some(id) = next_id.clone() {
            self.application
                .mount(id.clone(), Box::new(BrowserComponent::new()), vec![])
                .expect("mount Emby browser");
            self.application.active(&id).expect("activate Emby browser");
            self.emby_browser_id = Some(id);
        }
    }

    /// Event-driven content projection for the mounted Emby browser (task
    /// 5.3d.15/M2): the per-frame `sync_emby_browser` no longer rewrites
    /// content every loop pass. This mirror applies the current library
    /// `render_ctx`, cursor and panel-focus flag idempotently, and is called
    /// at every writer seam that can change the active Emby library (the same
    /// seams that re-project Home). `set_wide_movies` is NOT here — the wide
    /// Movies rail stride is a per-draw layout fact pushed in
    /// `render_emby_browser_component` (D18 step 1).
    pub(super) fn push_emby_browser_content(&mut self) {
        let Some(id) = self.emby_browser_id.as_ref() else {
            return;
        };
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let context = self.app.library_list_render_ctx(index, false);
        let focused = matches!(self.app.effective_panel_focus(), PanelFocus::Library);
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(browser) = comp.as_any_mut().downcast_mut::<BrowserComponent>() {
                browser.set_content(context, focused);
            }
        }
    }

    /// Legacy per-frame entry point (task 5.3d.15/M2): mount + content
    /// projection only. Kept for test compatibility; the live event loop
    /// still calls it once per loop pass, and the wide-Movies adapter now
    /// rides the per-draw render path.
    pub(super) fn sync_emby_browser(&mut self) {
        self.mount_emby_browser();
        self.push_emby_browser_content();
    }

    pub(super) fn render_emby_browser_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.emby_browser_id.as_ref() else {
            return;
        };
        // Task 5.3d.17a/17b: when the wide Movies/home-video layout is active
        // the component paints the full hero-on-left rect, so hand it the full
        // library area (`movies_wide_area`, published by the App render path
        // this frame); otherwise hand it the narrow inner list area.
        let wide = self.app.layout.main.is_wide_movies_active();
        let area = if wide {
            self.app.layout.main.movies_wide_area
        } else {
            self.app.layout.main.left_area
        };
        if area.width == 0 || area.height == 0 {
            return;
        }
        // Per-draw adapter (D18 step 1): the legacy base frame (self.app.render(f))
        // has already populated movies_wide_right_area / movies_wide_area this
        // frame. The base frame and the mounted component share one paint, so
        // the 1-column right-rail stride (the only reader of this field) is
        // consistent here. `home_video`/`letter_pills` tell the component which
        // pill row to render in the wide right rail.
        let (home_video, letter_pills) = if wide {
            match self.app.tab.emby_library_index() {
                Some(lib_idx) => (
                    self.app.is_home_video_view(lib_idx),
                    self.app.should_show_letter_pills(lib_idx),
                ),
                None => (false, false),
            }
        } else {
            (false, false)
        };
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(browser) = comp.as_any_mut().downcast_mut::<BrowserComponent>() {
                browser.set_wide_movies(wide, home_video, letter_pills);
                browser.set_use_nerd_fonts(self.app.use_nerd_fonts);
            }
        }
        self.application.view(id, frame, area);
        // Paint the hero cover image the component computed but could not
        // paint itself (no image-cache authority), mirroring HomeComponent.
        // Also read back the scroll the component painted at, so it can be
        // persisted into the App nav level (task 5.3d.17b).
        let (image_paint, painted_scroll) = self
            .application
            .get_component_mut(id)
            .and_then(|comp| comp.as_any_mut().downcast_mut::<BrowserComponent>())
            .map(|browser| (browser.take_image_paint(), browser.scroll()))
            .unwrap_or((None, 0));
        self.app.paint_home_image(frame, image_paint);
        // Preserve the legacy wide-renderer scroll write-back: the component
        // owns its cursor/scroll and `view()` returns the rendered scroll to
        // `self.scroll`, but `set_content` overwrites that next frame from
        // the App nav level — so without this write-back the rendered scroll
        // would be lost on resize / first paint.
        if let Some(lib_idx) = self.app.tab.emby_library_index() {
            if let Some(level) = self.app.libs[lib_idx].nav_stack.last_mut() {
                level.scroll = painted_scroll;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{BrowserComponent, Msg};
    use crate::app::render::make_movie_app;
    use crate::app::tests::{make_app_stub, make_item, make_items};
    use crate::app::{App, BrowseLevel, ContextAction, LibraryTab, PanelMode, TabSelection};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    /// Task 5.3d, Emby browser effect decoupling: `BrowserComponent` resolves
    /// its own selected `EmbyItem` from its component-local cursor over the
    /// mirrored content, and the shell routes each typed effect to an `App`
    /// handler that acts on the supplied item directly (never by copying the
    /// component cursor into a `BrowseLevel.cursor` and re-reading it).
    ///
    /// The regression parks App's nav cursor on the folder at the top of the
    /// list while the component selects the playable movie below it — the
    /// legacy Enter/Ctrl+P/Ctrl+A/Ctrl+W/'.' arms on the parked folder would
    /// navigate into the folder, play the folder, enqueue the folder, toggle
    /// the folder, or raise the folder-scoped context menu respectively — and
    /// proves each of the five effects acts on the component-selected movie
    /// instead. Requests are captured from the mounted component itself, so
    /// the emitted payload is the component-resolved item; every assertion is
    /// on the effect's outcome (nav-stack depth/cursor, queued item id, the
    /// unavailable-Service toast, or the raised menu's actions), never on a
    /// hand-set coordinate.
    #[test]
    fn shell_emby_browser_effects_honor_component_target() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut model = Model::new(browser_app_with_folder_and_movie());
        model.sync_emby_browser();
        let id = model.emby_browser_id.clone().expect("browser mounted");

        // Drive the component cursor onto the movie (index 1) while App's
        // nav cursor stays parked on the folder (index 0). The movement key
        // now returns the typed rows request (task 5.3d, Emby browser local
        // navigation) — the component cursor still advances in place.
        assert!(matches!(
            drive_browser_key(&mut model, &id, Key::Down, KeyModifiers::NONE),
            Some(Msg::Shell(ShellRequest::BrowserMoveRows { rows: 1 }))
        ));

        // Enter: the component emits BrowserActivate for its own selected
        // movie; routed with App's cursor parked on the folder, the effect
        // activates the supplied movie (cursor jumps to it, nav stack does
        // NOT grow into the folder, and the emby-gated play flashes the
        // unavailable Service) instead of the legacy folder navigation.
        let Some(Msg::Shell(ShellRequest::BrowserActivate { item })) =
            drive_browser_key(&mut model, &id, Key::Enter, KeyModifiers::NONE)
        else {
            panic!("browser Enter must emit BrowserActivate, got no typed request");
        };
        assert_eq!(
            item.id, "movie-b",
            "component must resolve its own selection"
        );
        model.app.libs[0].nav_stack[0].cursor = 0;
        model.handle_browser_request(ShellRequest::BrowserActivate { item });
        assert_eq!(
            model.app.libs[0].nav_stack.len(),
            1,
            "playable activation must not navigate into the parked folder"
        );
        assert_eq!(
            model.app.libs[0].nav_stack[0].cursor, 1,
            "the effect must select the supplied movie, not the parked cursor"
        );
        assert_eq!(model.app.status, "Emby is unavailable");

        // Ctrl+P: non-folder activation of the supplied movie, again with
        // the App cursor re-parked on the folder — same decisive signals as
        // Enter (folder play would have diverted to `play_folder`).
        model.app.status.clear();
        let Some(Msg::Shell(ShellRequest::BrowserPlay { item })) =
            drive_browser_key(&mut model, &id, Key::Char('p'), KeyModifiers::CONTROL)
        else {
            panic!("browser Ctrl+P must emit BrowserPlay, got no typed request");
        };
        assert_eq!(item.id, "movie-b");
        model.app.libs[0].nav_stack[0].cursor = 0;
        model.handle_browser_request(ShellRequest::BrowserPlay { item });
        assert_eq!(model.app.libs[0].nav_stack.len(), 1);
        assert_eq!(model.app.libs[0].nav_stack[0].cursor, 1);
        assert_eq!(model.app.status, "Emby is unavailable");

        // Ctrl+A: the supplied movie (not the parked folder) is enqueued.
        model.app.status.clear();
        let Some(Msg::Shell(ShellRequest::BrowserEnqueue { item })) =
            drive_browser_key(&mut model, &id, Key::Char('a'), KeyModifiers::CONTROL)
        else {
            panic!("browser Ctrl+A must emit BrowserEnqueue, got no typed request");
        };
        assert_eq!(item.id, "movie-b");
        model.app.libs[0].nav_stack[0].cursor = 0;
        model.handle_browser_request(ShellRequest::BrowserEnqueue { item });
        let queued = model.app.player_tab.emby_items();
        assert_eq!(queued.len(), 1);
        assert_eq!(
            queued[0].id, "movie-b",
            "enqueue must queue the supplied movie, not the parked folder"
        );

        // Ctrl+W: the supplied movie is toggled (the emby-gated effect
        // flashes the unavailable Service) even though a legacy arm on the
        // parked folder would skip silently via the folder guard.
        model.app.status.clear();
        let Some(Msg::Shell(ShellRequest::BrowserToggleWatched { item })) =
            drive_browser_key(&mut model, &id, Key::Char('w'), KeyModifiers::CONTROL)
        else {
            panic!("browser Ctrl+W must emit BrowserToggleWatched, got no typed request");
        };
        assert_eq!(item.id, "movie-b");
        model.app.libs[0].nav_stack[0].cursor = 0;
        model.handle_browser_request(ShellRequest::BrowserToggleWatched { item });
        assert_eq!(
            model.app.status, "Emby is unavailable",
            "watched toggle must act on the supplied movie, not skip on the parked folder"
        );

        // '.': the component emits BrowserContextMenu for its own selected
        // movie; the shell raises the menu for that supplied item via
        // `open_context_menu_for`. Legacy resolution on the parked folder
        // would raise the folder-scoped menu (Play All/Shuffle/Add to Queue),
        // so the menu must offer the generic per-item Play and no folder
        // actions — decisive that the menu targets the component-selected
        // movie, not the parked `BrowseLevel` cursor.
        let Some(Msg::Shell(ShellRequest::BrowserContextMenu { item })) =
            drive_browser_key(&mut model, &id, Key::Char('.'), KeyModifiers::NONE)
        else {
            panic!("browser '.' must emit BrowserContextMenu, got no typed request");
        };
        assert_eq!(item.id, "movie-b");
        model.app.libs[0].nav_stack[0].cursor = 0;
        model.handle_browser_request(ShellRequest::BrowserContextMenu { item });
        let menu = match model.app.pending_overlay.as_ref() {
            Some(crate::app::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
            _ => panic!("context menu must open for the supplied movie"),
        };
        let actions: Vec<_> = menu
            .entries
            .iter()
            .filter_map(|e| e.action.clone())
            .collect();
        assert!(
            actions.iter().any(|a| matches!(a, ContextAction::Play)),
            "context menu must offer the generic per-item Play, got: {actions:?}"
        );
        assert!(
            !actions.iter().any(|a| matches!(
                a,
                ContextAction::PlayFolder(_)
                    | ContextAction::ShuffleFolder(_)
                    | ContextAction::EnqueueFolder(_)
            )),
            "context menu must target the supplied movie, not the parked folder, got: {actions:?}"
        );

        // Ctrl+S: the component emits BrowserShuffle carrying its own
        // selected movie — not the parked folder that a legacy `shuffle_play`
        // on the App cursor would have resolved. The shell's preserved
        // `shuffle_play` tail then takes the non-folder branch (current
        // browse-level parent) for the supplied movie; the emitted payload is
        // decisive that the component-local cursor selected the target.
        model.app.status.clear();
        let Some(Msg::Shell(ShellRequest::BrowserShuffle { item })) =
            drive_browser_key(&mut model, &id, Key::Char('s'), KeyModifiers::CONTROL)
        else {
            panic!("browser Ctrl+S must emit BrowserShuffle, got no typed request");
        };
        assert_eq!(
            item.id, "movie-b",
            "shuffle must carry the component-selected movie, not the parked BrowseLevel.cursor folder"
        );

        // Bare `r` refreshes the active Emby library (task 5.3d, Emby browser
        // refresh): the component emits `BrowserRefresh`, and the shell derives
        // the library index from its own tab state and runs `App::refresh_lib`,
        // which lifts the current nav level's `loading` flag.
        let Some(Msg::Shell(ShellRequest::BrowserRefresh)) =
            drive_browser_key(&mut model, &id, Key::Char('r'), KeyModifiers::NONE)
        else {
            panic!("browser bare r must emit BrowserRefresh, got no typed request");
        };
        model.handle_browser_request(ShellRequest::BrowserRefresh);
        assert!(
            model.app.libs[0].nav_stack[0].loading,
            "refresh must lift the active library nav level's loading flag"
        );

        // Legacy Alt+`r` preserves a bare-refresh, not a rescan: the CONTROL
        // arm is guarded by the CONTROL modifier, Alt does not set it, and the
        // bare `r` arm below it catches Alt+`r` — exactly the legacy
        // `handle_lib_key` ordering.
        let Some(Msg::Shell(ShellRequest::BrowserRefresh)) =
            drive_browser_key(&mut model, &id, Key::Char('r'), KeyModifiers::ALT)
        else {
            panic!("browser Alt+r must still emit BrowserRefresh, got no typed request");
        };

        // Ctrl+`r` raises the Rescan Library confirmation (task 5.3d, Emby
        // browser rescan): the component emits `BrowserRescan`, and the shell
        // raises the same confirm modal (title/message/hint and
        // `ConfirmAction::RescanLibrary(lib_idx)`) the legacy arm raised.
        let Some(Msg::Shell(ShellRequest::BrowserRescan)) =
            drive_browser_key(&mut model, &id, Key::Char('r'), KeyModifiers::CONTROL)
        else {
            panic!("browser Ctrl+r must emit BrowserRescan, got no typed request");
        };
        model.handle_browser_request(ShellRequest::BrowserRescan);
        match model.app.pending_overlay.as_ref() {
            Some(crate::app::types_overlay::OverlayRequest::Confirm(modal)) => {
                assert_eq!(modal.title, " Rescan Library ");
                assert!(matches!(
                    modal.on_confirm,
                    crate::app::ConfirmAction::RescanLibrary(0)
                ));
                assert_eq!(modal.message, "Rescan 'Movies'?");
            }
            _ => panic!("Ctrl+r must raise the Rescan Library confirmation"),
        }

        // Esc/Backspace move back-navigation off `Msg::Legacy` (task 5.3d,
        // Emby browser back): with the browser focused, both keys emit a
        // typed `BrowserBack` — not a raw legacy key — and the shell routes
        // it to `App::go_back`, which pops the child level and restores the
        // parent cursor to the folder the child came from. Drive the parent
        // cursor off the folder first so the restoration is observable.
        model.app.libs[0].nav_stack[0].cursor = 1;
        model.app.libs[0].nav_stack.push(BrowseLevel {
            parent_id: "folder-a".into(),
            title: "Folder A".into(),
            items: vec![],
            total_count: 0,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        });
        let Some(Msg::Shell(ShellRequest::BrowserBack)) =
            drive_browser_key(&mut model, &id, Key::Esc, KeyModifiers::NONE)
        else {
            panic!("focused browser Esc must emit BrowserBack, got no typed request");
        };
        model.handle_browser_request(ShellRequest::BrowserBack);
        assert_eq!(
            model.app.libs[0].nav_stack.len(),
            1,
            "BrowserBack must pop the child browse level via go_back"
        );
        assert_eq!(
            model.app.libs[0].nav_stack[0].cursor, 0,
            "go_back must restore the parent cursor to the folder the child came from"
        );

        // Backspace routes the same way (the legacy arm matched both keys
        // with no modifier guard).
        let Some(Msg::Shell(ShellRequest::BrowserBack)) =
            drive_browser_key(&mut model, &id, Key::Backspace, KeyModifiers::NONE)
        else {
            panic!("focused browser Backspace must emit BrowserBack, got no typed request");
        };

        // `[`/`]` cycle the letter-range pill row (task 5.3d, Emby browser
        // selector cycling): the focused browser emits a typed
        // `BrowserCycleLetterPill` carrying the delta — never a raw legacy
        // key — and the shell derives the library index from its own tab
        // state and runs `App::cycle_letter_pill`, whose select effect lands
        // on the top browse level. The fixture's Movies library already sits
        // at its top browse level, so capturing a true total is the only
        // missing `should_show_letter_pills` piece.
        model.app.libs[0].library_total = Some(1000);
        let Some(Msg::Shell(ShellRequest::BrowserCycleLetterPill { delta })) =
            drive_browser_key(&mut model, &id, Key::Char(']'), KeyModifiers::NONE)
        else {
            panic!("focused browser ] must emit BrowserCycleLetterPill, got no typed request");
        };
        assert_eq!(delta, 1, "']' must carry +1");
        model.handle_browser_request(ShellRequest::BrowserCycleLetterPill { delta });
        assert_eq!(
            model.app.libs[0].nav_stack[0]
                .letter_filter
                .as_ref()
                .map(|f| f.index),
            Some(1),
            "']' must advance from the default A\u{2013}C pill to the next bucket"
        );

        // `[` cycles back the other way (the default is bucket 0, so this
        // round-trips to it).
        let Some(Msg::Shell(ShellRequest::BrowserCycleLetterPill { delta })) =
            drive_browser_key(&mut model, &id, Key::Char('['), KeyModifiers::NONE)
        else {
            panic!("focused browser [ must emit BrowserCycleLetterPill, got no typed request");
        };
        assert_eq!(delta, -1, "'[' must carry -1");
        model.handle_browser_request(ShellRequest::BrowserCycleLetterPill { delta });
        assert_eq!(
            model.app.libs[0].nav_stack[0]
                .letter_filter
                .as_ref()
                .map(|f| f.index),
            Some(0),
            "'[' must cycle back to the A\u{2013}C pill"
        );

        // Ctrl/Alt brackets are NOT letter-pill cycling: the legacy guard
        // excluded CONTROL and ALT, so those combinations are unbound and
        // forwarded through the typed global bridge.
        assert!(matches!(
            drive_browser_key(&mut model, &id, Key::Char('['), KeyModifiers::CONTROL),
            Some(Msg::Shell(ShellRequest::GlobalViewKey(_)))
        ));
        assert!(matches!(
            drive_browser_key(&mut model, &id, Key::Char(']'), KeyModifiers::ALT),
            Some(Msg::Shell(ShellRequest::GlobalViewKey(_)))
        ));
    }

    /// Drive one key into the mounted `BrowserComponent` and return its `Msg`
    /// (test helper for the Model-boundary regression above).
    fn drive_browser_key(
        model: &mut Model,
        id: &ComponentId,
        key: Key,
        modifiers: KeyModifiers,
    ) -> Option<Msg> {
        model
            .application
            .get_component_mut(id)
            .expect("browser mounted")
            .on(&Event::Keyboard(KeyEvent {
                code: key,
                modifiers,
            }))
    }

    fn browser_app_with_folder_and_movie() -> App {
        let mut app = make_app_stub();
        app.tab = TabSelection::EmbyLibrary(0);

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let mut folder = make_item("Folder A", "CollectionFolder");
        folder.id = "folder-a".into();
        folder.is_folder = true;

        let mut movie = make_item("Movie B", "Movie");
        movie.id = "movie-b".into();

        app.libs.push(LibraryTab {
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![folder, movie],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
                music_grouping: None,
            }],
            ..LibraryTab::new(library)
        });

        app
    }

    #[test]
    fn shell_mounts_and_syncs_the_generic_emby_browser() {
        let mut model = Model::new(make_movie_app());
        model.sync_emby_browser();
        let id = model.emby_browser_id.clone().expect("browser mounted");
        let message = {
            model
                .application
                .get_component_mut(&id)
                .unwrap()
                .on(&Event::Keyboard(KeyEvent {
                    code: Key::Down,
                    modifiers: KeyModifiers::NONE,
                }))
        };
        // The focused browser's Down now routes through the typed rows
        // request (task 5.3d, Emby browser local navigation) instead of
        // forwarding the raw legacy key; the shell arm moves the App cursor
        // through `App::move_lib_cursor_rows` the way `handle_lib_key` did.
        let Some(Msg::Shell(ShellRequest::BrowserMoveRows { rows })) = message else {
            panic!("browser movement should emit the typed rows request");
        };
        assert_eq!(rows, 1, "Down must carry one display row");
        model.handle_browser_request(ShellRequest::BrowserMoveRows { rows });
        model.sync_emby_browser();
        assert_eq!(model.app.libs[0].nav_stack[0].cursor, 1);
        assert!(model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<BrowserComponent>()
            .is_some());
    }

    /// Task 5.3d, Emby browser local navigation through the Model boundary:
    /// the focused `BrowserComponent` returns typed `BrowserMoveRows` /
    /// `BrowserMoveColumn` / `BrowserJumpCursor` requests in place of the
    /// raw legacy key, and the shell derives the active Emby library index
    /// from its own tab state and runs the same `App` cursor methods the
    /// legacy `handle_lib_key` movement arms call. The App cursor must move
    /// through that typed path (never a raw cursor-field write): a
    /// two-column painted list strides the App cursor by the column count
    /// per row (Down +2), Home/End jump to the first/last item, and
    /// Left/Right move within the row; a one-column list keeps Left/Right/
    /// h/l unbound (raw key consumed by the component as `Msg::Legacy`
    /// `NoOp`, App cursor unchanged) while the row keys keep their typed
    /// stride of one.
    #[test]
    fn shell_emby_browser_movement_drives_app_cursor_via_typed_requests() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = browser_app_with_flat_movies(10);
        // LibraryOnly hides the queue column so the library pane spans the
        // full window and clears the two-column threshold at render width;
        // the panel mode is a state the app already supports, not hand-set
        // layout rects (the whole frame is painted into a TestBackend).
        app.panel_mode = PanelMode::LibraryOnly;
        let mut model = Model::new(app);
        model.sync_emby_browser();
        let id = model.emby_browser_id.clone().expect("browser mounted");

        // Paint the App and the mounted browser at 150 columns: both derive
        // the same two-column stride from the same painted geometry (the
        // generic library never takes the wide-Movies 1-column rail).
        render_browser_model(&mut model, 150, 40);
        model.sync_emby_browser();

        // Down: the focused component returns `BrowserMoveRows { rows: 1 }`
        // (one display row, in place of the raw key), and the shell runs
        // `App::move_lib_cursor_rows` — its own painted two-column stride
        // lands the App cursor on item 2, exactly like the legacy arm.
        let Some(Msg::Shell(ShellRequest::BrowserMoveRows { rows })) =
            drive_browser_key(&mut model, &id, Key::Down, KeyModifiers::NONE)
        else {
            panic!("focused browser Down must emit BrowserMoveRows, got no typed request");
        };
        assert_eq!(rows, 1, "Down must carry one display row");
        model.handle_browser_request(ShellRequest::BrowserMoveRows { rows });
        assert_eq!(
            model.app.libs[0].nav_stack[0].cursor, 2,
            "two-column Down must move the App cursor two items via move_lib_cursor_rows"
        );

        // End/Home jump the App cursor to the last/first item through
        // `App::jump_lib_cursor`.
        let Some(Msg::Shell(ShellRequest::BrowserJumpCursor { to_end })) =
            drive_browser_key(&mut model, &id, Key::End, KeyModifiers::NONE)
        else {
            panic!("focused browser End must emit BrowserJumpCursor, got no typed request");
        };
        assert!(to_end, "End must carry to_end: true");
        model.handle_browser_request(ShellRequest::BrowserJumpCursor { to_end });
        assert_eq!(model.app.libs[0].nav_stack[0].cursor, 9);
        let Some(Msg::Shell(ShellRequest::BrowserJumpCursor { to_end })) =
            drive_browser_key(&mut model, &id, Key::Home, KeyModifiers::NONE)
        else {
            panic!("focused browser Home must emit BrowserJumpCursor, got no typed request");
        };
        assert!(!to_end, "Home must carry to_end: false");
        model.handle_browser_request(ShellRequest::BrowserJumpCursor { to_end });
        assert_eq!(model.app.libs[0].nav_stack[0].cursor, 0);

        // Right/Left move the App cursor within the row via
        // `App::move_lib_cursor` (the two-column list claims them).
        let Some(Msg::Shell(ShellRequest::BrowserMoveColumn { delta })) =
            drive_browser_key(&mut model, &id, Key::Right, KeyModifiers::NONE)
        else {
            panic!("focused two-column browser Right must emit BrowserMoveColumn, got no typed request");
        };
        assert_eq!(delta, 1, "Right must carry +1");
        model.handle_browser_request(ShellRequest::BrowserMoveColumn { delta });
        assert_eq!(model.app.libs[0].nav_stack[0].cursor, 1);
        let Some(Msg::Shell(ShellRequest::BrowserMoveColumn { delta })) =
            drive_browser_key(&mut model, &id, Key::Char('h'), KeyModifiers::NONE)
        else {
            panic!(
                "focused two-column browser h must emit BrowserMoveColumn, got no typed request"
            );
        };
        assert_eq!(delta, -1, "h must carry -1");
        model.handle_browser_request(ShellRequest::BrowserMoveColumn { delta });
        assert_eq!(model.app.libs[0].nav_stack[0].cursor, 0);

        // One-column list (queue panel restored at a width whose library
        // pane stays below the 82-column threshold): Left/Right/h/l stay
        // unbound locally — forwarded through the typed global bridge with no
        // movement request, App cursor unchanged — while the row keys keep
        // their typed stride of one item.
        model.app.panel_mode = PanelMode::Both;
        render_browser_model(&mut model, 100, 40);
        model.sync_emby_browser();
        for key in [Key::Left, Key::Right, Key::Char('h'), Key::Char('l')] {
            assert!(
                matches!(
                    drive_browser_key(&mut model, &id, key, KeyModifiers::NONE),
                    Some(Msg::Shell(ShellRequest::GlobalViewKey(_)))
                ),
                "one-column focused {key:?} must use the typed global bridge"
            );
        }
        let comp_cursor = model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<BrowserComponent>()
            .unwrap()
            .cursor();
        assert_eq!(
            comp_cursor, 0,
            "one-column Left/Right/h/l must not move the component cursor"
        );
        assert_eq!(
            model.app.libs[0].nav_stack[0].cursor, 0,
            "one-column Left/Right/h/l must not move the App cursor"
        );
        let Some(Msg::Shell(ShellRequest::BrowserMoveRows { rows })) =
            drive_browser_key(&mut model, &id, Key::Down, KeyModifiers::NONE)
        else {
            panic!("focused one-column browser Down must still emit BrowserMoveRows, got no typed request");
        };
        assert_eq!(rows, 1);
        model.handle_browser_request(ShellRequest::BrowserMoveRows { rows });
        assert_eq!(
            model.app.libs[0].nav_stack[0].cursor, 1,
            "one-column Down must stride the App cursor one item"
        );
    }

    /// Paint the App base frame and then the mounted Emby browser into a
    /// `TestBackend` of the given size — the same two-step the live shell's
    /// draw closure performs, so the App layout and the component's own
    /// painted `LayoutMain` agree on the column stride.
    fn render_browser_model(model: &mut Model, width: u16, height: u16) {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            model.app.render(f);
            model.render_emby_browser_component(f);
        })
        .unwrap();
    }

    /// A generic (non-Movies) Emby library with `n` flat Movie items: below
    /// the 82-column breakpoint it never takes the wide-Movies hero-on-left
    /// rail, so whatever column count the painted pane derives is the plain
    /// flat-list stride for both the App and the mounted browser.
    fn browser_app_with_flat_movies(n: usize) -> App {
        let mut app = make_app_stub();
        app.tab = TabSelection::EmbyLibrary(0);

        let mut library = make_item("Films", "CollectionFolder");
        library.id = "lib-films".into();
        library.is_folder = true;
        library.collection_type = "generic".into();

        app.libs.push(LibraryTab {
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-films".into(),
                title: "Films".into(),
                items: make_items(n),
                total_count: n,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
                music_grouping: None,
            }],
            ..LibraryTab::new(library)
        });

        app
    }
}
