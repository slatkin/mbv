#![allow(dead_code, unused_imports)]

use super::super::*;
use crate::app::components::{BrowserComponent, Msg};
use crate::app::render::make_movie_app;
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::types_browse::BrowseResting;
use crate::app::{
    App, BrowseLevel, ContextAction, FeedHomeVideoGroup, FeedHomeVideoState, LibraryTab,
    PanelFocus, PanelMode, TabSelection,
};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

/// Drive one key into the mounted `BrowserComponent` and return its `Msg`
/// (test helper for the Model-boundary regression above).
pub(super) fn drive_browser_key(
    model: &mut Model,
    id: &ComponentId,
    key: Key,
    modifiers: KeyModifiers,
) -> Option<Msg> {
    model
        .application
        .get_component_mut(id)
        .expect("browser mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: key,
            modifiers,
        }))
}

/// Paint the App base frame and then the mounted Emby browser into a
/// `TestBackend` of the given size — the same two-step the live shell's
/// draw closure performs, so the App layout and the component's own
/// painted `LayoutMain` agree on the column stride.
pub(super) fn render_browser_model(model: &mut Model, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        model.app.compose_base_frame(f, None);
        model.render_emby_browser_component(f);
    })
    .unwrap();
}

/// A generic (non-Movies) Emby library with `n` flat Movie items: below
/// the 82-column breakpoint it never takes the wide-Movies hero-on-left
/// rail, so whatever column count the painted pane derives is the plain
/// flat-list stride for both the App and the mounted browser.
pub(super) fn browser_app_with_flat_movies(n: usize) -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Films", "CollectionFolder");
    library.id = "lib-films".into();
    library.is_folder = true;
    library.collection_type = "generic".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-films".into(),
            title: "Films".into(),
            items: make_items(n),
            total_count: n,
            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    app
}

pub(super) fn browser_component_painted_rows(model: &Model, id: &ComponentId) -> Vec<Vec<usize>> {
    model
        .application
        .get_component(id)
        .unwrap()
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .unwrap()
        .test_layout()
        .left_item_rows
        .clone()
}

pub(super) fn browser_component_cursor(model: &Model, id: &ComponentId) -> usize {
    model
        .application
        .get_component(id)
        .unwrap()
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .unwrap()
        .cursor()
}
