use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::application::PollStrategy;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::library_panel::{LibraryContentOwner, LibraryPanel};
use crate::app::components::msg::TvHit;
use crate::app::components::tv_content::TvContent;
use crate::app::components::tv_tree_target::TvTreeTarget;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::make_movie_app;
use crate::app::shell::{fold_keyboard_messages, fold_mouse_messages};
use crate::app::state::types::events::NavigateLanding;
use crate::app::state::types::playback::{DestinationLatestSnapshot, DestinationLatestSource};
use crate::app::tests::install_test_emby;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::{LibEvent, PanelFocus, PanelMode, TabSelection};
use mbv_core::mock_http::MockHttp;
use mbv_core::playback_queue::QueueItem;
use std::time::{Duration, Instant};

fn tv_harness() -> TickHarness {
    let mut app = make_movie_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::Both;
    app.terminal_width = 160;
    app.terminal_height = 50;
    app.libs[0].library.collection_type = "tvshows".into();
    for (index, item) in app.libs[0].nav_stack[0].items.iter_mut().enumerate() {
        item.item_type = "Series".into();
        item.id = format!("series-{index}");
        item.overview.clear();
    }
    let mut season = crate::app::tests::make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = crate::app::tests::make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    app.series_detail_cache.insert(
        "series-0".into(),
        crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: [("season-1".into(), vec![episode])].into_iter().collect(),
        },
    );
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_tv_content();
    harness.model_mut().sync_active_destination();
    harness
}

/// The one TV owner (task 8.4, design D2): registered inside the mounted
/// `LibraryPanel` under `LibraryKey::Service(TvShows)` at every breakpoint.
fn tv(harness: &TickHarness) -> &TvContent {
    harness.model().test_tv_owner()
}

/// The mounted `LibraryPanel` hosting the TV owner.
fn panel(harness: &TickHarness) -> &LibraryPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("LibraryPanel")
}

fn flat_episode_harness(mode: mbv_core::config::TvContentMode) -> TickHarness {
    let mut harness = tv_harness();
    let mut episode = crate::app::tests::make_item("Latest Episode", "Episode");
    episode.id = "latest-episode".into();
    // Keep the fixture on the direct single-item play path; the series
    // continuation policy is unrelated to flat-mode activation.
    episode.series_id.clear();
    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.items = vec![episode];
    level.item_types = Some("Episode".into());
    level.tv_content_mode = Some(mode.clone());
    level.loading = false;
    harness.model_mut().app.libs[0].library_total = Some(301);
    harness.model_mut().app.libs[0].tv_content_mode = Some(mode);
    harness.model_mut().sync_mounted_surfaces();
    harness
}

fn draw(harness: &mut TickHarness) {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    // One throwaway draw publishes `root_frame` (the shell's startup draw);
    // the sync then mounts/activates the panel, and the recorded draw paints
    // it — the steady state the deleted component's tests saw after its
    // second `view`.
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
}

fn step_without_sync(harness: &mut TickHarness) -> Vec<Msg> {
    let pre_fold_focus = harness.model().application.focus().cloned();
    let raw_messages = harness
        .model_mut()
        .application
        .tick(PollStrategy::Once(std::time::Duration::from_millis(500)))
        .expect("tick injected event");
    let folded = fold_mouse_messages(raw_messages);
    let router = harness.model_mut().router_outcome(&folded);
    fold_keyboard_messages(folded, pre_fold_focus.as_ref(), &router)
}

/// One tick whose shell requests are handled like the run loop's.
fn step_and_drain(harness: &mut TickHarness) {
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
}

/// Opens Inline Search, types a query, and fires its debounce with a clock
/// tick past the deadline (no wall-clock waiting).
fn search_series(harness: &mut TickHarness, query: &str) {
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(harness);
    for ch in query.chars() {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Char(ch),
            modifiers: KeyModifiers::NONE,
        }));
        step_and_drain(harness);
    }
    harness
        .model_mut()
        .tick_inline_search_clock(Instant::now() + Duration::from_millis(301));
}

/// Enter on a Series search result navigates the library list to the
/// series' natural place and opens its workspace (Wide) / the Library Hero
/// overlay (Narrow) -- the ordinary browser Enter flow, launched from
/// search (inline-library-search spec, "Enter on a Series result").
mod landing;
mod latest;
mod launch;
mod owner;
mod tree;
