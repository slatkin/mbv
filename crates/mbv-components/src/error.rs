use std::{error::Error, fmt};

#[derive(Debug)]
pub struct ComponentsError(ComponentsErrorKind);

#[derive(Debug)]
enum ComponentsErrorKind {
    Search(String),
}

impl ComponentsError {
    pub fn search(message: impl Into<String>) -> Self {
        Self(ComponentsErrorKind::Search(message.into()))
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        "components.search"
    }
}

impl fmt::Display for ComponentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            ComponentsErrorKind::Search(message) => f.write_str(message),
        }
    }
}

impl Error for ComponentsError {}
