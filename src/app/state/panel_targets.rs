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
pub(in crate::app) enum PanelTarget {
    Emby(Box<SessionInfo>),
    Cast(CastReceiver),
}

/// Stable identity for one F3 target, qualified by its control channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SessionTargetKey {
    Emby(String),
    Cast(String),
}

impl PanelTarget {
    pub(in crate::app) fn key(&self) -> SessionTargetKey {
        match self {
            Self::Emby(session) => SessionTargetKey::Emby(session.id.clone()),
            Self::Cast(receiver) => SessionTargetKey::Cast(receiver.id.clone()),
        }
    }
}

/// Resolve an activation against the latest shell-owned target snapshot.
pub(in crate::app) fn resolve_session_target(
    targets: &[PanelTarget],
    key: &SessionTargetKey,
) -> Option<PanelTarget> {
    targets.iter().find(|target| target.key() == *key).cloned()
}

/// Concatenates Emby sessions and discovered cast receivers into one list,
/// Emby first: no dedup, no ordering decision beyond "which channel arrived
/// first" (8.2). Pure and side-effect free so it is testable without a
/// running panel or a network call (8.1).
pub(in crate::app) fn build_panel_targets(
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

impl crate::app::App {
    /// Rebuilds `panel_targets` from the current `sessions`/`cast_receivers`
    /// snapshots, called independently by each channel's completion handler
    /// (`SessionEvent::Loaded`, `CastEvent::DiscoveryCompleted`) so Emby rows
    /// render as soon as they arrive without waiting on the concurrent cast
    /// browse (8.1). Preserves the panel cursor's selection by identity
    /// across the rebuild when possible, falling back to a clamp.
    pub(in crate::app) fn rebuild_panel_targets(&mut self) {
        // SessionsComponent owns the cursor. Preserve only its selected row
        // identity in the runtime snapshot; the component clamps its cursor
        // when the replacement list arrives.
        self.panel_targets = build_panel_targets(&self.sessions, &self.cast_receivers);
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
            playable_media_types: Vec::new(),
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
    fn session_activation_missing_key_resolves_no_target() {
        let targets = build_panel_targets(&[session("current")], &[]);
        assert!(resolve_session_target(&targets, &SessionTargetKey::Emby("gone".into())).is_none());
    }

    #[test]
    fn session_activation_reordered_snapshot_keeps_target_identity() {
        let targets = build_panel_targets(&[session("other"), session("selected")], &[]);
        let selected =
            resolve_session_target(&targets, &SessionTargetKey::Emby("selected".into())).unwrap();
        assert!(matches!(selected, PanelTarget::Emby(ref session) if session.id == "selected"));
    }

    #[test]
    fn session_activation_equal_channel_ids_resolve_independently() {
        let targets = build_panel_targets(&[session("shared")], &[receiver("shared")]);
        let emby = resolve_session_target(&targets, &SessionTargetKey::Emby("shared".into()));
        let cast = resolve_session_target(&targets, &SessionTargetKey::Cast("shared".into()));
        assert!(matches!(emby, Some(PanelTarget::Emby(session)) if session.id == "shared"));
        assert!(matches!(cast, Some(PanelTarget::Cast(receiver)) if receiver.id == "shared"));
    }
}
