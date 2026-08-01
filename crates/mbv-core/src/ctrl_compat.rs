#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CtrlHello {
    pub protocol_version: u32,
    pub app_version: String,
    pub capabilities: Vec<String>,
    pub auth_token: Option<String>,
}

impl CtrlHello {
    pub fn current() -> Self {
        Self {
            protocol_version: CTRL_PROTOCOL_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec![
                CTRL_CAP_QUEUE_STATE.to_string(),
                CTRL_CAP_START_INDEX.to_string(),
                CTRL_CAP_STATUS_ONLY.to_string(),
            ],
            auth_token: None,
        }
    }

    pub fn current_client(auth_token: String) -> Self {
        let mut hello = Self::current();
        hello.auth_token = Some(auth_token);
        hello
    }

    pub fn compatible_client(auth_token: String, compatibility: CtrlCompatibility) -> Self {
        let mut hello = Self::current_client(auth_token);
        hello.protocol_version = compatibility.client_protocol_version;
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CtrlCompatibility {
    pub peer_protocol_version: u32,
    pub client_protocol_version: u32,
    pub supports_queue_append: bool,
}

impl CtrlCompatibility {
    pub fn for_peer(peer_protocol_version: u32) -> Result<Self, String> {
        match peer_protocol_version {
            7 => Ok(Self {
                peer_protocol_version: 7,
                client_protocol_version: 7,
                supports_queue_append: true,
            }),
            8 => Ok(Self {
                peer_protocol_version: 8,
                client_protocol_version: 8,
                supports_queue_append: true,
            }),
            _ => Err(format!(
                "incompatible daemon protocol version: peer={peer_protocol_version} local={CTRL_PROTOCOL_VERSION}"
            )),
        }
    }

    pub fn current() -> Self {
        Self::for_peer(CTRL_PROTOCOL_VERSION).expect("local ctrl protocol version is compatible")
    }
}
