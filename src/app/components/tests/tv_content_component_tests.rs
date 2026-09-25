//! TV embedded-owner tests (tasks 8.1–8.4). These exercise `TvContent`
//! directly for its content projection, local key interpretation and typed
//! message translation; pointer resolution and painting are exercised through
//! the mounted `LibraryPanel` that hosts the owner (task 8.4 deleted the
//! mounted component, its `ComponentId` and its hit stores).

use crate::app::components::inline_search::SearchPool;
use crate::app::components::library_panel::{LibraryContentOwner, LibraryKey, LibraryPanel};
use crate::app::components::media_list::MediaSemanticState;
use crate::app::components::msg::{Msg, ShellRequest, TerminalObserverEvent, TvHit};
use crate::app::components::tv_content::TvContent;
use crate::app::components::tv_tree_target::TvTreeTarget;
use crate::app::components::LibraryKind;
use crate::app::render::{LibraryListRenderCtx, TvWideRenderCtx};
use crate::app::tests::make_item;
use mbv_core::config::{
    EmbyLetterBucket, EmbySelectorKey, LibraryItemIdentity, SelectorIdentity, ServiceKind,
    TvContentMode,
};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute};

/// The TV owner's `LibraryKey` under the mounted panel: one `Service` key for
/// a `tvshows` library (task 8.4, design D2).
fn tv_key() -> LibraryKey {
    LibraryKey::Service {
        service: ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: LibraryKind::TvShows,
    }
}

fn panel_with(owner: TvContent, focused: bool) -> LibraryPanel {
    let mut panel = LibraryPanel::new();
    panel.insert_owner(tv_key(), Box::new(owner));
    panel.set_active(Some(tv_key()));
    Component::attr(&mut panel, Attribute::Focus, AttrValue::Flag(focused));
    panel
}

fn paint(panel: &mut LibraryPanel, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| Component::view(panel, frame, Rect::new(0, 0, width, height)))
        .unwrap();
    terminal
}

fn tv(panel: &LibraryPanel) -> &TvContent {
    panel
        .owner(&tv_key())
        .and_then(|owner| owner.as_any().downcast_ref::<TvContent>())
        .expect("TV owner installed")
}

fn tv_mut(panel: &mut LibraryPanel) -> &mut TvContent {
    panel
        .owner_mut(&tv_key())
        .and_then(|owner| owner.as_any_mut().downcast_mut::<TvContent>())
        .expect("TV owner installed")
}

fn key(code: Key) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }
}

fn down(owner: &mut TvContent, code: Key) -> Option<Msg> {
    owner.on_key(&key(code))
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event<crate::app::components::UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

mod content_projection;
mod episode_keyboard;
mod launch_state;
mod pointer_interaction;
mod search;
mod tree_interaction;
