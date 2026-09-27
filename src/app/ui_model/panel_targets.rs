use mbv_cast::discovery::CastReceiver;
use mbv_emby::SessionInfo;

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
