//! Model-owned Home content state methods (task 5.3d): `Model.home_content`
//! is the sole Home content owner; App-internal writers compute fresh
//! snapshots and deliver them through lib_tx, and these methods assign,
//! merge, resolve, or project them. `HomeContent` itself lives in
//! `types_playback.rs`; this file keeps the shell-side state transitions out
//! of the near-cap `shell.rs`/`shell_home.rs`.

use super::components::{ComponentId, HomeComponent};
use super::notify_actions::ToastSeverity;
use super::shell::Model;
use super::types_playback::HomeContent;
use super::ui_util::move_cursor;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
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
    pub(super) fn assign_home_content(&mut self, content: HomeContent) {
        let old_cursor = self.home_content.continue_cursor;
        self.home_content = content;
        self.home_content.continue_cursor = old_cursor;
        self.push_home_content();
    }

    /// Reset Home content after an Emby removal/replacement
    /// (`LibEvent::HomeContentCleared`, task 5.3d): wipes Continue Watching
    /// items, the column cursor, and every pill. The `loading` flag is
    /// intentionally left alone, matching the legacy `clear_emby_memory`
    /// which never reset it.
    pub(super) fn clear_home_content(&mut self) {
        self.home_content.continue_items.clear();
        self.home_content.latest.clear();
        self.home_content.continue_cursor = 0;
        self.push_home_content();
    }

    /// Merge freshly computed Audiobookshelf pill sections into the
    /// Model-owned `latest` (the shared cross-provider splice canonicalizes
    /// pill order and preserves cursors) and re-project. Delivered from
    /// `LibEvent::AudiobookshelfLatestRebuilt` (task 5.3d).
    pub(super) fn merge_home_abs_sections(
        &mut self,
        sections: Vec<(
            String,
            super::types_playback::HomeLatestSource,
            Vec<QueueItem>,
        )>,
    ) {
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
        self.push_home_content();
    }

    /// Merge freshly computed Feeds Latest pill sections (at most one) into
    /// the Model-owned `latest` (the shared cross-provider splice canonicalizes
    /// pill order and preserves cursors) and re-project. Delivered from
    /// `LibEvent::FeedsLatestRebuilt` at the lib_rx drain (task 5.3d).
    pub(super) fn merge_home_feeds_sections(
        &mut self,
        sections: Vec<(
            String,
            super::types_playback::HomeLatestSource,
            Vec<QueueItem>,
        )>,
    ) {
        super::library_load_actions::merge_home_sections(
            &mut self.home_content.latest,
            sections,
            |source| matches!(source, super::types_playback::HomeLatestSource::Feeds),
        );
        self.push_home_content();
    }

    /// Resolve the Home component's flat target index (the component owns
    /// the flat cursor) against Model-owned content (task 5.3d): Continue
    /// Watching rows lead, per-pill items follow in canonical pill order —
    /// the same flat layout `HomeComponent` renders (`section_range`).
    /// Returns the item and whether it came from Continue Watching, so the
    /// App effect keeps the CW-vs-`latest` distinction with an explicit
    /// target (never a re-read App cursor).
    pub(super) fn home_flat_target(&self, cursor: usize) -> Option<(QueueItem, bool)> {
        let mut pos = 0usize;
        for item in &self.home_content.continue_items {
            if pos == cursor {
                return Some((QueueItem::Emby(Box::new(item.clone())), true));
            }
            pos += 1;
        }
        for (_, _, items, _) in &self.home_content.latest {
            for item in items {
                if pos == cursor {
                    return Some((item.clone(), false));
                }
                pos += 1;
            }
        }
        None
    }

    /// The Continue Watching column item under the column's own
    /// `continue_cursor` (Model-owned, task 5.3d) — the authoritative target
    /// for the CW effects (`cw_play`/`cw_enqueue`/`cw_toggle_watched`), the
    /// context-menu Home/queue-coupling arms, and the keyboard-threaded
    /// `cw_item` (§5.3d input thread).
    pub(super) fn home_cw_item(&self) -> Option<EmbyItem> {
        self.home_content
            .continue_items
            .get(self.home_content.continue_cursor)
            .cloned()
    }

    /// Move the Continue Watching column cursor — the preserved legacy wheel
    /// quirk (task 5.3d): identical clamp/`ui_util::move_cursor` semantics to
    /// the deleted `App::cw_move_cursor`, operating on Model-owned
    /// `home_content.continue_cursor`. The mounted component's section-local
    /// cursor is moved separately in `handle_home_scroll`.
    pub(super) fn cw_move_cursor(&mut self, delta: i64) {
        let n = self.home_content.continue_items.len();
        if n == 0 {
            return;
        }
        let cur = self.home_content.continue_cursor.min(n - 1);
        self.home_content.continue_cursor = move_cursor(cur, delta, n);
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
        self.application
            .get_component(&ComponentId::Home)
            .and_then(|c| c.as_any().downcast_ref::<HomeComponent>())
            .map(HomeComponent::section)
            .map(|section| section == 0)
            .unwrap_or(false)
    }

    /// Persist a selected section's semantic source (task 5.3d, numeric Home
    /// section deletion): `[`/`]`/pill selection arrives here as a numeric
    /// section; the component owns that number, so the shell maps it to its
    /// `HomeLatestSource` via `source_for_section`, stores it in the
    /// shell-owned semantic preference, and persists through the unchanged
    /// `App::save_prefs`. Continue Watching (section 0) resolves to `None`
    /// (the empty-string sentinel); a missing component is a defensive no-op.
    pub(super) fn select_home_section_from_component(&mut self, section: usize) {
        let Some(source) = self
            .application
            .get_component(&ComponentId::Home)
            .and_then(|c| c.as_any().downcast_ref::<HomeComponent>())
            .map(|home| home.source_for_section(section))
        else {
            return;
        };
        if self.home_section_pref_semantic != source {
            self.home_section_pref_semantic = source;
            self.persist_home_section_pref();
        }
    }

    pub(super) fn home_section_pref(&self) -> String {
        self.home_section_pref_semantic
            .as_ref()
            .map(super::types_playback::HomeLatestSource::pref_key)
            .unwrap_or_default()
    }

    pub(super) fn persist_home_section_pref(&self) {
        let path = crate::config::prefs_path();
        let mut prefs = super::App::load_prefs();
        if !prefs.is_object() {
            prefs = serde_json::json!({});
        }
        let value = self
            .home_section_pref_semantic
            .as_ref()
            .map(super::types_playback::HomeLatestSource::pref_key)
            .unwrap_or_default();
        if prefs
            .get("home_section")
            .and_then(serde_json::Value::as_str)
            == Some(value.as_str())
        {
            return;
        }
        prefs["home_section"] = serde_json::Value::String(value);
        if let Ok(serialized) = serde_json::to_string(&prefs) {
            let _ = std::fs::write(path, serialized);
        }
    }
}
