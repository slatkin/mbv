//! Shared fixtures for the `media_list` test modules.

use super::{MediaKind, MediaListRow, MediaSemanticState};

pub(super) fn lifecycle_item(target: &str) -> MediaListRow<String> {
    MediaListRow::Item {
        target: target.into(),
        primary: target.into(),
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}
