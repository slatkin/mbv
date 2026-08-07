use std::fmt;

// Type-safe Emby identifiers. Bare strings for these can be silently crossed
// (e.g. passing an item id where a session id is expected); the newtypes make
// such crossings compile errors. Match `QueueSlotId`'s style: tuple struct with
// an explicit accessor, no `Deref<Target = str>` (that would let the newtypes
// compare with each other via `&str`, defeating the point). `Display` covers
// logging; serde derives keep the ctrl wire format unchanged (a newtype struct
// serializes as its inner value).

/// Emby item identifier (movies, episodes, albums, tracks).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ItemId(String);

impl ItemId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// A specific media source within an item.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MediaSourceId(String);

impl MediaSourceId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MediaSourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// The Emby playback session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EmbySessionId(String);

impl EmbySessionId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EmbySessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
