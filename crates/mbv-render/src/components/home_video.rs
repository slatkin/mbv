//! Feed/home-video group browsing is painted by the embedded
//! `EmbyLibraryContent` owner; the shell owns loading and projects
//! `FeedHomeVideoState` into its context.

/// Transitional re-export (task 1.7): the Emby release-date formatter now
/// lives in `mbv_ui_model::ui_util` (with the hero producers that use it).
/// Task 1.8 deletes this re-export and moves its importers across.
#[doc(inline)]
pub use mbv_ui_model::ui_util::format_release_date;
