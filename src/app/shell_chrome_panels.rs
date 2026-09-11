//! S1 chrome panels (tasks 2.1–2.2): the tab bar and the status row move
//! from shell-painted base-frame chrome to mounted Interactive Components
//! (`TabPanel`, `StatusBarPanel`). The sync pass mounts each panel exactly
//! when its `RootFrame` placement exists (design D1's mount rule, following
//! the `sync_queue_boundary` precedent) and projects shell-owned content
//! into it one-way; the draw path only hands each panel its placement.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::component::AppComponent;

use super::components::{
    ComponentId, Msg, QueuePlaybackPanel, StatusBarPanel, TabPanel, UserEvent,
};
use super::*;
use crate::app::layout::CardGeometry;
use crate::app::render::arrangements::chrome::{
    queue_playback_column_wide, queue_playback_transport_area, RootFrame,
};
use crate::app::render::StatusBarModel;
use crate::app::NowPlayingStatus;

impl Model {
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
}

impl ChromePanel {
    fn id(self) -> ComponentId {
        match self {
            Self::Tab => ComponentId::TabPanel,
            Self::StatusBar => ComponentId::StatusBarPanel,
            Self::QueuePlayback => ComponentId::QueuePlaybackPanel,
        }
    }

    fn new_component(self) -> Box<dyn AppComponent<Msg, UserEvent>> {
        match self {
            Self::Tab => Box::new(TabPanel::new()),
            Self::StatusBar => Box::new(StatusBarPanel::new()),
            Self::QueuePlayback => Box::new(QueuePlaybackPanel::new()),
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
    pub(super) fn sync_tab_panel(&mut self) {
        let placement = self.sync_chrome_root().tab;
        self.mount_to_placement(ChromePanel::Tab, placement);
        let id = ChromePanel::Tab.id();
        let titles: Vec<String> = std::iter::once("Home".to_string())
            .chain(self.app.libs.iter().map(|l| l.library.name.clone()))
            .chain(
                self.app
                    .audiobookshelf_libraries
                    .iter()
                    .map(|l| l.name.clone()),
            )
            .chain(
                self.app
                    .has_feeds_subscriptions()
                    .then(|| "Feeds".to_string()),
            )
            .collect();
        let selected = self
            .app
            .tab
            .to_position_with_counts(self.app.libs.len(), self.app.feeds_tab_pos());
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(panel) = comp.as_any_mut().downcast_mut::<TabPanel>() {
                panel.set_content(titles, selected, self.app.tab_scroll);
            }
        }
    }

    /// Mount/unmount the `StatusBarPanel` to the `RootFrame.status_bar`
    /// placement and project the status-row content (pill and right-segment
    /// spans, built by `chrome_status.rs`'s App-side builders).
    pub(super) fn sync_status_bar_panel(&mut self) {
        let placement = self.sync_chrome_root().status_bar;
        self.mount_to_placement(ChromePanel::StatusBar, placement);
        let id = ChromePanel::StatusBar.id();
        // The base frame has always passed `show_session_pill: false` here:
        // the queue column's title pills show the same remote/session info.
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
        let model = StatusBarModel {
            show_session_pill,
            remote: remote_status,
            mute: self.app.mute_status_spans(),
            volume: self.app.volume_status_spans(),
            right: self.app.status_bar_right_spans(),
        };
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(panel) = comp.as_any_mut().downcast_mut::<StatusBarPanel>() {
                panel.set_model(model);
            }
        }
    }

    /// Mount/unmount the `QueuePlaybackPanel` to the `RootFrame.queue_playback`
    /// placement and project its content (task 3.5): the header row's status
    /// word and playback target, and the transport facts from the shared
    /// transport projection. Mounted in every queue-visible layout, idle
    /// included, because the header is always painted (D10).
    pub(super) fn sync_queue_playback_panel(&mut self) {
        let placement = self.sync_chrome_root().queue_playback;
        self.mount_to_placement(ChromePanel::QueuePlayback, placement);
        let id = ChromePanel::QueuePlayback.id();
        let mut transport = self.transport_projection();
        // The queue column's transport band: the fixed chrome surface, in
        // every queue-visible layout (D10).
        transport.panel = crate::app::palette::Surface::QueueOnlyPlaybackPanel;
        transport.panel_focused = false;
        let status = self.app.now_playing_status();
        let host = self.app.playback_host_label();
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(panel) = comp.as_any_mut().downcast_mut::<QueuePlaybackPanel>() {
                panel.set_header(status, host);
                panel.set_transport(transport);
            }
        }
    }

    /// Paint the mounted `QueuePlaybackPanel` into the `RootFrame.queue_playback`
    /// placement (task 3.5, D1's view rule). The panel paints the header row —
    /// the placement's first row — and the transport rect the shell computes
    /// from the visual slot's freshly painted size (side by side at 100+
    /// columns with the 2-cell gap, stacked below it otherwise, through the
    /// shared `queue_playback_transport_area` arrangement); the App-side slot
    /// adapter (the moved `render_card`, task 3.4) paints the slot below the
    /// header row. While idle the slot and transport collapse to zero rows
    /// (task 3.6) and only the header row paints.
    pub(super) fn render_queue_playback_panel(&mut self, frame: &mut Frame) {
        let id = ComponentId::QueuePlaybackPanel;
        if !self.application.mounted(&id) {
            return;
        }
        // The panel paints into its `RootFrame.queue_playback` placement;
        // mounted exactly when a placement exists (the D1 mount rule), so
        // the placement check is both the paint gate and the view rect.
        let Some(placement) = self.app.layout.root_frame.queue_playback else {
            return;
        };
        // The slot/transport geometry recomputes from the same paint-free
        // checkpoint the placement was published from. The slot region starts
        // on the row below the placement's header row and reaches the rest of
        // the queue column: the slot's render caps itself (the same 12/24-row
        // budget the queue card always had), and a freshly painted slot may
        // exceed the placement for one frame — the one-frame card publish
        // below reserves those rows from the next frame on.
        let chrome = self.app.compute_chrome_geometry(ratatui::layout::Rect::new(
            0,
            0,
            self.app.terminal_width,
            self.app.terminal_height,
        ));
        let content = chrome.left_content;
        let slot_region = Rect {
            y: content.y + 1,
            height: content.height.saturating_sub(1),
            ..content
        };
        let wide = queue_playback_column_wide(chrome.left_area.width);
        let (transport_area, card): (Option<Rect>, CardGeometry) =
            if self.app.now_playing_status() == NowPlayingStatus::Idle {
                // Idle collapse (task 3.6): no visual slot, no transport — the
                // panel paints only the header row, and the slot publishes zero
                // geometry so the queue panel reclaims the rows.
                (None, CardGeometry::default())
            } else {
                let (slot_h, slot_w, _) =
                    self.app
                        .render_queue_playback_slot(frame, slot_region, wide);
                let card = CardGeometry {
                    height: slot_h,
                    width: slot_w,
                };
                let transport_area =
                    queue_playback_transport_area(slot_region, wide, card.width, card.height);
                (Some(transport_area), card)
            };
        // The freshly painted slot size publishes for the queue panel's
        // placement this frame (it recomputes from `AppLayout::main.card`
        // after this pass) and for the next frame's chrome placements.
        self.app.layout.main.card = card;
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(panel) = comp.as_any_mut().downcast_mut::<QueuePlaybackPanel>() {
                panel.set_transport_area(transport_area);
            }
        }
        // The panel's `view` paints the header row (row 0 of the placement)
        // and the transport rect the shell just computed; the visual slot was
        // already painted by the App-side adapter above.
        self.application.view(&id, frame, placement);
    }

    /// Paint the mounted `TabPanel` into the `RootFrame.tab` placement.
    pub(super) fn render_tab_panel(&mut self, frame: &mut Frame) {
        let placement = self.app.layout.root_frame.tab;
        self.render_placed_panel(frame, placement, &ComponentId::TabPanel);
    }

    /// Paint the mounted `StatusBarPanel` into the `RootFrame.status_bar`
    /// placement.
    pub(super) fn render_status_bar_panel(&mut self, frame: &mut Frame) {
        let placement = self.app.layout.root_frame.status_bar;
        self.render_placed_panel(frame, placement, &ComponentId::StatusBarPanel);
    }
}
