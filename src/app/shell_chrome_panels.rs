//! S1 chrome panels (tasks 2.1–2.2): the tab bar and the status row move
//! from shell-painted base-frame chrome to mounted Interactive Components
//! (`TabPanel`, `StatusBarPanel`). The sync pass mounts each panel exactly
//! when its `RootFrame` placement exists (design D1's mount rule, following
//! the `sync_queue_boundary` precedent) and projects shell-owned content
//! into it one-way; the draw path only hands each panel its placement.

use super::components::{ComponentId, StatusBarPanel, TabPanel};
use super::*;
use crate::app::render::StatusBarModel;

impl Model {
    /// Mount/unmount the `TabPanel` to the `RootFrame.tab` placement and
    /// project the tab content (titles, selected position, scroll anchor).
    pub(super) fn sync_tab_panel(&mut self) {
        let id = ComponentId::TabPanel;
        let Some(_placement) = self.app.layout.root_frame.tab else {
            // D1 mount rule: no placement in the current Panel mode, so the
            // panel is unmounted here and never outlives its placement.
            if self.application.mounted(&id) {
                let _ = self.application.umount(&id);
            }
            return;
        };
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(TabPanel::new()), vec![])
                .expect("mount TabPanel");
        }
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
        let id = ComponentId::StatusBarPanel;
        let Some(_placement) = self.app.layout.root_frame.status_bar else {
            if self.application.mounted(&id) {
                let _ = self.application.umount(&id);
            }
            return;
        };
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(StatusBarPanel::new()), vec![])
                .expect("mount StatusBarPanel");
        }
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
    pub(super) fn render_tab_panel(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::TabPanel;
        if let Some(area) = self
            .app
            .layout
            .root_frame
            .tab
            .filter(|_| self.application.mounted(&id))
        {
            self.application.view(&id, frame, area);
        }
    }

    /// Paint the mounted `StatusBarPanel` into the `RootFrame.status_bar`
    /// placement.
    pub(super) fn render_status_bar_panel(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::StatusBarPanel;
        if let Some(area) = self
            .app
            .layout
            .root_frame
            .status_bar
            .filter(|_| self.application.mounted(&id))
        {
            self.application.view(&id, frame, area);
        }
    }
}
