//! Model-owned Home content state methods (task 5.3d): `Model.home_content`
//! is the sole Home content owner; App-internal writers compute fresh
//! snapshots and deliver them through lib_tx, and these methods assign,
//! merge, resolve, or project them. `HomeContent` itself lives in
//! `types_playback.rs`; this file keeps the shell-side state transitions out
//! of the near-cap `shell.rs`/`shell_home.rs`.

use super::components::home_content::HomeContent as HomeOwner;
use super::components::library_panel::LibraryKey;
use super::components::library_panel::LibraryPanel;
use super::components::ComponentId;
use super::notify_actions::ToastSeverity;
use super::shell::Model;
use super::types_playback::{HomeContent, HomeLatestSection, HomeLatestSource};
use mbv_core::playback_queue::QueueItem;
use std::collections::HashMap;
use std::time::Instant;

impl Model {
    /// Assign a freshly computed Home content snapshot (from `fetch_home`,
    /// `apply_emby_bootstrap`, or a lib_tx-delivered App-side computation)
    /// and re-project it into `HomeComponent`. The Continue Watching column
    /// cursor is a user-visible selection owned by `home_content`: a content
    /// refresh never resets it — it survives verbatim, exactly like the
    /// legacy `fetch_home`, which never touched `home.continue_cursor`. The
    /// content's own `loading` flag (always `false` for a completed
    /// computation) is authoritative, so an assigned refresh also clears a
    /// pending startup skeleton.
    pub(super) fn assign_home_content(&mut self, mut content: HomeContent) {
        content.feed_names = self.resolve_home_feed_names(&content);
        recompute_home_latest_markers(&mut content, self.app.home_latest_launch_window);
        self.merge_tv_latest_snapshots_from_home(&mut content);
        self.home_content = content;
        self.project_tv_latest_snapshots_to_libraries();
        self.push_home_content();
    }

    fn merge_tv_latest_snapshots_from_home(&mut self, content: &mut HomeContent) {
        for section in &mut content.latest {
            let super::types_playback::HomeLatestSource::Emby(library_id) = &section.source else {
                continue;
            };
            if !self.app.libs.iter().any(|lib| {
                lib.library.id == *library_id && lib.library.collection_type == "tvshows"
            }) {
                continue;
            }
            let snapshot = self
                .tv_latest_snapshots
                .entry(library_id.clone())
                .or_insert_with(|| section.clone());
            snapshot.title.clone_from(&section.title);
            snapshot.items.clone_from(&section.items);
            section.items.clone_from(&snapshot.items);
            section.has_new_content = snapshot.has_new_content;
        }
    }

    pub(super) fn update_tv_latest_snapshot(
        &mut self,
        library_id: String,
        title: String,
        items: Vec<mbv_core::playback_queue::QueueItem>,
    ) {
        let source = HomeLatestSource::Emby(library_id.clone());
        let existing_section = self
            .home_content
            .latest
            .iter()
            .find(|section| section.source == source)
            .cloned();
        let snapshot = self
            .tv_latest_snapshots
            .entry(library_id)
            .or_insert_with(|| {
                existing_section.unwrap_or_else(|| {
                    HomeLatestSection::new_with_launch_window(
                        title.clone(),
                        source.clone(),
                        items.clone(),
                        self.app.home_latest_launch_window,
                    )
                })
            });
        snapshot.title = title;
        snapshot.items = items;
        if let Some(section) = self
            .home_content
            .latest
            .iter_mut()
            .find(|section| section.source == source)
        {
            section.title.clone_from(&snapshot.title);
            section.items.clone_from(&snapshot.items);
            section.has_new_content = snapshot.has_new_content;
        }
        self.project_tv_latest_snapshots_to_libraries();
        self.push_home_content();
    }

    fn project_tv_latest_snapshots_to_libraries(&mut self) {
        for lib in &mut self.app.libs {
            if lib.library.collection_type != "tvshows" {
                continue;
            }
            let Some(level) = lib.nav_stack.last_mut() else {
                continue;
            };
            if level.tv_content_mode != Some(mbv_core::config::TvContentMode::Latest) {
                continue;
            }
            let Some(snapshot) = self.tv_latest_snapshots.get(&lib.library.id) else {
                continue;
            };
            level.items = snapshot
                .items
                .iter()
                .filter_map(|item| item.as_emby().cloned())
                .collect();
            level.fetched_rows = level.items.len();
            level.total_count = level.items.len();
            level.loading = false;
        }
    }

    /// Shell-side feed-name resolution for a Home snapshot (design D2):
    /// `Config` never crosses into a component, so the shell resolves feed
    /// display names at assignment time (the App layer's subscription match)
    /// for the component's projection to look up by `feed_id`.
    pub(super) fn resolve_home_feed_names(&self, content: &HomeContent) -> HashMap<String, String> {
        content
            .latest
            .iter()
            .flat_map(|section| &section.items)
            .filter_map(|item| item.as_feed())
            .filter_map(|entry| {
                Some((
                    entry.feed_id.as_deref()?.to_string(),
                    self.app.feed_subscription_display_name(entry)?,
                ))
            })
            .collect()
    }

    /// Reset Home content after an Emby removal/replacement
    /// (`LibEvent::HomeContentCleared`, task 5.3d): wipes Continue Watching
    /// items, the column cursor, and every pill. The `loading` flag is
    /// intentionally left alone, matching the legacy `clear_emby_memory`
    /// which never reset it.
    pub(super) fn clear_home_content(&mut self) {
        self.home_content.continue_items.clear();
        self.home_content.latest.clear();
        self.home_content.feed_names.clear();
        self.push_home_content();
    }

    /// Merge freshly computed Audiobookshelf pill sections into the
    /// Model-owned `latest` (the shared cross-provider splice canonicalizes
    /// pill order and preserves cursors) and re-project. Delivered from
    /// `LibEvent::AudiobookshelfLatestRebuilt` (task 5.3d).
    pub(super) fn merge_home_abs_sections(&mut self, sections: Vec<HomeLatestSection>) {
        super::library_load_actions::merge_home_sections(
            &mut self.home_content.latest,
            sections,
            |source| {
                matches!(
                    source,
                    super::types_playback::HomeLatestSource::Audiobookshelf(_)
                )
            },
        );
        recompute_home_latest_markers(&mut self.home_content, self.app.home_latest_launch_window);
        self.push_home_content();
    }

    /// Merge freshly computed Feeds Latest pill sections (at most one) into
    /// the Model-owned `latest` (the shared cross-provider splice canonicalizes
    /// pill order and preserves cursors) and re-project. Delivered from
    /// `LibEvent::FeedsLatestRebuilt` at the lib_rx drain (task 5.3d).
    pub(super) fn merge_home_feeds_sections(&mut self, sections: Vec<HomeLatestSection>) {
        super::library_load_actions::merge_home_sections(
            &mut self.home_content.latest,
            sections,
            |source| matches!(source, super::types_playback::HomeLatestSource::Feeds),
        );
        // Merged feed sections never pass through `assign_home_content`, so
        // re-resolve the lookup over the merged content (design D2).
        self.home_content.feed_names = self.resolve_home_feed_names(&self.home_content);
        recompute_home_latest_markers(&mut self.home_content, self.app.home_latest_launch_window);
        self.push_home_content();
    }

    /// Resolve the Home component's flat target index (the component owns
    /// the flat cursor) against Model-owned content (task 5.3d): Continue
    /// Watching rows lead, per-pill items follow in canonical pill order —
    /// the same flat layout `HomeComponent` renders (`section_range`).
    /// Returns the item and whether it came from Continue Watching, so the
    /// App effect keeps the CW-vs-`latest` distinction with an explicit
    /// target (never a re-read App cursor).
    pub(super) fn home_stable_target(
        &self,
        target: &super::components::msg::HomeRowTarget,
    ) -> Option<(QueueItem, bool)> {
        if target.from_continue_watching {
            let item_id = target.item_id.as_deref()?;
            return self
                .home_content
                .continue_items
                .iter()
                .find(|item| item.id == item_id)
                .cloned()
                .map(|item| (QueueItem::Emby(Box::new(item)), true));
        }
        self.home_content
            .latest
            .iter()
            .find(|section| target.source.as_deref() == Some(section.source.pref_key().as_str()))
            .and_then(|section| {
                section
                    .items
                    .iter()
                    .find(|item| Some(item.id()) == target.item_id.as_deref())
                    .cloned()
            })
            .map(|item| (item, false))
    }

    /// Synchronous startup/commit fetch drain for `fetch_home` (task 5.3d):
    /// the fetch itself is never deferred — its App-side side effects are
    /// order-sensitive — and the shell owns the computed content, so this
    /// assigns it to `home_content` (preserving the CW cursor) and
    /// re-projects. `loading` clears even on error, matching the legacy
    /// unconditional `home_loading = false` after the startup fetch.
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
            Err(e) => {
                self.app
                    .flash(format!("Couldn't load home: {e}"), ToastSeverity::Warning);
                self.push_home_content();
            }
        }
    }

    /// Emby startup-completion drain (task 5.3d): apply the completion
    /// (bootstrap → fresh content over the current Model-owned latest) and
    /// assign + re-project; a stale/error completion returns `None` and the
    /// unchanged content is re-projected idempotently — the seam contract the
    /// pre-5.3d drain had with its `push_home_content` call.
    pub(super) fn apply_emby_completion_drain(
        &mut self,
        completion: super::service_startup::Completion,
    ) {
        if let Some(content) = self
            .app
            .apply_emby_completion(completion, &self.home_content.latest)
        {
            self.assign_home_content(content);
        } else {
            self.push_home_content();
        }
    }

    /// Emby setup-completion drain (task 5.3d): same assign/re-project
    /// contract as `apply_emby_completion_drain`, for the setup path.
    pub(super) fn apply_emby_setup_completion_drain(
        &mut self,
        completion: super::service_startup::SetupCompletion,
    ) {
        if let Some(content) = self
            .app
            .apply_emby_setup_completion(completion, &self.home_content.latest)
        {
            self.assign_home_content(content);
        } else {
            self.push_home_content();
        }
    }

    /// The authoritative "is Continue Watching selected?" fact for the
    /// context-menu builder and keyboard thread, resolved at the Model
    /// boundary from the mounted `HomeComponent` (task 5.3d): reading
    /// `HomeComponent::section() == 0` here replaces the deleted numeric
    /// `App.home.section == 0` read; the value is passed into the App-owned
    /// builder and never copied into a new App field. With no mounted Home
    /// component the fact defaults to `false` (Home is mounted for the whole
    /// session, so this is only a defensive fallback).
    pub(super) fn home_continue_watching_selected(&self) -> bool {
        self.home_owner_shared()
            .map(|owner| owner.section())
            .map(|section| section == 0)
            .unwrap_or(false)
    }

    /// The Home owner for shared reads, through the panel's owner map
    /// (design D2: addressed by `LibraryKey`, never by a destination
    /// component). `None` before the first `push_home_content` installs it.
    pub(super) fn home_owner_shared(&self) -> Option<&HomeOwner> {
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|c| c.as_any().downcast_ref::<LibraryPanel>())
            .and_then(|panel| panel.owner(&LibraryKey::Home))
            .and_then(|owner| owner.as_any().downcast_ref::<HomeOwner>())
    }

    /// Resolve a component-owned numeric section to the semantic Home source
    /// identity retained in memory for the current session. Continue Watching
    /// (section 0) resolves to `None`; a missing component is a defensive no-op.
    pub(super) fn select_home_section_from_component(&mut self, section: usize) {
        let Some(source) = self
            .home_owner_shared()
            .map(|home| home.source_for_section(section))
        else {
            return;
        };
        if self.home_section_pref_semantic != source {
            self.home_section_pref_semantic = source;
        }
    }

    /// Acknowledge a Home Latest section from either surface. The set is
    /// shell-owned, then both mounted owners are re-projected so their
    /// markers clear in the same tick. This deliberately updates the Home
    /// owner directly instead of pushing its content: a direct shell request
    /// may precede the component's local selection projection.
    pub(super) fn acknowledge_home_latest(&mut self, source: HomeLatestSource) {
        self.acknowledged_home_latest_sources.insert(source);
        let acknowledged = self.acknowledged_home_latest_sources.clone();
        self.update_home_owner(|home| home.set_acknowledged_latest_sources(&acknowledged));
        self.push_tv_workspace_content();
    }

    /// Test-only accessor for the retained semantic source. Gated to avoid a
    /// dead-code warning in
    /// non-test builds (task 5.3d, 2c cleanup).
    #[cfg(test)]
    pub(super) fn home_section_pref(&self) -> String {
        self.home_section_pref_semantic
            .as_ref()
            .map(super::types_playback::HomeLatestSource::pref_key)
            .unwrap_or_default()
    }
}

fn recompute_home_latest_markers(
    content: &mut HomeContent,
    launch_window: super::home_latest::HomeLatestLaunchWindow,
) {
    for section in &mut content.latest {
        section.recompute_new_content(launch_window);
    }
}
