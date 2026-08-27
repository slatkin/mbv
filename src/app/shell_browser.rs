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
#[path = "shell_browser_tests.rs"]
mod tests;
