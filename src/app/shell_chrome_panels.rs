//! S1 chrome panels (tasks 2.1–2.2): the tab bar and the status row move
//! from shell-painted base-frame chrome to mounted Interactive Components
//! (`TabPanel`, `StatusBarPanel`). The sync pass mounts each panel exactly
//! when its `RootFrame` placement exists (design D1's mount rule, following
//! the `sync_queue_boundary` precedent) and projects shell-owned content
//! into it one-way; the draw path only hands each panel its placement.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::component::AppComponent;

use super::components::{ComponentId, Msg, StatusBarPanel, TabPanel, UserEvent};
use super::*;
use crate::app::render::arrangements::chrome::RootFrame;
use crate::app::render::StatusBarModel;

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
}

impl ChromePanel {
    fn id(self) -> ComponentId {
        match self {
            Self::Tab => ComponentId::TabPanel,
            Self::StatusBar => ComponentId::StatusBarPanel,
        }
    }

    fn new_component(self) -> Box<dyn AppComponent<Msg, UserEvent>> {
        match self {
            Self::Tab => Box::new(TabPanel::new()),
            Self::StatusBar => Box::new(StatusBarPanel::new()),
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
