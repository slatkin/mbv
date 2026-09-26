//! The Library panel's shell integration (task 5.9, design D2). The mounted
//! `LibraryPanel` is the library area's event boundary; the shell drives it
//! one-way: it mounts/unmounts it with the library column, points it at the
//! active library's owner, retains owners while their libraries are in the
//! catalog (the rule moved inside from `reconcile_destination_mounts`), and
//! hands it the session-only split width. The transitional branch lives in
//! the draw path: `RootFrame` gives the library rect to the panel only while
//! the active library's owner has migrated — otherwise the old mounted
//! destination stays the surface.

use ratatui::layout::Rect;

use super::components::book_content::BookContent;
use super::components::library_panel::content::HeroImageState;
use super::components::library_panel::owner::LaunchSelector;
use super::components::library_panel::{LibraryContentOwner, LibraryPanel};
use super::components::podcast_content::PodcastContent;
use super::components::{ComponentId, LibraryKey, LibraryKind};
use super::Model;
use super::{PanelFocus, PanelMode, TabSelection};
use crate::app::state::types::playback::DestinationLatestSource;
use mbv_core::config::ServiceKind;

impl Model {
    /// The active library's [`LibraryKey`] from the resolved tab: the owner
    /// map's addressing key (design D2: `Home | Feeds | Service(LibraryKey)`).
    pub(in crate::app) fn active_library_key(&self) -> Option<LibraryKey> {
        match self.app.tab {
            TabSelection::Home => Some(LibraryKey::Home),
            TabSelection::Feeds => Some(LibraryKey::Feeds),
            TabSelection::EmbyLibrary(index) => {
                let library = self.app.libs.get(index)?;
                Some(LibraryKey::Service {
                    service: ServiceKind::Emby,
                    library_id: library.library.id.clone(),
                    kind: LibraryKind::from_collection_type(&library.library.collection_type),
                })
            }
            TabSelection::AudiobookshelfLibrary(index) => {
                let library = self.app.audiobookshelf_libraries.get(index)?;
                let kind = match self.app.audiobookshelf_kind_at(index)? {
                    AudiobookshelfBrowseKind::Podcast => LibraryKind::AudiobookshelfPodcast,
                    AudiobookshelfBrowseKind::Book => LibraryKind::AudiobookshelfBook,
                };
                Some(LibraryKey::Service {
                    service: ServiceKind::Audiobookshelf,
                    library_id: library.id.clone(),
                    kind,
                })
            }
        }
    }

    /// Assemble the selected destination's bounded launch identities. This is
    /// a teardown-only query: the panel asks only its active owner, never any
    /// unselected destination.
    pub(in crate::app) fn launch_state_snapshot(&self) -> mbv_core::config::TuiLaunchState {
        let key = self.active_library_key();
        let (selector, item) = key
            .as_ref()
            .and_then(|key| {
                self.application
                    .get_component(&ComponentId::Library)
                    .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
                    .and_then(|panel| panel.launch_snapshot(key))
            })
            .unwrap_or((None, None));
        // A stale Service index has no stable library identity; represent it
        // as Home so startup follows the ordered first-guaranteed-tab fallback.
        let tab = key.as_ref().map_or(
            mbv_core::config::TabIdentity::Home,
            LibraryKey::tab_identity,
        );
        mbv_core::config::TuiLaunchState {
            version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
            tab,
            panel_focus: match self.app.effective_panel_focus() {
                PanelFocus::Library => mbv_core::config::LaunchPanelFocus::Library,
                PanelFocus::Queue => mbv_core::config::LaunchPanelFocus::Queue,
            },
            selector,
            item,
        }
    }

    /// The active library's stable selection origin (design D6/D7): the
    /// identity a projection or delayed action carries so a clear intent can
    /// route to the list that produced it, never the dispatch-time focus.
    pub(in crate::app) fn active_library_selection_origin(
        &self,
    ) -> Option<crate::app::components::media_list::SelectionOrigin> {
        self.active_library_key().map(|key| {
            crate::app::components::media_list::SelectionOrigin::Library(
                crate::app::components::media_list::LibrarySelectionOrigin::from(key),
            )
        })
    }

    fn set_emby_owner_latest_mode(&mut self, key: &LibraryKey, latest: bool) {
        if let Some(owner) = self
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(|panel| panel.owner_mut(key))
            .and_then(|owner| {
                owner
                    .as_any_mut()
                    .downcast_mut::<super::components::emby_library_content::EmbyLibraryContent>()
            })
        {
            owner.set_latest_mode(latest);
        }
    }

    fn spawn_emby_latest_snapshot(&mut self, key: &LibraryKey) {
        let LibraryKey::Service {
            service: ServiceKind::Emby,
            library_id,
            ..
        } = key
        else {
            return;
        };
        if let Some(lib_idx) = self
            .app
            .libs
            .iter()
            .position(|library| library.library.id == *library_id)
        {
            self.app.spawn_destination_latest_snapshot(lib_idx);
        }
    }

    fn apply_launch_selector(&mut self, key: &LibraryKey, selector: LaunchSelector) {
        match selector {
            LaunchSelector::EmbyLatest => {
                if !matches!(
                    key,
                    LibraryKey::Service {
                        kind: LibraryKind::Music,
                        ..
                    }
                ) {
                    self.set_emby_owner_latest_mode(key, true);
                    self.spawn_emby_latest_snapshot(key);
                }
            }
            LaunchSelector::Emby { index } => {
                if let Some(lib_idx) = self.app.tab.emby_library_index() {
                    if index == usize::MAX {
                        self.clear_emby_letter_filter(lib_idx);
                        self.set_emby_owner_latest_mode(key, false);
                    } else {
                        self.app.handle_mouse_selector_click_emby(lib_idx, index);
                        self.set_emby_owner_latest_mode(key, false);
                    }
                }
            }
            LaunchSelector::AudiobookshelfShow(library_item_id) => {
                self.app.select_audiobookshelf_show_target(&library_item_id);
            }
            LaunchSelector::AudiobookshelfState => {
                self.app.commit_audiobookshelf_podcast_state_scope();
            }
            LaunchSelector::AudiobookshelfLatest => {
                self.record_home_latest_acknowledgement(DestinationLatestSource::Audiobookshelf(
                    match key {
                        LibraryKey::Service { library_id, .. } => library_id.clone(),
                        _ => unreachable!(),
                    },
                ));
            }
        }
        match key {
            LibraryKey::Service {
                kind: LibraryKind::Music,
                ..
            } => self.push_music_workspace_content(),
            LibraryKey::Service {
                kind: LibraryKind::TvShows,
                ..
            } => self.push_tv_workspace_content(),
            LibraryKey::Service {
                kind: LibraryKind::AudiobookshelfPodcast,
                ..
            } => self.push_audiobookshelf_podcast_content(),
            _ => self.push_active_emby_library_owner_content(),
        }
    }

    /// Return an Emby letter-pilled library to its unfiltered scope. This is
    /// the clear counterpart to an ordinary pill click: it refreshes the full
    /// range instead of treating index zero as an A–C pill.
    pub(in crate::app) fn clear_emby_letter_filter(&mut self, lib_idx: usize) {
        if !self.app.should_show_letter_pills(lib_idx) {
            return;
        }
        let Some(level) = self.app.libs[lib_idx].nav_stack.last() else {
            return;
        };
        if level.letter_filter.is_none() {
            return;
        }
        let mut key = crate::app::state::types::browse::LevelFetchKey::from_level(level);
        key.letter_filter = None;
        if let Some(level) = self.app.libs[lib_idx].nav_stack.last_mut() {
            level.letter_filter = None;
            level.set_resting_cursor(0);
            level.set_resting_scroll(0);
            level.loading = true;
            level.items.clear();
            level.all_items = None;
        }
        self.app.spawn_refresh(lib_idx, 0, key);
        self.app.save_default_library_position(lib_idx);
    }

    /// Consume the pending destination-level launch state after the selected
    /// owner has received its current content. Resolution is deliberately
    /// pill-before-item and happens once; later refreshes only project content.
    pub(in crate::app) fn reanchor_pending_launch_destination(&mut self) {
        if !self.app.pending_launch_tab_resolved {
            return;
        }
        let Some(state) = self.app.pending_launch_state.clone() else {
            return;
        };
        let Some(key) = self.active_library_key() else {
            return;
        };
        let selector = self
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .and_then(|panel| panel.launch_selector(&key, &state));
        if let Some(selector) = selector {
            self.apply_launch_selector(&key, selector);
        }
        let applied = self
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .is_some_and(|panel| panel.reanchor_launch_state(&key, &state));
        if applied {
            let focus = match state.panel_focus {
                mbv_core::config::LaunchPanelFocus::Library => PanelFocus::Library,
                mbv_core::config::LaunchPanelFocus::Queue => PanelFocus::Queue,
            };
            self.app.set_panel_focus(focus);
            // Queue focus is restored only after the selected destination has
            // accepted its launch state. Re-run the normal Queue projection
            // so its framework focus, frame state, and local selection follow
            // the same path as an ordinary panel-focus change; no Queue
            // target is carried by the launch snapshot.
            self.sync_queue();
            self.app.pending_launch_state = None;
            self.app.pending_launch_tab_resolved = false;
        }
    }

    /// Every [`LibraryKey`] currently in the catalog: the shell tabs that are
    /// always live (Home, Feeds) plus one `Service` key per configured
    /// library, independent of the active tab. `LibraryPanel::retain_owners`
    /// drops owners whose key is not here (design D2's retention rule).
    pub(in crate::app) fn live_library_keys(&self) -> Vec<LibraryKey> {
        let mut keys = vec![LibraryKey::Home, LibraryKey::Feeds];
        keys.extend(self.app.libs.iter().map(|tab| LibraryKey::Service {
            service: ServiceKind::Emby,
            library_id: tab.library.id.clone(),
            kind: LibraryKind::from_collection_type(&tab.library.collection_type),
        }));
        keys.extend(self.app.audiobookshelf_libraries.iter().map(|library| {
            // `from_media_type` maps every ABS media type to exactly one of
            // Book | Podcast (same rule the browse surfaces apply).
            let kind = match AudiobookshelfBrowseKind::from_media_type(&library.media_type) {
                AudiobookshelfBrowseKind::Podcast => LibraryKind::AudiobookshelfPodcast,
                AudiobookshelfBrowseKind::Book => LibraryKind::AudiobookshelfBook,
            };
            LibraryKey::Service {
                service: ServiceKind::Audiobookshelf,
                library_id: library.id.clone(),
                kind,
            }
        }));
        keys
    }

    /// Whether the panel hosts an owner for `key`.
    pub(in crate::app) fn library_panel_has_owner(&self, key: &LibraryKey) -> bool {
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .is_some_and(|panel| panel.has_owner(key))
    }

    /// Typed immutable access to the panel's owner for `key`: the shared
    /// lookup/downcast path every per-content-type `*_owner` reader collapses
    /// to (design D2 — one panel, many typed owners).
    pub(in crate::app) fn library_owner<T: LibraryContentOwner + 'static>(
        &self,
        key: &LibraryKey,
    ) -> Option<&T> {
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .and_then(|panel| panel.owner(key))
            .and_then(|owner| owner.as_any().downcast_ref::<T>())
    }

    /// Typed mutable access to the panel's owner for `key`.
    pub(in crate::app) fn library_owner_mut<T: LibraryContentOwner + 'static>(
        &mut self,
        key: &LibraryKey,
    ) -> Option<&mut T> {
        self.application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(|panel| panel.owner_mut(key))
            .and_then(|owner| owner.as_any_mut().downcast_mut::<T>())
    }

    /// Mutate the owner for `key`, creating it via `make` on first reach: the
    /// shared create-if-absent path every per-content-type `update_*_owner`
    /// collapses to.
    pub(in crate::app) fn update_library_owner<T: LibraryContentOwner + 'static, R>(
        &mut self,
        key: &LibraryKey,
        make: impl FnOnce() -> Box<T>,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        if !self.library_panel_has_owner(key) {
            self.push_library_owner(key.clone(), make());
        }
        self.library_owner_mut(key).map(f)
    }

    /// Open the active owner's Hero through the Library panel's local overlay
    /// contract. Destination actions use this instead of constructing the
    /// Library Hero overlay.
    pub(in crate::app) fn open_library_hero_overlay(&mut self) -> bool {
        self.application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .is_some_and(LibraryPanel::open_hero_overlay_for_active)
    }

    /// Push one content owner into the panel, addressed by `LibraryKey`
    /// (design D2). Production callers are the per-destination conversion
    /// slices (tasks 5.11+); the test harness pushes fixture owners to prove
    /// the mount/focus/mouse/retention wiring ahead of any conversion.
    pub(in crate::app) fn push_library_owner(
        &mut self,
        key: LibraryKey,
        owner: Box<dyn LibraryContentOwner>,
    ) {
        if let Some(panel) = self
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        {
            panel.insert_owner(key, owner);
        }
    }

    /// Mount the `LibraryPanel` with the library column (design D1's mount
    /// rule for the Library panel), drive its owner map — the active pointer
    /// and the catalog-retention rule — and push the session split width. The
    /// panel paints only a migrated owner; with zero migrated owners the old
    /// destination stays the surface, so this never makes the panel claim a
    /// surface it does not paint.
    ///
    /// D1 mount-rule exception (design D2): D1 would unmount the panel when
    /// the library column hides, but the owners live inside it, and an
    /// unmount would destroy every owner's cursor/scroll/focus/drafts — the
    /// retention D2 grants while the library is in the catalog. Once mounted
    /// the panel therefore stays mounted across Panel modes (mounted ≠
    /// painted): `render_library_panel` views it only with a real placement
    /// and a migrated active owner, and the mouse eligibility ladder
    /// subscribes it only while the library column is visible, so a hidden
    /// panel never paints or claims input.
    pub(in crate::app) fn sync_library_panel(&mut self) {
        let id = ComponentId::Library;
        if self.library_panel_visible() && !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(LibraryPanel::new()), vec![])
                .expect("mount LibraryPanel");
        }
        // Embedded owners are installed/projected after the panel is mounted
        // so the first painted frame has a current owner and geometry, and
        // re-projected every sync pass so navigation landings, async
        // completions, and saved-position restores reach the retained
        // owners (whose re-seed guards make repeat pushes idempotent).
        self.push_active_emby_library_owner_content();
        self.push_music_workspace_content();
        let active = self.active_library_key();
        // Register the Books owner as part of panel/catalog reconciliation,
        // not from the Books content projection. A newly active Books tab is
        // then populated by this discrete registration hand-off.
        let register_book = active.as_ref().is_some_and(|key| {
            matches!(
                key,
                LibraryKey::Service {
                    service: ServiceKind::Audiobookshelf,
                    kind: LibraryKind::AudiobookshelfBook,
                    ..
                }
            ) && !self.library_panel_has_owner(key)
        });
        if register_book {
            if let Some(key) = active.clone() {
                self.push_library_owner(key, Box::new(BookContent::new()));
                self.push_audiobookshelf_book_content();
            }
        }
        let register_podcast = active.as_ref().is_some_and(|key| {
            matches!(
                key,
                LibraryKey::Service {
                    service: ServiceKind::Audiobookshelf,
                    kind: LibraryKind::AudiobookshelfPodcast,
                    ..
                }
            ) && !self.library_panel_has_owner(key)
        });
        if register_podcast {
            if let Some(key) = active.clone() {
                self.push_library_owner(key, Box::new(PodcastContent::new()));
                self.push_audiobookshelf_podcast_content();
            }
        }
        let live = self.live_library_keys();
        let list_pane_width = self.app.list_pane_width;
        // The split gesture's eligibility-loss reset is decided before the
        // panel borrow (the panel owns the split gesture state, so the
        // panel's split drag moved in from): an overlay mount mid-drag must
        // not leave a stale armed drag behind.
        let mouse_eligible = self.panel_mouse_eligible();
        let Some(panel) = self
            .application
            .get_component_mut(&id)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        else {
            return;
        };
        // Owner retention while the library is in the catalog (design
        // D2): the rule `reconcile_destination_mounts` applies to the old
        // destination components, moved inside for the panel's owners. Driven
        // in every Panel mode so a hidden library's owners keep their state.
        panel.retain_owners(&live);
        panel.set_active(active);
        panel.sync_overlay_state();
        panel.set_list_pane_width(list_pane_width);
        panel.set_terminal_height(self.app.terminal_height);
        panel.sync_mouse_eligibility(mouse_eligible);
    }

    /// The library content rect painted by the mounted panel.
    pub(in crate::app) fn library_panel_content_area(&self) -> Option<Rect> {
        let area = self.app.layout.root_frame.library?;
        let collapsed = self.app.effective_panel_mode() != PanelMode::Both;
        Some(crate::app::render::components::widgets::right_panel_content_area(area, collapsed))
    }

    /// The Library column's body fill: the placement's own back, painted with
    /// the panel's focus bit. Focused it takes the column level's
    /// `SURFACE_FOCUSED` fill, resting the column's app backdrop; one named
    /// authority both the placement fill and the status band's padding rows
    /// read, so both follow the same bit in every geometry.
    pub(in crate::app) fn library_body_fill(&self) -> ratatui::style::Color {
        let focused = matches!(self.app.effective_panel_focus(), super::PanelFocus::Library);
        crate::app::palette::surface_colors(crate::app::palette::Surface::LibraryColumn, focused)
            .fill
    }

    /// The transitional draw step: give the library rect to the mounted
    /// `LibraryPanel` when the active library's owner has migrated; otherwise
    /// the old destination components paint (the caller gates them).
    pub(in crate::app) fn render_library_panel_at(
        &mut self,
        frame: &mut ratatui::Frame,
        area: Rect,
    ) {
        let id = ComponentId::Library;
        if !self.application.mounted(&id) {
            return;
        }
        // Fill the placement's own background first (task 12.2): the panel
        // paints a narrower inset content rect (`library_panel_content_area`
        // insets by `TAB_LEFT_PAD`/one column, matching the tab bar's own
        // left indent), so the margin columns outside that inset must still
        // show the column's own background rather than whatever was painted
        // underneath before this panel owned the placement.
        frame.render_widget(ratatui::widgets::Clear, area);
        // The panel body follows the library column's focus pair: the panel's
        // own back lightens while the right panel holds focus. The non-Wide
        // panel is the Wide browser pane without a Hero, so one column
        // identity serves both skeletons.
        let content_area = self.library_panel_content_area().unwrap_or(area);
        frame.render_widget(
            ratatui::widgets::Block::default()
                .style(ratatui::style::Style::default().bg(self.library_body_fill())),
            area,
        );
        self.application.view(&id, frame, content_area);
        // The projected hero image's pixel paint (task 5.10, design D9): the
        // painters read projected state and reserve the box; the shell paints
        // the cached protocol into it right after view returns — the same
        // defer-the-pixel-paint seam every destination component uses.
        let image_paint = self
            .application
            .get_component_mut(&id)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(LibraryPanel::take_image_paint);
        if let Some(paint) = image_paint {
            let suffix = self.app.current_protocol_suffix();
            self.app
                .images
                .paint_panel_hero_image(frame, &paint, suffix);
        }
    }

    /// The library heroes' image projection (task 5.10, design D9, TV's
    /// `push_tv_workspace_content` shape generalized): for the active
    /// migrated owner's current hero, run the artwork policy's source and the
    /// paint-free `hero_artwork_box` with the Library panel's `RootFrame`
    /// area, issue every `fetch_card_image` here, and project the image
    /// state (loading/ready, decoded size, cover-fit box keyed by size)
    /// into the owner. Painting reads the projected state only; the queue
    /// visual slot's separate projection (task 3.4) is untouched.
    /// Driven every sync pass so a cursor move, breakpoint change, or split
    /// drag re-projects on the next pass.
    pub(in crate::app) fn sync_library_hero_images(&mut self) {
        if !self.library_panel_visible() {
            return;
        }
        let Some(area) = self.library_panel_content_area() else {
            return;
        };
        let list_pane_width = self.app.list_pane_width;
        let hero_data = {
            let panel = self
                .application
                .get_component_mut(&ComponentId::Library)
                .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>());
            if let Some(data) = panel.and_then(LibraryPanel::active_hero_data) {
                data
            } else {
                // No hero this frame: the placeholder is final.
                if let Some(panel) = self
                    .application
                    .get_component_mut(&ComponentId::Library)
                    .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
                {
                    panel.set_active_hero_image(HeroImageState::None);
                }
                return;
            }
        };
        let overlay_box = self
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .and_then(LibraryPanel::active_hero_image_box);
        let state = self.app.project_hero_image(
            &hero_data.facts,
            false,
            area,
            list_pane_width,
            overlay_box,
        );
        if let Some(panel) = self
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        {
            panel.set_active_hero_image(state);
        }
    }
}

use crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseKind;
