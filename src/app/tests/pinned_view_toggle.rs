//! App-dispatch contracts for the pinned view toggle (change
//! `pinned-view-toggle`, design D2/D3).
//!
//! The live `pinwin::Panel` cannot be built hermetically, so these drive the
//! post-apply dispatch step directly: `apply_pinned_view` owns the width-first
//! view assignment and the rejection flash, and `pinned_view_toggle` only adds
//! the panel apply in front of it.

use super::*;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::pin::PinnedWidth;
use mbv_components::QueuePlaybackPanel;
use mbv_ui_msg::ComponentId;

/// Design D2/D3: from the collapsed width, an accepted expand records the
/// expanded width and stores the library view. `terminal_width` is held at the
/// pre-toggle collapsed width, so a `set_panel_focus` call would take its
/// mini-view branch and leave `panel_focus` stale at `Queue`.
#[test]
fn pinned_expand_applies_the_library_view_from_the_stale_collapsed_width() {
    let mut app = make_app_stub();
    app.terminal_width = 40;
    app.pinned_width = PinnedWidth::Collapsed;
    app.panel_mode = PanelMode::QueueOnly;
    app.panel_focus = PanelFocus::Queue;
    app.mini_view_focus = PanelFocus::Queue;

    app.apply_pinned_view(PinnedWidth::Expanded, Ok(()));

    assert_eq!(app.pinned_width, PinnedWidth::Expanded);
    assert_eq!(app.panel_mode, PanelMode::LibraryOnly);
    assert_eq!(app.mini_view_focus, PanelFocus::Library);
    assert_eq!(
        app.panel_focus,
        PanelFocus::Library,
        "the stored panel focus must be written, not derived"
    );
}

/// Design D2/D3: from the expanded width, an accepted collapse records the
/// collapsed width and stores the queue view (mini view at the sub-80
/// collapsed width), mirroring the expand assertions.
#[test]
fn pinned_collapse_applies_the_queue_view_from_the_stale_expanded_width() {
    let mut app = make_app_stub();
    app.terminal_width = 120;
    app.pinned_width = PinnedWidth::Expanded;
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;

    app.apply_pinned_view(PinnedWidth::Collapsed, Ok(()));

    assert_eq!(app.pinned_width, PinnedWidth::Collapsed);
    assert_eq!(app.panel_mode, PanelMode::QueueOnly);
    assert_eq!(app.mini_view_focus, PanelFocus::Queue);
    assert_eq!(
        app.panel_focus,
        PanelFocus::Queue,
        "the stored panel focus must be written, not derived"
    );
}

/// Design D2: a rejected target width flashes the reason and changes neither
/// the width nor the displayed panel mode.
#[test]
fn rejected_pinned_expand_flashes_and_leaves_the_view_alone() {
    let mut app = make_app_stub();
    app.terminal_width = 40;
    app.pinned_width = PinnedWidth::Collapsed;
    app.panel_mode = PanelMode::QueueOnly;
    app.panel_focus = PanelFocus::Queue;
    app.mini_view_focus = PanelFocus::Queue;

    app.apply_pinned_view(
        PinnedWidth::Expanded,
        Err("the layout is invalid for this monitor".into()),
    );

    assert_eq!(app.pinned_width, PinnedWidth::Collapsed);
    assert_eq!(app.panel_mode, PanelMode::QueueOnly);
    assert_eq!(app.mini_view_focus, PanelFocus::Queue);
    assert_eq!(app.panel_focus, PanelFocus::Queue);
    assert_eq!(
        app.status, "the layout is invalid for this monitor",
        "the rejection reason is flashed"
    );
    assert_eq!(app.status_severity, ToastSeverity::Warning);
}

/// Design D4: `Ctrl+e`'s success mutation records only the width, so the
/// stored panel mode and its focus survive a toggle in both directions.
#[test]
fn recording_the_toggled_width_leaves_the_panel_mode_alone() {
    let mut app = make_app_stub();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;

    app.note_pinned_width_applied(PinnedWidth::Expanded);

    assert_eq!(app.pinned_width, PinnedWidth::Expanded);
    assert_eq!(app.panel_mode, PanelMode::LibraryOnly);
    assert_eq!(app.panel_focus, PanelFocus::Library);
    assert_eq!(app.mini_view_focus, PanelFocus::Library);

    app.note_pinned_width_applied(PinnedWidth::Collapsed);

    assert_eq!(app.pinned_width, PinnedWidth::Collapsed);
    assert_eq!(app.panel_mode, PanelMode::LibraryOnly);
    assert_eq!(app.panel_focus, PanelFocus::Library);
    assert_eq!(app.mini_view_focus, PanelFocus::Library);
}

/// Regression (2026-10-05 crash, pinned view toggle): the collapse race must
/// paint without indexing outside the buffer. The width tween resizes the pty
/// after the sync pass read the size, so the first frame after the snap runs
/// its sync pass at the stale expanded width — the queue playback panel
/// retains a wider transport band than the collapsed buffer holds — and then
/// draws into the snapped 40-column buffer. The band's gauge painter indexes
/// the buffer without clipping (`ratatui-widgets` gauge.rs `buf[(x, y)]`), so
/// the unclamped band panicked with `index outside of buffer`. The panel now
/// clamps the retained transport rect to the placement it paints, and this
/// drives that exact frame end to end: settle the expanded frame, flip the
/// view, sync at the stale width, draw into the snapped buffer.
#[test]
fn the_collapse_race_frame_paints_the_stale_wide_transport_within_the_snapped_buffer() {
    let mut app = crate::app::tests::render_fixtures::make_queue_app(3);
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.terminal_width = 120;
    app.terminal_height = 51;
    app.player.update_status(|status| {
        status.active = true;
        status.queue_len = 3;
        status.current_idx = 0;
        status.position_ticks = 45 * mbv_emby_model::TICKS_PER_SECOND;
        status.runtime_ticks = 90 * mbv_emby_model::TICKS_PER_SECOND;
    });
    let mut harness = TickHarness::new(app);
    let mut terminal = Terminal::new(TestBackend::new(120, 51)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();

    // The toggle dispatch: the view flips immediately, the width stays stale.
    let model = harness.model_mut();
    model.app.panel_mode = PanelMode::QueueOnly;
    model.app.mini_view_focus = PanelFocus::Queue;
    model.app.panel_focus = PanelFocus::Queue;
    model.app.focus_queue_initial_item();
    // The race frame: the sync pass still sees the stale expanded width, and
    // the draw's buffer already holds the snapped collapsed size.
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(40, 51)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();

    // The transport's hit geometry reflects what the race frame painted:
    // either clamped into the snapped buffer or zeroed when the stale band
    // no longer overlaps it — never the stale expanded width.
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::QueuePlaybackPanel)
        .and_then(|component| component.as_any().downcast_ref::<QueuePlaybackPanel>())
        .expect("queue playback panel mounted in a queue-visible layout");
    let (_, seekbar) = panel.transport_hits();
    assert!(
        seekbar.right() <= 40,
        "the seekbar must stay within the snapped buffer, got {seekbar:?}"
    );
}
