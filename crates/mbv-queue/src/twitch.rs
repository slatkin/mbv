//! Twitch queue item: the URL snapshot the prompt submits, its derived
//! title, and the host check that decides whether a URL is a Twitch URL.
//! A Twitch item keeps no playback state and plays only through mpv.

/// A queued Twitch URL: the URL as given plus a title derived from it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct TwitchItem {
    pub url: String,
    pub title: String,
}

/// Why `TwitchItem::from_url` refused a URL: the scheme is not `http`/`https`,
/// or the host is not `twitch.tv` or a subdomain of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwitchUrlError {
    reason: String,
}

impl TwitchUrlError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for TwitchUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::error::Error for TwitchUrlError {}

impl TwitchItem {
    /// Parses a pasted URL into a Twitch item. The scheme must be `http` or
    /// `https` and the host must be `twitch.tv` or a subdomain of it; the
    /// path is never examined. The item keeps the trimmed URL as given and
    /// derives its title from the path (the host when the path is empty).
    pub fn from_url(input: &str) -> Result<Self, TwitchUrlError> {
        let trimmed = input.trim();
        let rest = trimmed
            .strip_prefix("https://")
            .or_else(|| trimmed.strip_prefix("http://"))
            .ok_or_else(|| {
                TwitchUrlError::new("not a Twitch URL: the scheme must be http or https")
            })?;
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let authority = rest[..authority_end].to_ascii_lowercase();
        let host = authority.split(':').next().unwrap_or_default();
        if host != "twitch.tv" && !host.ends_with(".twitch.tv") {
            return Err(TwitchUrlError::new(format!(
                "not a Twitch URL: {host} is not a twitch.tv host"
            )));
        }

        let path = rest[authority_end..]
            .split_once(['?', '#'])
            .map_or(&rest[authority_end..], |(path, _)| path);
        let title = path.trim_start_matches('/');
        let title = if title.is_empty() {
            host.to_string()
        } else {
            title.to_string()
        };
        Ok(Self {
            url: trimmed.to_string(),
            title,
        })
    }
}
