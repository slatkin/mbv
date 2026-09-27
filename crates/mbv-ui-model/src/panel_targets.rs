/// Provider-neutral presentation data for one F3 target row.
#[derive(Clone, Debug)]
pub enum SessionTargetRow {
    Emby {
        id: String,
        device_name: String,
        client: String,
        user_name: String,
        host: String,
        now_playing: Option<String>,
        is_paused: bool,
        position_s: i64,
        runtime_s: i64,
    },
    Cast {
        id: String,
        friendly_name: String,
        host: String,
        port: u16,
    },
}
/// Stable identity for one F3 target, qualified by its control channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionTargetKey {
    Emby(String),
    Cast(String),
}

impl SessionTargetRow {
    #[must_use]
    pub fn key(&self) -> SessionTargetKey {
        match self {
            Self::Emby { id, .. } => SessionTargetKey::Emby(id.clone()),
            Self::Cast { id, .. } => SessionTargetKey::Cast(id.clone()),
        }
    }
}
