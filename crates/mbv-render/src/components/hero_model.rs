//! Transitional re-export (task 1.7): the Series image-type chain and the
//! shared hero meta-row builder now live in `mbv_ui_model::library_hero` so
//! both Clients read them from the shared leaf crate. Task 1.8 deletes this
//! module and moves its importers to `mbv_ui_model::library_hero`.

#[doc(inline)]
pub use mbv_ui_model::library_hero::{SERIES_LANDSCAPE_IMAGE_TYPES, emby_hero_meta_rows_plain};
