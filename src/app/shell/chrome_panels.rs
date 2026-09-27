//! S1 chrome panels (tasks 2.1–2.2): the tab bar and the status row move
//! from shell-painted base-frame chrome to mounted Interactive Components
//! (`TabPanel`, `StatusBarPanel`). The sync pass mounts each panel exactly
//! when its `RootFrame` placement exists (design D1's mount rule, following
//! the `sync_queue_boundary` precedent) and projects shell-owned content
//! into it one-way; the draw path only hands each panel its placement.
//! S3 (task 4.1): the right-column strip joins them as the mounted
//! `LibraryPlaybackPanel`, mounted only when the queue column is hidden.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::component::AppComponent;

use super::components::{
    ComponentId, LibraryPlaybackPanel, Msg, QueueComponent, QueuePlaybackPanel, StatusBarPanel,
    TabPanel, UserEvent,
};
use super::{App, DestinationLatestSource, Model, PanelFocus};
use crate::app::components::library_panel::LibraryPanel;
use crate::app::layout::CardGeometry;
use crate::app::render::arrangements::chrome::{
    queue_playback_column_wide, queue_playback_transport_area, status_bar_row, RootFrame,
    QUEUE_PLAYBACK_HEADER_ROWS,
};
use crate::app::render::components::card::queue_card_reserved_rect;
use crate::app::render::components::widgets::{fill_surface, queue_panel_inset};
use crate::app::render::{StatusBarModel, VisualModeIndicator};
use crate::app::NowPlayingStatus;

pub(crate) fn sync_panel_area(app: &App) -> Option<Rect> {
    let area = app
        .compute_chrome_geometry(Rect::new(0, 0, app.terminal_width, app.terminal_height))
        .panel_area;
    (area.width > 0 && area.height > 0).then_some(area)
}

impl Model {
    /// Publish the queue card reservation before root placements are synced.
    /// The reservation is derived from the prior paint checkpoint, so draw
    /// remains read-only with respect to `AppLayout`.
    pub(in crate::app) fn sync_queue_card_geometry(&mut self) {
        if !self.app.visual_slot_shown() {
            self.app.layout.card = CardGeometry::default();
            return;
        }
        let chrome = self.app.compute_chrome_geometry(Rect::new(
            0,
            0,
            self.app.terminal_width,
            self.app.terminal_height,
        ));
        if chrome.left_area.width == 0 {
            self.app.layout.card = CardGeometry::default();
            return;
        }
        let slot_region = Rect {
            y: chrome.left_content.y + 1,
            height: chrome.left_content.height.saturating_sub(1),
            ..chrome.left_content
        };
        let wide = queue_playback_column_wide(chrome.left_area.width);
        let rect = queue_card_reserved_rect(
            (
                self.app.images.last_card_height,
                self.app.images.last_card_width,
            ),
            self.app.terminal_height,
            slot_region,
            wide,
        );
        self.app.layout.card = CardGeometry {
            height: rect.height,
            width: rect.width,
        };
    }

    /// Paint-free chrome geometry for the sync pass. `compute_chrome_geometry`
    /// is a pure read of `App` state (its S0 doc comment), so the panel mounts
    /// are decidable here — before any draw — from the same computation the
    /// draw path publishes as `AppLayout.root_frame`. Deciding from the
    /// published `root_frame` instead would leave a placement that just
    /// appeared (startup, or leaving mini-view) unmounted for the first frame
    /// that shows it.
    fn sync_chrome_root(&self) -> RootFrame {
        self.app
            .compute_chrome_geometry(ratatui::layout::Rect::new(
                0,
                0,
                self.app.terminal_width,
                self.app.terminal_height,
            ))
            .root
    }
}

/// The two chrome panels' shared mount-to-placement prologue (review of
/// tasks 2.1-2.2): one copy of the D1 mount rule, so the S2 queue panels
/// add an arm here instead of a second prologue.
#[derive(Clone, Copy)]
enum ChromePanel {
    Tab,
    StatusBar,
    QueuePlayback,
    LibraryPlayback,
}

impl ChromePanel {
    fn id(self) -> ComponentId {
        match self {
            Self::Tab => ComponentId::TabPanel,
            Self::StatusBar => ComponentId::StatusBarPanel,
            Self::QueuePlayback => ComponentId::QueuePlaybackPanel,
            Self::LibraryPlayback => ComponentId::LibraryPlaybackPanel,
        }
    }

    fn new_component(self) -> Box<dyn AppComponent<Msg, UserEvent>> {
        match self {
            Self::Tab => Box::new(TabPanel::new()),
            Self::StatusBar => Box::new(StatusBarPanel::new()),
            Self::QueuePlayback => Box::new(QueuePlaybackPanel::new()),
            Self::LibraryPlayback => Box::new(LibraryPlaybackPanel::new()),
        }
    }
}

impl Model {
    /// D1 mount rule, shared by all chrome panels: mount the panel when its
    /// placement exists in the current Panel mode, unmount it when it
    /// doesn't, so it never outlives its placement.
    fn mount_to_placement(&mut self, panel: ChromePanel, placement: Option<Rect>) {
        let id = panel.id();
        match placement {
            Some(_) => {
                if !self.application.mounted(&id) {
                    self.application
                        .mount(id.clone(), panel.new_component(), vec![])
                        .unwrap_or_else(|e| panic!("mount {id:?}: {e:?}"));
                }
            }
            None => {
                if self.application.mounted(&id) {
                    let _ = self.application.umount(&id);
                }
            }
        }
    }

    /// Paint the mounted panel into its `RootFrame` placement, if both exist.
    fn render_placed_panel(
        &mut self,
        frame: &mut Frame,
        placement: Option<Rect>,
        id: &ComponentId,
    ) {
        if let Some(area) = placement.filter(|_| self.application.mounted(id)) {
            self.application.view(id, frame, area);
        }
    }

    /// Mount/unmount the `TabPanel` to the `RootFrame.tab` placement and
    /// project the tab content (titles, selected position, scroll anchor).
    pub(in crate::app) fn sync_tab_panel(&mut self) {
        let placement = self.sync_chrome_root().tab;
        self.mount_to_placement(ChromePanel::Tab, placement);
        let id = ChromePanel::Tab.id();
        let (titles, markers): (Vec<String>, Vec<bool>) = std::iter::once((
            crate::app::ui_util::continue_tab_title(self.app.use_nerd_fonts).to_string(),
            false,
        ))
        .chain(self.app.libs.iter().map(|lib| {
            let marker = self
                .destination_latest_marker(&DestinationLatestSource::Emby(lib.library.id.clone()));
            (lib.library.name.clone(), marker)
        }))
        .chain(self.app.audiobookshelf_libraries.iter().map(|lib| {
            let marker = self.destination_latest_marker(&DestinationLatestSource::Audiobookshelf(
                lib.id.clone(),
            ));
            (lib.name.clone(), marker)
        }))
        .chain(self.app.has_feeds_subscriptions().then(|| {
            let marker = self.destination_latest_marker(&DestinationLatestSource::Feeds);
            ("Feeds".to_string(), marker)
        }))
        .unzip();
        let selected = self
            .app
            .tab
            .to_position_with_counts(self.app.libs.len(), self.app.feeds_tab_pos());
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(panel) = comp.as_any_mut().downcast_mut::<TabPanel>() {
                panel.set_content(titles, markers, selected, self.app.tab_scroll);
            }
        }
    }

    /// Mount/unmount the `StatusBarPanel` to the `RootFrame.status_bar`
    /// placement and project the status-row content (pill and right-segment
    /// spans, built by `chrome_status.rs`'s App-side builders). The
    /// Local/Remote queue-scope pills are queue concern: the shell projects
    /// them into the `QueueComponent` footer in `sync_queue`, never here.
    pub(in crate::app) fn sync_status_bar_panel(&mut self) {
        let placement = self.sync_chrome_root().status_bar;
        self.mount_to_placement(ChromePanel::StatusBar, placement);
        let id = ChromePanel::StatusBar.id();
        // The base frame has always passed `show_session_pill: false` here.
        let show_session_pill = false;
        let remote_status = if show_session_pill {
            let endpoint = self
                .app
                .config
                .lock()
                .unwrap()
                .daemon_client_endpoint
                .clone();
            self.app
                .remote_status_spans(self.app.remote_slot_state(), &endpoint)
        } else {
            Vec::new()
        };
        // Queue-scope pills are queue concern (projected into the
        // `QueueComponent` footer in `sync_queue`); the status row carries
        // only the focused list's count-only summary.
        let focused_summary = if self.app.effective_panel_focus() == PanelFocus::Queue {
            self.application
                .get_component(&ComponentId::Queue)
                .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
                .map(super::super::components::queue::QueueComponent::selection_summary)
        } else {
            self.application
                .get_component_mut(&ComponentId::Library)
                .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
                .and_then(
                    super::super::components::library_panel::panel::LibraryPanel::focused_summary,
                )
        };
        let visual_origin = focused_summary
            .as_ref()
            .map(|summary| summary.origin.clone());
        self.visual_selection = focused_summary
            .as_ref()
            .filter(|summary| summary.count > 0)
            .map(|summary| (self.app.effective_panel_focus(), summary.count));
        let model = StatusBarModel {
            show_session_pill,
            remote: remote_status,
            mute: self.app.mute_status_spans(),
            volume: self.app.volume_status_spans(),
            right: self.app.status_bar_right_spans(),
            visual_mode: self
                .visual_selection
                .map(|(_, count)| VisualModeIndicator { count }),
            prefix_armed: self.app.prefix_armed_status_spans(),
        };
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(panel) = comp.as_any_mut().downcast_mut::<StatusBarPanel>() {
                // The clear intent must carry the origin captured when this
                // pill was projected (design D6), not the focus at dispatch.
                if let Some(origin) = visual_origin {
                    panel.set_visual_origin(origin);
                }
                panel.set_model(model);
            }
        }
    }

    /// Mount/unmount the `QueuePlaybackPanel` to the `RootFrame.queue_playback`
    /// placement and project its content (task 3.5): the header row's status
    /// word and playback target, and the transport facts from the shared
    /// transport projection. Mounted in every queue-visible layout, idle
    /// included, because the header is always painted (D10).
    pub(in crate::app) fn sync_queue_playback_panel(&mut self) {
        let placement = self.sync_chrome_root().queue_playback;
        self.mount_to_placement(ChromePanel::QueuePlayback, placement);
        let id = ChromePanel::QueuePlayback.id();
        let mut transport = self.transport_projection();
        // The queue column's transport band: the fixed chrome surface, in
        // every queue-visible layout (D10).
        transport.panel = crate::app::palette::Surface::QueueOnlyPlaybackPanel;
        transport.panel_focused = false;
        let status = self.app.now_playing_status();
        let (host, host_is_remote) = self.app.playback_host_label_and_remote();
        let transport_area = if status == NowPlayingStatus::Idle {
            None
        } else {
            placement.map(|placement| {
                let inset = queue_panel_inset(placement);
                let slot_region = Rect {
                    y: placement.y + QUEUE_PLAYBACK_HEADER_ROWS,
                    height: placement.height.saturating_sub(QUEUE_PLAYBACK_HEADER_ROWS),
                    ..inset
                };
                let wide = queue_playback_column_wide(placement.width);
                let card = &self.app.layout.card;
                queue_playback_transport_area(
                    slot_region,
                    wide,
                    card.width,
                    card.height,
                    transport
                        .title_parts
                        .as_ref()
                        .is_some_and(|parts| parts.context.is_some()),
                )
            })
        };
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(panel) = comp.as_any_mut().downcast_mut::<QueuePlaybackPanel>() {
                panel.set_header(status, host, host_is_remote);
                panel.set_transport(transport);
                panel.set_transport_area(transport_area);
            }
        }
    }

    /// Paint the mounted `QueuePlaybackPanel` into the `RootFrame.queue_playback`
    /// placement (task 3.5, D1's view rule). The sync pass computes and
    /// publishes the transport rect from the prior-paint card checkpoint, so
    /// draw is read-only with respect to panel layout state. The panel paints
    /// the header row and transport; the App-side slot adapter (the moved
    /// `render_card`, task 3.4) paints the visual slot below the header row.
    /// While idle the slot and transport collapse to zero rows (task 3.6) and
    /// only the header row paints.
    pub(in crate::app) fn render_queue_playback_panel(
        &mut self,
        frame: &mut Frame,
        placement: Rect,
    ) {
        let id = ComponentId::QueuePlaybackPanel;
        if !self.application.mounted(&id) {
            return;
        }
        // The root loop supplies the placement and is the paint gate. Fill
        // the complete queue-column placement before the slot and transport
        // paint, including the outer padding and the boundary column.
        fill_surface(
            frame,
            placement,
            crate::app::palette::Surface::QueueColumn,
            matches!(self.app.effective_panel_focus(), PanelFocus::Queue),
        );
        // The slot region starts on the row below the placement's header band
        // (the header's recessed padding row plus the painted header row) and
        // keeps the queue panel's shared horizontal inner padding.
        let inset = queue_panel_inset(placement);
        let slot_region = Rect {
            y: placement.y + QUEUE_PLAYBACK_HEADER_ROWS,
            height: placement.height.saturating_sub(QUEUE_PLAYBACK_HEADER_ROWS),
            ..inset
        };
        let wide = queue_playback_column_wide(placement.width);
        // The slot region owns the QueueOnlyPlaybackPanel background even
        // when its content is hidden. Idle placements have a zero-height
        // slot region, so this remains a no-op while idle.
        frame.render_widget(
            ratatui::widgets::Block::default().style(
                ratatui::style::Style::default().bg(crate::app::palette::surface_colors(
                    crate::app::palette::Surface::QueueOnlyPlaybackPanel,
                    false,
                )
                .fill),
            ),
            slot_region,
        );
        if self.app.visual_slot_shown() {
            self.app
                .render_queue_playback_slot(frame, slot_region, wide);
        }
        self.application.view(&id, frame, placement);
    }

    /// Paint the mounted `TabPanel` into the `RootFrame.tab` placement.
    pub(in crate::app) fn render_tab_panel_at(&mut self, frame: &mut Frame, area: Rect) {
        self.render_placed_panel(frame, Some(area), &ComponentId::TabPanel);
    }

    /// Mount/unmount the `LibraryPlaybackPanel` to the `RootFrame
    /// .library_playback` placement and project the strip's transport facts
    /// from the shared transport projection (task 4.1, D10). Mounted only
    /// when the queue column is hidden — there the strip is the frame's one
    /// transport; in every queue-visible layout the panel is unmounted and
    /// the transport is the Queue playback panel's.
    pub(in crate::app) fn sync_library_playback_panel(&mut self) {
        let placement = self.sync_chrome_root().library_playback;
        self.mount_to_placement(ChromePanel::LibraryPlayback, placement);
        let id = ChromePanel::LibraryPlayback.id();
        let transport = self.transport_projection();
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(panel) = comp.as_any_mut().downcast_mut::<LibraryPlaybackPanel>() {
                panel.set_projection(transport);
            }
        }
    }

    /// Paint the mounted `LibraryPlaybackPanel` into the `RootFrame
    /// .library_playback` placement (task 4.1, D1's view rule). The panel
    /// paints the whole placement — the `PLAYER_BOX_HEIGHT` strip band —
    /// through the shared transport arrangement.
    pub(in crate::app) fn render_library_playback_panel_at(
        &mut self,
        frame: &mut Frame,
        area: Rect,
    ) {
        self.render_placed_panel(frame, Some(area), &ComponentId::LibraryPlaybackPanel);
    }

    /// Paint the mounted `StatusBarPanel` into its `RootFrame.status_bar`
    /// band. The band's padding rows take the library column's body fill
    /// (`library_body_fill`: the column's focus pair), and
    /// the status row is inset two columns each side, so the bar floats clear
    /// of the content above and the column's edges.
    pub(in crate::app) fn render_status_bar_panel_at(&mut self, frame: &mut Frame, area: Rect) {
        let id = ComponentId::StatusBarPanel;
        if !self.application.mounted(&id) {
            return;
        }
        // The band's padding rows belong to the library column, not the bar:
        // they take the same body fill the placement above them took — the
        // column's focus pair in every geometry. The bar
        // row itself is painted by `StatusBarPanel` afterwards.
        frame.render_widget(ratatui::widgets::Clear, area);
        frame.render_widget(
            ratatui::widgets::Block::default()
                .style(ratatui::style::Style::default().bg(self.library_body_fill())),
            area,
        );
        let row = status_bar_row(area);
        self.application.view(&id, frame, row);
    }
}
