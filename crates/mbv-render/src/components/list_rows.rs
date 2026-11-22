//! The `List` component (design.md "Component catalogue"): the browser-list
//! input the wide TV/Music render contexts embed. The canonical media-list
//! painters (`render/components/media_list/{row,wide}.rs`) own browser row
//! painting, and `render_right_scrollbar` (`widgets.rs`) is the shared
//! `Scrollbar`.
//!
//! The input itself is a shared presentation model: `LibraryListRenderCtx`
//! lives in `mbv-ui-model` and is re-exported here for the paths that read it
//! from this module.

pub use mbv_ui_model::list_ctx::LibraryListRenderCtx;
