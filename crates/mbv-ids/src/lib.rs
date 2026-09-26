use std::fmt;

// Type-safe Emby identifiers. Bare strings for these can be silently crossed
// (e.g. passing an item id where a session id is expected); the newtypes make
// such crossings compile errors. Match `QueueSlotId`'s style: tuple struct with
// an explicit accessor, no `Deref<Target = str>` (that would let the newtypes
// compare with each other via `&str`, defeating the point). `Display` covers
// logging; serde derives keep the ctrl wire format unchanged (a newtype struct
// serializes as its inner value).

/// String-backed identifier newtype: `new`, `empty`/`clear` (reusing the
/// heap allocation), `as_str`, and `Display`.
macro_rules! string_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            /// Create an empty identifier, reusing the existing heap allocation when used
            /// via [`clear`](Self::clear) on an already-initialized value.
            pub fn empty() -> Self {
                Self(String::new())
            }

            /// Clear the identifier in place, reusing the internal buffer.
            pub fn clear(&mut self) {
                self.0.clear();
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

string_id! {
    /// Emby item identifier (movies, episodes, albums, tracks).
    ItemId
}
string_id! {
    /// A specific media source within an item.
    MediaSourceId
}
string_id! {
    /// The Emby playback session.
    EmbySessionId
}
