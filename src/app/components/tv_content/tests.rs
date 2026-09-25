use super::*;
use crate::app::render::{LibraryListRenderCtx, TvWideRenderCtx};
use crate::app::tests::make_item;
use mbv_core::api::EmbyItem;

mod episode_rows_tests;
mod tree_panel_tests;
mod tree_projection_tests;
mod workspace_tests;

fn tv_tree_context(
    items: Vec<EmbyItem>,
    selected_id: Option<&str>,
    detail: Option<crate::app::SeriesDetail>,
    show_letter_pills: bool,
) -> TvWideRenderCtx {
    let selected = selected_id.and_then(|id| items.iter().find(|item| item.id == id).cloned());
    TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(items, 0),
        selected,
        detail,
        0,
        None,
        show_letter_pills,
    )
}

fn tv_show(name: &str, id: &str) -> EmbyItem {
    let mut item = make_item(name, "Series");
    item.id = id.into();
    item
}
