/// Provider-neutral presentation data for one F3 target row.
#[derive(Clone, Debug)]
pub enum SessionTargetRow {
    Emby {
        id: String,
        /// The Emby device identity, which the owner's Emby Session output
        /// names (emby-session-owner-output design D12).
        device_id: String,
        device_name: String,
        client: String,
        user_name: String,
        host: String,
        now_playing: Option<String>,
        is_paused: bool,
        position_s: i64,
        runtime_s: i64,
        /// Whether the session advertises a direct Player-owner endpoint other
        /// than the local Owner (design D6): the shell projects
        /// `session_direct_endpoint` per row, and `false` covers this
        /// device's own row (`DaemonEndpoint::Local`). A Cast row never has
        /// one, so the variant carries no field.
        has_direct_endpoint: bool,
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

/// Which queue-transfer actions the selected F3 target offers (design D6):
/// the Interactive Component resolves the gates from its shell-owned
/// projection, and the Sessions painter lists the offered keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionTransferOffers {
    pub hand_off: bool,
    pub take: bool,
}

/// The shell-owned transfer gating inputs (design D6): whether the home of
/// the Client is the local Owner, and whether the local Owner plays. The
/// component resolves them against its own selection into
/// `SessionTransferOffers`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionTransferGates {
    pub home_is_local_daemon: bool,
    pub local_owner_plays: bool,
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
