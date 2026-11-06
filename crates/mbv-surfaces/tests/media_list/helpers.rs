//! Shared builders for the canonical media-list owner's public-API tests
//! (moved with `MediaList` from `mbv-components`, extract-client-core 4.3).

use mbv_surfaces::media_list::MediaList;
use mbv_ui_model::media_list::{MediaKind, MediaListRow, MediaSemanticState};

pub fn item(target: &str) -> MediaListRow<String> {
    MediaListRow::Item {
        target: target.into(),
        primary: target.into(),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

pub fn u8_item(target: u8) -> MediaListRow<u8> {
    MediaListRow::Item {
        target,
        primary: target.to_string(),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

pub fn list() -> MediaList<u8> {
    let mut list = MediaList::new();
    list.set_content((1..=8).map(u8_item).collect());
    list
}

/// A target whose clones the test counts: the old wheel path cloned every
/// selectable target through `row_flow()` (#926), so a large list floods the
/// counter while a length-only scroll stays constant.
#[derive(Debug, PartialEq, Eq)]
pub struct Counted(u32);

thread_local! {
    static TARGET_CLONES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl Clone for Counted {
    fn clone(&self) -> Self {
        TARGET_CLONES.with(|count| count.set(count.get() + 1));
        Self(self.0)
    }
}

pub fn counted_row(target: u32) -> MediaListRow<Counted> {
    MediaListRow::Item {
        target: Counted(target),
        primary: String::new(),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

/// Read the clone counter the `Counted` targets bump.
pub fn target_clones() -> u64 {
    TARGET_CLONES.with(std::cell::Cell::get)
}

/// Reset the clone counter before a counted test body runs.
pub fn reset_target_clones() {
    TARGET_CLONES.with(|count| count.set(0));
}
