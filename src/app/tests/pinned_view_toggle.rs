//! App-dispatch contracts for the pinned view toggle (change
//! `pinned-view-toggle`, design D2/D3).
//!
//! The live `pinwin::Panel` cannot be built hermetically, so these drive the
//! post-apply dispatch step directly: `apply_pinned_view` owns the width-first
//! view assignment and the rejection flash, and `pinned_view_toggle` only adds
//! the panel apply in front of it.

use super::*;
use crate::pin::PinnedWidth;

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
