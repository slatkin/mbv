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

    pub(super) fn sync_emby_browser(&mut self) {
        let next_id = self.emby_browser_component_id();
        if self.emby_browser_id != next_id {
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

    pub(super) fn render_emby_browser_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.emby_browser_id.as_ref() else {
            return;
        };
        let area = self.app.layout.main.left_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(id, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{BrowserComponent, LegacyTerminalEvent, Msg};
    use crate::app::render::make_movie_app;
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::{App, BrowseLevel, ContextAction, LibraryTab, TabSelection};
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
        // nav cursor stays parked on the folder (index 0).
        assert!(matches!(
            drive_browser_key(&mut model, &id, Key::Down, KeyModifiers::NONE),
            Some(Msg::Legacy(LegacyTerminalEvent::Key(_)))
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
        let Some(Msg::Legacy(LegacyTerminalEvent::Key(key))) = message else {
            panic!("browser movement should forward to the legacy handler");
        };
        assert!(!model.app.handle_key(key));
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
}
