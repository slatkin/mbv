use super::*;
use crate::app::render::{LibraryListRenderCtx, TvWideRenderCtx};
use crate::app::tests::make_item;
use mbv_core::api::EmbyItem;

mod episode_rows_tests;
mod flat_latest_activation_tests;
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

fn tv_episode(name: &str, id: &str) -> EmbyItem {
    let mut item = make_item(name, "Episode");
    item.id = id.into();
    item
}

#[test]
fn narrow_keyboard_moves_and_refreshes_through_shell_intents() {
    let mut owner = TvContent::new();
    owner.set_content(tv_tree_context(
        vec![tv_show("First", "first"), tv_show("Second", "second")],
        Some("first"),
        None,
        false,
    ));
    owner.set_is_wide(false);
    owner.pane = Pane::Episodes;
    let key = |code| KeyEvent {
        code,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    };

    assert!(matches!(
        owner.handle_key(&key(Key::Down)),
        Some(Msg::Shell(request)) if matches!(request.as_ref(), ShellRequest::EmbyLibraryCursorIndex { index: 1 })
    ));
    assert!(matches!(
        owner.handle_key(&key(Key::Char('r'))),
        Some(Msg::Shell(request)) if matches!(request.as_ref(), ShellRequest::EmbyLibraryRefresh)
    ));
}
