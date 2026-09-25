mod latest;
mod launch_reanchor;
mod mounted_browser;

use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::emby_library_content::EmbyLibraryContent as BrowserOwner;
use crate::app::components::inline_search::InlineSearchHost;
use crate::app::components::library_panel::{LibraryContentOwner, LibraryPanel};
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::make_movie_app;
use crate::app::LibEvent;

use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::tests::{install_test_emby, make_session};
use std::time::{Duration, Instant};

/// The migrated Movies/HomeVideos/Generic owner inside the mounted
/// `LibraryPanel` (task 6.1): the panel is the library area's one event
/// boundary, and the browse state is read through the owner the panel hosts
/// — never a destination component (`emby_browser_id` stays `None` for
/// these kinds).
fn browser_owner(harness: &TickHarness) -> &BrowserOwner {
    let (_, key, _) = harness
        .model()
        .active_emby_library_owner()
        .expect("the active library's owner has migrated");
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .owner(&key)
        .and_then(|owner| owner.as_any().downcast_ref::<BrowserOwner>())
        .expect("browser owner installed")
}

fn library_panel(harness: &TickHarness) -> &LibraryPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
}

fn dispatch_messages(harness: &mut TickHarness, messages: Vec<Msg>) {
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
}

fn selector_click_point(harness: &TickHarness, target: usize) -> Position {
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type");
    let (rect, _) = panel
        .test_selector_hits()
        .regions()
        .iter()
        .find(|(_, candidate)| *candidate == target)
        .expect("selector target was painted");
    Position::new(rect.x, rect.y)
}

fn click_selector(
    harness: &mut TickHarness,
    target: usize,
) -> crate::app::tests::tick_integration::harness::StepOutcome {
    let at = selector_click_point(harness, target);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: at.x,
        row: at.y,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step()
}

fn draw(harness: &mut TickHarness, width: u16, height: u16) -> Terminal<TestBackend> {
    harness.model_mut().app.terminal_width = width;
    harness.model_mut().app.terminal_height = height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    terminal
}

fn draw_mounted(harness: &mut TickHarness, width: u16, height: u16) {
    let _ = draw(harness, width, height);
    let _ = draw(harness, width, height);
}
