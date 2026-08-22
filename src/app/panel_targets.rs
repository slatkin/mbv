// The F3 target panel's mixed Emby+Cast target list (8.1/8.2). `App` holds
// the two raw sources it is built from -- `sessions` (Emby, `/Sessions`) and
// `cast_receivers` (cast discovery, `cast_actions::spawn_cast_discovery`) --
// separately, since each is refreshed on its own channel/cadence; this
// module's job is only the merge.

use mbv_core::api::SessionInfo;
use mbv_core::cast_discovery::CastReceiver;

/// One row in the F3 target panel. The channel that produced a target
/// determines how mbv controls it (design.md "Discovery is a second channel
/// beside `/Sessions`"), so a device reachable on both channels is two
/// distinct rows rather than one deduplicated entry.
///
/// `Emby` is boxed: `SessionInfo` is far larger than `CastReceiver` (9.2),
/// and `PanelTarget` lives in `App.panel_targets`, a `Vec` rebuilt on every
/// panel refresh -- unlike `render/components/home.rs`'s `HeroContentDims`
/// (the lint's other occurrence), which is a single per-render stack local,
/// not a persisted collection element, so it was left unboxed there.
#[derive(Clone)]
pub(super) enum PanelTarget {
    Emby(Box<SessionInfo>),
    Cast(CastReceiver),
}

/// A `PanelTarget`'s identity, used only to re-locate the previously
/// selected row after a rebuild (`App::rebuild_panel_targets`) -- the two
/// channels' identifiers live in separate namespaces, so tagging by kind
/// keeps an Emby session id from ever being confused with a cast receiver
/// id that happens to collide.
#[derive(PartialEq, Eq)]
enum PanelTargetKey {
    Emby(String),
    Cast(String),
}

fn panel_target_key(target: &PanelTarget) -> PanelTargetKey {
    match target {
        PanelTarget::Emby(session) => PanelTargetKey::Emby(session.id.clone()),
        PanelTarget::Cast(receiver) => PanelTargetKey::Cast(receiver.id.clone()),
    }
}

/// Concatenates Emby sessions and discovered cast receivers into one list,
/// Emby first: no dedup, no ordering decision beyond "which channel arrived
/// first" (8.2). Pure and side-effect free so it is testable without a
/// running panel or a network call (8.1).
pub(super) fn build_panel_targets(
    sessions: &[SessionInfo],
    cast_receivers: &[CastReceiver],
) -> Vec<PanelTarget> {
    sessions
        .iter()
        .cloned()
        .map(|s| PanelTarget::Emby(Box::new(s)))
        .chain(cast_receivers.iter().cloned().map(PanelTarget::Cast))
        .collect()
}

impl super::App {
    /// Rebuilds `panel_targets` from the current `sessions`/`cast_receivers`
    /// snapshots, called independently by each channel's completion handler
    /// (`SessionEvent::Loaded`, `CastEvent::DiscoveryCompleted`) so Emby rows
    /// render as soon as they arrive without waiting on the concurrent cast
    /// browse (8.1). Preserves the panel cursor's selection by identity
    /// across the rebuild when possible, falling back to a clamp.
    pub(super) fn rebuild_panel_targets(&mut self) {
        let selected_key = self
            .panel_targets
            .get(self.sessions_cursor)
            .map(panel_target_key);
        self.panel_targets = build_panel_targets(&self.sessions, &self.cast_receivers);
        self.sessions_cursor = selected_key
            .and_then(|key| {
                self.panel_targets
                    .iter()
                    .position(|target| panel_target_key(target) == key)
            })
            .unwrap_or_else(|| {
                self.sessions_cursor
                    .min(self.panel_targets.len().saturating_sub(1))
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            device_name: format!("device-{id}"),
            client: "Emby".to_string(),
            user_name: "user".to_string(),
            host: "host".to_string(),
            supported_commands: Vec::new(),
            now_playing: None,
            now_playing_item_id: None,
            position_s: 0,
            runtime_s: 0,
            position_ticks: 0,
            runtime_ticks: 0,
            is_paused: false,
            volume: 100,
            sub_index: -1,
            audio_index: 0,
            muted: false,
            media_info: Default::default(),
        }
    }

    fn receiver(id: &str) -> CastReceiver {
        CastReceiver {
            id: id.to_string(),
            friendly_name: format!("receiver-{id}"),
            host: "192.168.0.5".to_string(),
            port: 8009,
        }
    }

    #[test]
    fn panel_content_is_produced_from_the_session_list_before_the_browse_result_arrives() {
        // No cast browse result yet (empty cast_receivers): the merged list
        // is exactly the Emby sessions already loaded (8.1).
        let targets = build_panel_targets(&[session("s1"), session("s2")], &[]);
        assert_eq!(targets.len(), 2);
        assert!(matches!(targets[0], PanelTarget::Emby(ref s) if s.id == "s1"));
        assert!(matches!(targets[1], PanelTarget::Emby(ref s) if s.id == "s2"));
    }

    #[test]
    fn a_device_on_both_channels_is_two_distinct_targets() {
        let targets = build_panel_targets(&[session("shared")], &[receiver("shared")]);
        assert_eq!(targets.len(), 2);
        assert!(matches!(targets[0], PanelTarget::Emby(ref s) if s.id == "shared"));
        assert!(matches!(targets[1], PanelTarget::Cast(ref r) if r.id == "shared"));
    }

    #[test]
    fn rebuild_preserves_the_selected_cast_target_across_an_emby_reload() {
        let mut app = crate::app::tests::make_app_stub();
        app.cast_receivers = vec![receiver("r1")];
        app.sessions = vec![session("s1")];
        app.rebuild_panel_targets();
        app.sessions_cursor = 1; // the cast row
        assert!(matches!(app.panel_targets[1], PanelTarget::Cast(_)));

        // Emby session list reloads with a different session; the cast row
        // stays selected instead of the cursor drifting onto whatever now
        // sits at index 1.
        app.sessions = vec![session("s1"), session("s2")];
        app.rebuild_panel_targets();

        assert!(matches!(
            app.panel_targets[app.sessions_cursor],
            PanelTarget::Cast(ref r) if r.id == "r1"
        ));
    }
}
