use std::{error::Error, fmt};

#[derive(Debug)]
pub struct UiModelError {
    kind: UiModelErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

#[derive(Debug, Clone, Copy)]
enum UiModelErrorKind {
    Operation,
}

impl UiModelError {
    pub fn operation(message: impl Into<String>) -> Self {
        Self {
            kind: UiModelErrorKind::Operation,
            message: message.into(),
            source: None,
        }
    }

    pub fn operation_context<E>(context: &str, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind: UiModelErrorKind::Operation,
            message: format!("{context}: {source}"),
            source: Some(Box::new(source)),
        }
    }

    /// Wraps a domain error keeping its `Display` text as the message so the
    /// UI-visible string is unchanged while the cause stays inspectable.
    pub fn operation_source<E>(source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind: UiModelErrorKind::Operation,
            message: source.to_string(),
            source: Some(Box::new(source)),
        }
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            UiModelErrorKind::Operation => "ui-model.operation",
        }
    }
}

impl fmt::Display for UiModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for UiModelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
