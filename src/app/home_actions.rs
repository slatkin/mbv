use super::notify_actions::ToastSeverity;
use super::types_playback::HomeLatestSource;
use super::ui_util::is_playable;
use super::App;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;

impl App {
    // ── Home flat list ───────────────────────────────────────────────────────

    /// The QueueItem at the given flat `cursor` (the Home component's
    /// target index), or None. The caller supplies the cursor — the effect
    /// never reads an App-owned cursor, so the component's target is honored
    /// even when App's remaining section-specific state differs (task 5.3d,
    /// Home typed-effect prep + cursor deletion).
    pub(super) fn home_current_item(&self, cursor: usize) -> Option<QueueItem> {
        let mut pos = 0usize;
        for item in &self.home.continue_items {
            if pos == cursor {
                return Some(QueueItem::Emby(Box::new(item.clone())));
            }
            pos += 1;
        }
        for (_, _, items, _) in &self.home.latest {
            for item in items {
                if pos == cursor {
                    return Some(item.clone());
                }
                pos += 1;
            }
        }
        None
    }

    fn home_new_sections(&self) -> Vec<usize> {
        // Every section in `home.latest` is a selectable pill (an ABS library,
        // an Emby view, or Feeds), empty or not — matching Continue Watching.
        // An empty section renders as an "(empty)" row rather than vanishing.
        (0..self.home.latest.len()).map(|idx| idx + 1).collect()
    }

    /// Whether `section_idx` is a selectable Home pill: section 0 (Continue
    /// Watching) is always valid, and any other index is valid iff it maps to
    /// a section in `home.latest` (even an empty one).
    pub(super) fn home_section_is_valid(&self, section_idx: usize) -> bool {
        section_idx == 0 || self.home_new_sections().contains(&section_idx)
    }

    /// The semantic identity currently selected by `home.section`: section 0
    /// (Continue Watching) has no `latest` entry and resolves to `None` (the
    /// empty-string persistence sentinel); any real pill resolves to its
    /// `HomeLatestSource` (task 5.3d). Resolving by section here keeps the
    /// off-by-one rule in one place; `home_section_pref()` persists the
    /// resolved identity, never the numeric index.
    pub(super) fn home_section_identity(&self) -> Option<HomeLatestSource> {
        if self.home.section == 0 {
            return None;
        }
        self.home
            .latest
            .get(self.home.section - 1)
            .map(|(_, source, _, _)| source.clone())
    }

    /// Stash the identity resolved from the current `home.section` into the
    /// shell-owned semantic preference, so an unrelated `save_prefs()` reads a
    /// stable identity instead of deriving it from the numeric section state
    /// (which is soon deleted, task 5.3d). Call after every numeric
    /// `home.section` write: explicit selection, one-time persisted
    /// restoration, and both asynchronous section rebuild/clamp paths.
    pub(super) fn update_home_section_pref(&mut self) {
        self.home_section_pref_semantic = self.home_section_identity();
    }

    /// Async section rebuild/clamp variant of `update_home_section_pref`:
    /// update the semantic preference only when no one-time persisted restore
    /// is still pending. While a pending source has not arrived, the numeric
    /// `home.section` is still 0 (Continue Watching), so resolving it here
    /// would clear the loaded semantic source and let an unrelated
    /// `save_prefs()` overwrite it before restoration (task 5.3d). Once
    /// restoration succeeds (`home_section_pending` clears), later clamps
    /// track the resulting source again. Shared by both async clamp sites.
    pub(super) fn update_home_section_pref_guarded(&mut self) {
        if self.home_section_pending.is_none() {
            self.update_home_section_pref();
        }
    }

    pub(super) fn home_select_section(&mut self, section_idx: usize) {
        let section_idx = if self.home_section_is_valid(section_idx) {
            section_idx
        } else if let Some(first) = self.home_new_sections().first() {
            *first
        } else {
            self.home.section = 0;
            self.home_section_pref_semantic = None;
            return;
        };
        self.home.section = section_idx;
        self.update_home_section_pref();
        // Persist the selection so the pill is restored on the next launch.
        self.save_prefs();
    }

    /// Play the item at the component-provided flat `cursor`. Uses the
    /// supplied target directly instead of any App-owned cursor, so the
    /// request's own target is honored (task 5.3d, Home typed-effect prep + cursor deletion).
    pub(super) fn home_play(&mut self, cursor: usize) {
        let Some(item) = self.home_current_item(cursor) else {
            return;
        };
        if matches!(&item, QueueItem::Emby(inner) if inner.is_folder) {
            return;
        }
        let cw_len = self.home.continue_items.len();
        if cursor < cw_len {
            // CW items resume through the item-targeted tail with the
            // already-resolved item (task 5.3d, Home effect decoupling)
            // instead of temporarily forcing `home.section` to 0 and
            // re-reading it via `select_home`.
            if let QueueItem::Emby(item) = item {
                self.play_home_cw_item(*item);
            }
        } else {
            match item {
                QueueItem::Emby(item) => self.play_item(*item),
                // Non-Emby `latest` items (Audiobookshelf today, Feeds in
                // Part 3) submit through the shared helper, which neither
                // reads nor mutates the owning tab's cursor/filter.
                other => {
                    self.submit_queue_item(other, true);
                }
            }
        }
    }

    /// Enqueue the item at the component-provided flat `cursor` (task 5.3d,
    /// Home typed-effect prep). Uses the supplied target directly instead of
    /// any App-owned cursor.
    pub(super) fn home_enqueue(&mut self, cursor: usize) {
        let Some(item) = self.home_current_item(cursor) else {
            return;
        };
        let cw_len = self.home.continue_items.len();
        if cursor < cw_len {
            // CW items enqueue through the item-targeted helper with the
            // already-resolved item (task 5.3d, Home effect decoupling)
            // instead of temporarily forcing `home.section` to 0 and
            // re-reading it via `enqueue_selected(None)`.
            if let QueueItem::Emby(item) = item {
                self.enqueue_home_item(*item);
            }
        } else {
            match item {
                QueueItem::Emby(item) => self.do_enqueue_folder(*item),
                other => {
                    self.submit_queue_item(other, false);
                }
            }
        }
    }

    /// Resume the given Continue Watching Emby item (task 5.3d, Home effect
    /// decoupling): the playable tail of the deleted `select_home`, extracted
    /// so the Home and CW effects pass an already-resolved item directly
    /// instead of temporarily pointing `home.section` at section 0 and
    /// re-reading it. Folder guards stay with the callers
    /// (`home_play`/`cw_play` pre-filter them, matching today's reachable
    /// behavior); non-playable items are a silent no-op, as in `select_home`.
    pub(super) fn play_home_cw_item(&mut self, item: EmbyItem) {
        if !is_playable(&item) {
            return;
        }
        let fresh = {
            let Some(client) = self.emby_client() else {
                self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
                return;
            };
            let c = client.lock().unwrap();
            c.get_items_by_ids(std::slice::from_ref(&item.id))
                .ok()
                .and_then(|mut v| {
                    if v.is_empty() {
                        None
                    } else {
                        Some(v.remove(0))
                    }
                })
                .unwrap_or(item)
        };
        self.play_item(fresh);
    }

    /// Remove the Continue Watching item at the component-provided flat
    /// `cursor` from the resume row, guarding on the CW range exactly like the
    /// legacy Delete arm. Non-CW cursors are ignored, and the
    /// CW column cursor is saved/restored around the removal just like the
    /// legacy arm (task 5.3d, Home typed-effect prep).
    pub(super) fn home_delete(&mut self, cursor: usize) {
        let cw_len = self.home.continue_items.len();
        if cursor >= cw_len {
            return;
        }
        let saved = self.home.continue_cursor;
        self.home.continue_cursor = cursor;
        self.remove_from_continue_watching();
        self.home.continue_cursor = saved;
    }
}
