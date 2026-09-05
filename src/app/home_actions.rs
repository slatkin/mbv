use super::notify_actions::ToastSeverity;
use super::ui_util::is_playable;
use super::App;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;

impl App {
    // ── Home flat list ───────────────────────────────────────────────────────

    /// Play the Home flat-list item the shell resolved at the Model boundary
    /// (task 5.3d, Home typed-effect prep + re-homing): the effect acts on
    /// the supplied item directly, never on a re-read App cursor. `from_cw`
    /// distinguishes Continue Watching rows (resumed through the
    /// item-targeted tail) from `latest` pills (played directly, or
    /// submitted through the shared non-Emby helper). The folder guard
    /// mirrors `home_play`'s early return.
    pub(super) fn home_play_target(&mut self, item: QueueItem, from_cw: bool) {
        if matches!(&item, QueueItem::Emby(inner) if inner.is_folder) {
            return;
        }
        if from_cw {
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

    /// Enqueue the Home flat-list item the shell resolved at the Model
    /// boundary (task 5.3d, Home typed-effect prep). Uses the supplied item
    /// directly instead of any App-owned cursor.
    pub(super) fn home_enqueue_target(&mut self, item: QueueItem, from_cw: bool) {
        if from_cw {
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
    /// (`home_play_target`/`cw_play` pre-filter them, matching today's
    /// reachable behavior); non-playable items are a silent no-op, as in
    /// `select_home`.
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
}
