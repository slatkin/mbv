//! The visual slot's right-click and Hide entry, end to end through the
//! real tick (`split-tui-gui-clients` row 2.4). It sits in its own module
//! so the parent file stays within its line ceiling, and reuses the
//! parent's fixtures.

use super::*;

/// End to end: the queue playback panel resolves a right-click on the visual
/// slot, the shell opens the slot's chrome menu from the click, and the Hide
/// entry hides the slot. The slot rect comes from the panel's own projected
/// hit geometry, not a recomputed expectation.
#[test]
fn tick_visual_slot_right_click_opens_the_menu_and_hide_hides_the_slot() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = crate::app::tests::render_fixtures::make_movie_app();
    app.seed_active_tab_for_test(TabSelection::Home);
    // The two-panel layout needs a non-empty queue (hide-empty-queue-column D1).
    app.seed_local_queue_for_test(vec![make_item("Queue Item", "Movie")], 0);
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    let slot = visual_slot_area(&harness);
    assert!(
        slot.width > 0,
        "the visual slot must be projected: {slot:?}"
    );

    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: slot.x,
        row: slot.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    apply_outcome(&mut harness, outcome);
    harness.model_mut().sync_mounted_surfaces();

    let menu_id = ComponentId::Overlay(OverlayId::ContextMenu);
    assert!(
        harness.model().application.mounted(&menu_id),
        "a right-click on the visual slot opens its chrome menu"
    );

    press_key(&mut harness, tuirealm::event::Key::Down);
    press_key(&mut harness, tuirealm::event::Key::Enter);

    assert!(
        harness
            .model()
            .app
            .presentation_facts_for_test()
            .visual_slot_hidden,
        "the Hide entry hides the visual slot"
    );
}

/// The queue playback panel's projected visual slot rect.
fn visual_slot_area(harness: &TickHarness) -> Rect {
    harness
        .model()
        .application
        .get_component(&ComponentId::QueuePlaybackPanel)
        .expect("queue playback panel mounted")
        .as_any()
        .downcast_ref::<mbv_components::QueuePlaybackPanel>()
        .expect("queue playback panel type")
        .visual_slot_area_for_test()
}

/// Inject one key, step, and feed the produced messages to the shell.
fn press_key(harness: &mut TickHarness, code: tuirealm::event::Key) {
    harness.inject(Event::Keyboard(tuirealm::event::KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    apply_outcome(harness, outcome);
}
