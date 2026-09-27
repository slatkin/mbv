use mbv_emby_model::EmbyItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbyFailureClass {
    AuthenticationRejected,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbyFailure {
    pub class: EmbyFailureClass,
    pub message: String,
}

impl EmbyFailure {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            class: EmbyFailureClass::Unavailable,
            message: message.into(),
        }
    }
}

impl From<String> for EmbyFailure {
    fn from(message: String) -> Self {
        Self::unavailable(message)
    }
}

impl std::fmt::Display for EmbyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Network results needed to populate Continue Watching and the Emby library
/// catalog. The UI applies this value after the bounded startup worker completes.
#[derive(Debug, Clone, Default)]
pub struct EmbyBootstrap {
    pub continue_items: Vec<EmbyItem>,
    pub views: Vec<EmbyItem>,
}
