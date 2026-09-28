use std::{error::Error, fmt, io};

#[derive(Debug)]
pub struct ConfigError {
    kind: ConfigErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

#[derive(Debug, Clone, Copy)]
enum ConfigErrorKind {
    Io,
    Parse,
    Lifecycle,
    Admin,
    Credentials,
    State,
    Save,
    Launch,
}

impl ConfigError {
    fn new(kind: ConfigErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    fn with_source<E>(kind: ConfigErrorKind, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind,
            message: source.to_string(),
            source: Some(Box::new(source)),
        }
    }

    pub(crate) fn io(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Io, message)
    }

    pub(crate) fn parse(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Parse, message)
    }

    pub(crate) fn lifecycle(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Lifecycle, message)
    }

    pub(crate) fn admin(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Admin, message)
    }

    pub(crate) fn credentials(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Credentials, message)
    }

    pub(crate) fn state(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::State, message)
    }

    pub(crate) fn save(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Save, message)
    }

    pub(crate) fn launch(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Launch, message)
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            ConfigErrorKind::Io => "config.io",
            ConfigErrorKind::Parse => "config.parse",
            ConfigErrorKind::Lifecycle => "config.lifecycle",
            ConfigErrorKind::Admin => "config.admin",
            ConfigErrorKind::Credentials => "config.credentials",
            ConfigErrorKind::State => "config.state",
            ConfigErrorKind::Save => "config.save",
            ConfigErrorKind::Launch => "config.launch",
        }
    }

    #[must_use]
    pub fn is_io(&self) -> bool {
        matches!(self.kind, ConfigErrorKind::Io)
    }

    #[must_use]
    pub fn is_parse(&self) -> bool {
        matches!(self.kind, ConfigErrorKind::Parse)
    }

    #[must_use]
    pub fn is_lifecycle(&self) -> bool {
        matches!(self.kind, ConfigErrorKind::Lifecycle)
    }

    #[must_use]
    pub fn is_admin(&self) -> bool {
        matches!(self.kind, ConfigErrorKind::Admin)
    }

    #[must_use]
    pub fn is_credentials(&self) -> bool {
        matches!(self.kind, ConfigErrorKind::Credentials)
    }

    #[must_use]
    pub fn is_state(&self) -> bool {
        matches!(self.kind, ConfigErrorKind::State)
    }

    #[must_use]
    pub fn is_save(&self) -> bool {
        matches!(self.kind, ConfigErrorKind::Save)
    }

    #[must_use]
    pub fn is_launch(&self) -> bool {
        matches!(self.kind, ConfigErrorKind::Launch)
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

impl From<io::Error> for ConfigError {
    fn from(source: io::Error) -> Self {
        Self::with_source(ConfigErrorKind::Io, source)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(source: toml::de::Error) -> Self {
        Self::with_source(ConfigErrorKind::Parse, source)
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(source: toml::ser::Error) -> Self {
        Self::with_source(ConfigErrorKind::Save, source)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(source: serde_json::Error) -> Self {
        Self::with_source(ConfigErrorKind::State, source)
    }
}

impl From<mbv_keybinds::KeybindsError> for ConfigError {
    fn from(source: mbv_keybinds::KeybindsError) -> Self {
        Self::with_source(ConfigErrorKind::Parse, source)
    }
}
