use crate::app::{App, SessionEvent};
use mbv_core::api::{EmbyClient, TICKS_PER_SECOND};
impl App {
    pub(in crate::app) fn submit_attached_sequence(
        &mut self,
        conn_id: &str,
        items: &[mbv_core::api::EmbyItem],
        start_idx: usize,
    ) {
        let id = conn_id.to_string();
        let item_ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
        let start_ticks = items
            .get(start_idx)
            .map_or(0, |item| item.playback_position_ticks);
        self.do_session_command(move |client| {
            client.session_play_items(&id, &item_ids, start_idx, start_ticks)
        });
    }

    /// Advances whenever queue identity or state is replaced, so in-flight
    /// playlist mutations can detect stale completions.
    pub(in crate::app) fn advance_queue_epoch(&mut self) {
        self.queue_epoch.advance();
    }

    pub(in crate::app) fn spawn_sessions_load(&mut self) {
        self.sessions_loading = true;
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.sessions_tx.clone();
        std::thread::spawn(move || match client.get_sessions() {
            Ok(sessions) => {
                let _ = tx.send(SessionEvent::Loaded { sessions });
            }
            Err(e) => {
                let _ = tx.send(SessionEvent::Error(e));
            }
        });
    }

    pub(in crate::app) fn session_jump_track(
        &mut self,
        conn_id: &str,
        delta: i64,
        fallback_cmd: &'static str,
    ) {
        self.clear_playback_overlays();
        let id = conn_id.to_string();
        let current_remote_id = self
            .connected_session_state
            .as_ref()
            .and_then(|s| s.now_playing_item_id.as_deref());
        // Resolve the destination and payload directly from the visible queue.
        let Some((target_idx, _)) = remote_jump_target(&self.player_tab, current_remote_id, delta)
        else {
            self.do_session_command(move |c| c.session_transport(&id, fallback_cmd));
            return;
        };
        let emby_items = self.player_tab.emby_items();
        // Remap the canonical queue index to the Emby-only item index used by
        // the session API.
        let emby_start = self
            .player_tab
            .queue
            .slots()
            .iter()
            .take(target_idx)
            .filter(|s| s.item.as_emby().is_some())
            .count();
        self.submit_attached_sequence(&id, &emby_items, emby_start);
    }

    /// Compute the absolute tick position for a remote-session seek, given
    /// the current position in seconds and a relative delta in seconds.
    ///
    /// This reconstructs the asymmetric math the old inline remote-session
    /// `<`/`>` handlers in `input.rs` had: rewinding (`delta < 0`) clamps at
    /// zero, fast-forwarding does not (matching the prior
    /// `(pos_s - 5).max(0)` vs. `(pos_s + 5)`). Used by `action::dispatch`'s
    /// `Action::SeekRelative` arm; kept here alongside its sibling
    /// session-math helpers (`session_jump_track`, `do_session_command`)
    /// rather than in `action.rs`, since it's pure session-position math with
    /// no dependency on the `Action` seam itself.
    pub(in crate::app) fn remote_seek_ticks(pos_s: i64, delta: f64) -> i64 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "seconds↔ticks conversion through f64; no lossless integer-path conversion exists (approved, issue #804)"
        )]
        let moved = pos_s + delta as i64;
        let target = if delta < 0.0 { moved.max(0) } else { moved };
        target * TICKS_PER_SECOND
    }

    pub(in crate::app) fn clear_playback_overlays(&mut self) {
        self.next_up_item = None;
        self.status.clear();
    }

    pub(in crate::app) fn do_session_command(
        &mut self,
        f: impl FnOnce(&EmbyClient) -> Result<(), String> + Send + 'static,
    ) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.sessions_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = f(&client) {
                let _ = tx.send(SessionEvent::CommandError { error: e });
                return;
            }
            // Refresh the directly observed Session state after a successful command.
            match client.get_sessions() {
                Ok(sessions) => {
                    let _ = tx.send(SessionEvent::Loaded { sessions });
                }
                Err(e) => {
                    let _ = tx.send(SessionEvent::Error(e));
                }
            }
        });
    }
}

/// Resolve a remote Next/Previous destination by locating the connected
/// session's now-playing item in the visible queue and applying the delta.
pub(in crate::app) fn remote_jump_target(
    player_tab: &crate::app::PlayerTab,
    now_playing_item_id: Option<&str>,
    delta: i64,
) -> Option<(usize, i64)> {
    let current = now_playing_item_id.and_then(|rid| {
        player_tab
            .queue
            .slots()
            .iter()
            .position(|s| s.item.id() == rid)
    })?;
    let t = i128::try_from(current)
        .ok()?
        .checked_add(i128::from(delta))?;
    let t = usize::try_from(t).ok()?;
    if t >= player_tab.total_queue_len() {
        return None;
    }
    Some((
        t,
        player_tab
            .emby_item_at(t)
            .map_or(0, |i| i.playback_position_ticks),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_jump_target_moves_to_adjacent_queue_item() {
        let player = crate::app::PlayerTab::from_emby_items(crate::app::tests::make_items(3), 0);
        assert_eq!(
            remote_jump_target(&player, Some("id1"), 1).map(|(index, _)| index),
            Some(2)
        );
        assert_eq!(
            remote_jump_target(&player, Some("id1"), -1).map(|(index, _)| index),
            Some(0)
        );
    }
}
