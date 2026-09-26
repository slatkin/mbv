use super::{
    CTRL_CAP_ABS_BOOK_PROGRESS, CTRL_CAP_ABS_BOOK_QUEUE, CTRL_CAP_ABS_PROGRESS, CTRL_CAP_ABS_QUEUE,
    CTRL_CAP_AUDIO_ONLY, CTRL_CAP_CONTROL_AUTH, CTRL_CAP_LIFECYCLE_SHUTDOWN,
    CTRL_CAP_OWNER_QUEUE_LOAD, CTRL_CAP_QUEUE_STATE, CTRL_CAP_START_INDEX, CTRL_CAP_STATUS_ONLY,
    CTRL_PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CtrlHello {
    pub protocol_version: u32,
    pub app_version: String,
    pub capabilities: Vec<String>,
    /// Control credential used only when the peer advertises `control-auth`.
    #[serde(default)]
    pub control_token: Option<String>,
}

impl CtrlHello {
    #[must_use]
    pub fn current() -> Self {
        Self {
            protocol_version: CTRL_PROTOCOL_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec![
                CTRL_CAP_QUEUE_STATE.to_string(),
                CTRL_CAP_START_INDEX.to_string(),
                CTRL_CAP_STATUS_ONLY.to_string(),
                CTRL_CAP_LIFECYCLE_SHUTDOWN.to_string(),
                super::CTRL_CAP_UNIFIED_QUEUE.to_string(),
                CTRL_CAP_CONTROL_AUTH.to_string(),
                CTRL_CAP_ABS_QUEUE.to_string(),
                CTRL_CAP_ABS_PROGRESS.to_string(),
                CTRL_CAP_ABS_BOOK_QUEUE.to_string(),
                CTRL_CAP_ABS_BOOK_PROGRESS.to_string(),
                CTRL_CAP_OWNER_QUEUE_LOAD.to_string(),
            ],
            control_token: None,
        }
    }

    #[must_use]
    pub fn current_control_client(control_token: String) -> Self {
        let mut hello = Self::current();
        hello.control_token = Some(control_token);
        hello
    }

    pub fn validate_peer(&self) -> Result<(), String> {
        self.compatibility()?;
        self.validate_required_capabilities()
    }

    pub fn compatibility(&self) -> Result<CtrlCompatibility, String> {
        CtrlCompatibility::for_peer(self.protocol_version)
    }

    fn validate_required_capabilities(&self) -> Result<(), String> {
        for required in [
            CTRL_CAP_QUEUE_STATE,
            CTRL_CAP_START_INDEX,
            CTRL_CAP_STATUS_ONLY,
        ] {
            if !self.capabilities.iter().any(|cap| cap == required) {
                return Err(format!(
                    "peer missing daemon protocol capability: {required}"
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn supports_lifecycle_shutdown(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_LIFECYCLE_SHUTDOWN)
    }

    #[must_use]
    pub fn supports_audio_only(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_AUDIO_ONLY)
    }

    #[must_use]
    pub fn supports_control_auth(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_CONTROL_AUTH)
    }

    #[must_use]
    pub fn supports_abs_queue(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_QUEUE)
    }

    #[must_use]
    pub fn supports_abs_progress(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_PROGRESS)
    }

    #[must_use]
    pub fn supports_abs_book_queue(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_BOOK_QUEUE)
    }

    #[must_use]
    pub fn supports_abs_book_progress(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_ABS_BOOK_PROGRESS)
    }

    #[must_use]
    pub fn supports_owner_queue_load(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap == CTRL_CAP_OWNER_QUEUE_LOAD)
    }

    pub fn validate_control_credential(&self, expected: &str) -> Result<(), String> {
        let Some(presented) = self.control_token.as_deref() else {
            return Err("invalid Control credential".to_string());
        };
        if presented.len() == expected.len()
            && presented
                .as_bytes()
                .iter()
                .zip(expected.as_bytes())
                .fold(0u8, |difference, (&presented, &expected)| {
                    difference | (presented ^ expected)
                })
                == 0
        {
            Ok(())
        } else {
            Err("invalid Control credential".to_string())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "queue/progress/book_queue/book_progress are four independently negotiated peer capabilities, not one state (design analysis, issue #804)"
)]
pub struct CtrlAudiobookshelfCapabilities {
    pub queue: bool,
    pub progress: bool,
    pub book_queue: bool,
    pub book_progress: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each supports_* bit is an independently negotiated optional protocol feature; no honest enum or bundle exists across queue/lifecycle/audio/auth/owner-load (design analysis, issue #804)"
)]
pub struct CtrlCompatibility {
    pub peer_protocol_version: u32,
    pub client_protocol_version: u32,
    pub supports_queue_append: bool,
    pub supports_lifecycle_shutdown: bool,
    pub supports_audio_only: bool,
    pub supports_control_auth: bool,
    pub audiobookshelf: CtrlAudiobookshelfCapabilities,
    pub supports_owner_queue_load: bool,
}

impl CtrlCompatibility {
    pub fn for_peer(peer_protocol_version: u32) -> Result<Self, String> {
        match peer_protocol_version {
            CTRL_PROTOCOL_VERSION => Ok(Self {
                peer_protocol_version,
                client_protocol_version: CTRL_PROTOCOL_VERSION,
                supports_queue_append: true,
                supports_lifecycle_shutdown: false,
                supports_audio_only: false,
                supports_control_auth: true,
                audiobookshelf: CtrlAudiobookshelfCapabilities {
                    queue: true,
                    progress: true,
                    book_queue: true,
                    book_progress: true,
                },
                supports_owner_queue_load: false,
            }),
            _ => Err(format!(
                "incompatible daemon protocol version: peer={peer_protocol_version} local={CTRL_PROTOCOL_VERSION}"
            )),
        }
    }

    /// # Panics
    ///
    /// Panics if `for_peer` rejects `CTRL_PROTOCOL_VERSION`, i.e. if the
    /// "local ctrl protocol version is compatible" invariant is violated.
    /// `for_peer` accepts exactly that version, so the compatible local
    /// protocol version always resolves.
    #[must_use]
    pub fn current() -> Self {
        Self::for_peer(CTRL_PROTOCOL_VERSION).expect("local ctrl protocol version is compatible")
    }
}
