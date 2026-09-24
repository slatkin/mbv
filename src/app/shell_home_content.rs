//! Shell-side Continue Watching projection and destination Latest marker state.

#[cfg(test)]
use super::components::home_content::HomeContent as HomeOwner;
#[cfg(test)]
use super::components::library_panel::LibraryKey;
#[cfg(test)]
use super::components::library_panel::LibraryPanel;
#[cfg(test)]
use super::components::ComponentId;
use super::notify_actions::ToastSeverity;
use super::shell::Model;
use crate::app::state::types::playback::{
    DestinationLatestSnapshot, DestinationLatestSource, HomeContent,
};
use mbv_core::playback_queue::QueueItem;
use std::time::Instant;

impl Model {
    pub(super) fn assign_home_content(&mut self, content: HomeContent) {
        self.home_content = content;
        self.push_home_content();
    }

    pub(super) fn update_emby_latest_snapshot(
        &mut self,
        library_id: String,
        title: String,
        items: Vec<QueueItem>,
    ) {
        let source = DestinationLatestSource::Emby(library_id.clone());
        let snapshot = self
            .tv_latest_snapshots
            .entry(library_id.clone())
            .or_insert_with(|| {
                DestinationLatestSnapshot::new_with_launch_window(
                    title.clone(),
                    source,
                    items.clone(),
                    self.app.home_latest_launch_window,
                )
            });
        snapshot.title = title;
        snapshot.items = items;
        recompute_destination_latest_marker(
            snapshot,
            self.app.home_latest_launch_window,
            &self.acknowledged_home_latest_sources,
        );
        let items = snapshot
            .items
            .iter()
            .filter_map(QueueItem::as_emby)
            .cloned()
            .collect::<Vec<_>>();
        for library in &mut self.app.libs {
            if library.library.id != library_id || library.library.collection_type != "tvshows" {
                continue;
            }
            let Some(level) = library.nav_stack.last_mut() else {
                continue;
            };
            if level.tv_content_mode != Some(mbv_core::config::TvContentMode::Latest) {
                continue;
            }
            level.items.clone_from(&items);
            level.fetched_rows = items.len();
            level.total_count = items.len();
            level.loading = false;
        }
        self.push_active_emby_library_owner_content();
    }

    pub(super) fn clear_home_content(&mut self) {
        self.home_content.continue_items.clear();
        self.push_home_content();
    }

    pub(super) fn home_stable_target(
        &self,
        target: &super::components::msg::HomeRowTarget,
    ) -> Option<(QueueItem, bool)> {
        if !target.from_continue_watching {
            return None;
        }
        let item_id = target.item_id.as_deref()?;
        self.home_content
            .continue_items
            .iter()
            .find(|item| item.id == item_id)
            .cloned()
            .map(|item| (QueueItem::Emby(Box::new(item)), true))
    }

    pub(super) fn fetch_home_at_startup(&mut self) {
        let fetched_home = self.app.fetch_home();
        self.home_content.loading = false;
        match fetched_home {
            Ok(content) => {
                let has_live_flash = self.app.status_expires.is_some_and(|t| t > Instant::now());
                if !has_live_flash {
                    self.app.status.clear();
                }
                self.assign_home_content(content);
            }
            Err(error) => {
                self.app.flash(
                    format!("Couldn't load home: {error}"),
                    ToastSeverity::Warning,
                );
                self.push_home_content();
            }
        }
    }

    pub(super) fn apply_emby_completion_drain(
        &mut self,
        completion: super::service_startup::Completion,
    ) {
        if let Some(content) = self.app.apply_emby_completion(completion) {
            self.assign_home_content(content);
        }
    }

    pub(super) fn apply_emby_setup_completion_drain(
        &mut self,
        completion: super::service_startup::SetupCompletion,
    ) {
        if let Some(content) = self.app.apply_emby_setup_completion(completion) {
            self.assign_home_content(content);
        }
    }

    pub(super) fn home_continue_watching_selected(&self) -> bool {
        true
    }

    #[cfg(test)]
    pub(super) fn home_owner_shared(&self) -> Option<&HomeOwner> {
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .and_then(|panel| panel.owner(&LibraryKey::Home))
            .and_then(|owner| owner.as_any().downcast_ref::<HomeOwner>())
    }

    pub(super) fn acknowledge_home_latest(&mut self, source: DestinationLatestSource) {
        self.record_home_latest_acknowledgement(source);
        self.push_tv_workspace_content();
    }

    pub(super) fn record_home_latest_acknowledgement(&mut self, source: DestinationLatestSource) {
        self.acknowledged_home_latest_sources.insert(source.clone());
        if let DestinationLatestSource::Emby(library_id) = source {
            if let Some(snapshot) = self.tv_latest_snapshots.get_mut(&library_id) {
                snapshot.has_new_content = false;
            }
        }
    }
}

fn recompute_destination_latest_marker(
    snapshot: &mut DestinationLatestSnapshot,
    launch_window: crate::app::state::home_latest::HomeLatestLaunchWindow,
    acknowledged: &std::collections::HashSet<DestinationLatestSource>,
) {
    snapshot.recompute_new_content(launch_window);
    if acknowledged.contains(&snapshot.source) {
        snapshot.has_new_content = false;
    }
}
