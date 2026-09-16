//! Task 5.2: one real `Application::tick()` through the shell sync pass
//! proves both playback panels receive the typed now-playing title parts
//! from the one shared transport projection — a two-part item (audio track
//! with an artist context) and a single-part item (a movie) at the same
//! playhead, in the queue-column panel and in the right-column strip.

use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

use crate::app::components::{ComponentId, LibraryPlaybackPanel, QueuePlaybackPanel};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{App, PanelFocus, PanelMode};
use mbv_core::playback_queue::{PlaybackTitlePart, PlaybackTitlePartRole, PlaybackTitleParts};

/// An unbound chord: no policy arm claims it and the focused component
/// ignores it, so the tick is real while mutating nothing.
fn inert_key() -> Event<crate::app::components::UserEvent> {
    Event::Keyboard(KeyEvent {
        code: Key::Function(24),
        modifiers: KeyModifiers::NONE,
    })
}

/// An active, locally-owned playhead over a two-item queue: slot 0 is an
/// audio track with an artist (two parts), slot 1 a movie (one part).
fn title_parts_app(active_idx: usize) -> App {
    let mut app = make_app_stub();
    app.panel_mode = PanelMode::Both;
    app.panel_focus = PanelFocus::Queue;
    let mut track = make_item("Track Two", "Audio");
    track.id = "track-two".into();
    track.media_type = "Audio".into();
    track.artist = "Artist B".into();
    let mut movie = make_item("Movie One", "Movie");
    movie.id = "movie-one".into();
    app.player_tab.set_items(vec![track, movie], 0);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = active_idx;
        status.queue_len = 2;
    }
    app
}

fn expected_parts(title: &str, context: Option<&str>) -> PlaybackTitleParts {
    PlaybackTitleParts {
        title: PlaybackTitlePart {
            role: PlaybackTitlePartRole::Title,
            text: title.into(),
        },
        context: context.map(|text| PlaybackTitlePart {
            role: PlaybackTitlePartRole::Context,
            text: text.into(),
        }),
    }
}

fn step_tick(harness: &mut TickHarness) {
    harness.inject(inert_key());
    harness.step();
}

fn queue_panel(harness: &TickHarness) -> &QueuePlaybackPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::QueuePlaybackPanel)
        .and_then(|component| component.as_any().downcast_ref::<QueuePlaybackPanel>())
        .expect("Queue playback panel mounted in a queue-visible layout")
}

fn strip(harness: &TickHarness) -> &LibraryPlaybackPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::LibraryPlaybackPanel)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPlaybackPanel>())
        .expect("the strip mounts when the queue column is hidden")
}

#[test]
fn tick_projects_title_parts_to_both_playback_panels_from_the_one_projection() {
    let mut harness = TickHarness::new(title_parts_app(0));

    // Queue-column panel (queue visible): the two-part slot projects both
    // parts, the single-part slot one.
    step_tick(&mut harness);
    assert_eq!(
        queue_panel(&harness).transport_title_parts_for_test().as_ref(),
        Some(&expected_parts("Track Two", Some("Artist B"))),
        "the two-part slot projects a title part and a context part"
    );

    harness.model_mut().app.player.status.lock().unwrap().current_idx = 1;
    step_tick(&mut harness);
    assert_eq!(
        queue_panel(&harness).transport_title_parts_for_test().as_ref(),
        Some(&expected_parts("Movie One", None)),
        "the single-part slot projects the title part alone"
    );

    // Right-column strip (queue column hidden): the same playheads project
    // through the same shared projection into the other panel.
    harness.model_mut().app.panel_mode = PanelMode::LibraryOnly;
    harness.model_mut().app.panel_focus = PanelFocus::Library;
    step_tick(&mut harness);
    assert_eq!(
        strip(&harness).title_parts_for_test().as_ref(),
        Some(&expected_parts("Movie One", None)),
        "the strip receives the single-part projection"
    );

    harness.model_mut().app.player.status.lock().unwrap().current_idx = 0;
    step_tick(&mut harness);
    assert_eq!(
        strip(&harness).title_parts_for_test().as_ref(),
        Some(&expected_parts("Track Two", Some("Artist B"))),
        "the strip receives the two-part projection"
    );
}
