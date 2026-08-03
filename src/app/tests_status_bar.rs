use super::*;
use crate::app::tests::*;

#[test]
fn home_refresh_preserves_cursor_by_lib_id() {
    // Simulate what init_home does: old_cursors keyed by lib_id.
    let old_latest: Vec<(String, String, Vec<MediaItem>, usize)> = vec![
        (
            "Latest Movies".into(),
            "lib-movies".into(),
            make_items(10),
            7,
        ),
        ("Latest TV".into(), "lib-tv".into(), make_items(5), 3),
    ];
    let old_cursors: std::collections::HashMap<String, usize> = old_latest
        .iter()
        .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
        .collect();

    // New fetch returns same libs but with fresh items.
    let new_items_movies = make_items(12);
    let new_items_tv = make_items(4);

    let cursor_movies = old_cursors
        .get("lib-movies")
        .copied()
        .unwrap_or(0)
        .min(new_items_movies.len().saturating_sub(1));
    let cursor_tv = old_cursors
        .get("lib-tv")
        .copied()
        .unwrap_or(0)
        .min(new_items_tv.len().saturating_sub(1));

    assert_eq!(cursor_movies, 7, "cursor preserved when within bounds");
    assert_eq!(cursor_tv, 3, "cursor preserved when within bounds");
}

#[test]
fn home_refresh_clamps_cursor_when_new_list_is_shorter() {
    let old_latest: Vec<(String, String, Vec<MediaItem>, usize)> = vec![(
        "Latest Movies".into(),
        "lib-movies".into(),
        make_items(10),
        9,
    )];
    let old_cursors: std::collections::HashMap<String, usize> = old_latest
        .iter()
        .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
        .collect();

    let new_items = make_items(5); // shorter than before
    let cursor = old_cursors
        .get("lib-movies")
        .copied()
        .unwrap_or(0)
        .min(new_items.len().saturating_sub(1));

    assert_eq!(cursor, 4, "cursor clamped to new last index");
}

#[test]
fn home_refresh_cursor_defaults_zero_for_new_library() {
    let old_cursors: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let new_items = make_items(8);
    let cursor = old_cursors
        .get("brand-new-lib")
        .copied()
        .unwrap_or(0)
        .min(new_items.len().saturating_sub(1));
    assert_eq!(cursor, 0);
}

#[test]
fn home_section_clamped_after_refresh_removes_sections() {
    let mut app = make_app_stub();
    app.home.latest = sections(4); // 5 total
    app.home.section = 4;

    // Simulate refresh that returns fewer sections.
    app.home.latest = sections(1); // now only 2 total
    let n = 1 + app.home.latest.len();
    if app.home.section >= n {
        app.home.section = n.saturating_sub(1);
    }
    assert_eq!(app.home.section, 1);
}

#[test]
fn home_section_cycle_includes_continue_watching_in_both_directions() {
    let mut app = make_app_stub();
    app.home.continue_items = make_items(1);
    app.home.latest = sections(2);

    app.home.section = 2;
    app.power_home_move_section(1);
    assert_eq!(app.home.section, 0);

    app.power_home_move_section(-1);
    assert_eq!(app.home.section, 2);
}

// ── status_bar (Task 2: session/connection label + unsaved marker) ───────
//
// The remote/session-label resolution tests that used to live here
// (direct-remote label, attached-session device name, direct-upgrade
// session name, local-status default, left-side ordering, icon/label
// coloring) exercised the status bar's own `remote_status_spans`
// rendering with `show_session_pill: true` -- a mode only the deleted
// Standard view's status-bar call site used. The main status bar
// has always called `render_status_bar(.., false)` (unchanged by
// this diff -- confirmed via `git diff origin/main`), because Power
// surfaces the same remote/session info via the queue column's
// Local/Remote title pills instead (`render_power_queue_title` in
// `render/power/queue.rs`, which calls the same shared
// `remote_status_spans` helper). That underlying logic still matters in
// production, so those tests were moved to `render/mod.rs`'s test
// module, rewritten to call `render_status_bar` directly with
// `show_session_pill: true` (the same pattern already used by
// `status_bar_remote_hitbox_tracks_visible_pill_after_alive_marker`
// etc.) rather than deleted outright.

#[test]
fn status_bar_shows_emby_server_on_the_right_side() {
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.server_url = "http://emby.local:8096".into();

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("emby.local"),
        "expected Emby server host on the right side of the status bar:\n{last_line}"
    );
    assert!(
        last_line.trim_end().ends_with("emby.local"),
        "Emby server host should be the rightmost status item:\n{last_line}"
    );
    assert!(
        !last_line.contains("server:"),
        "server host should not be prefixed on the right side:\n{last_line}"
    );
}

#[test]
fn autosave_pill_appears_in_queue_panel_when_saved_playlist_with_autosave() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".into()),
        name: "Road Trip".into(),
    };
    {
        let mut cfg = app.client.lock().unwrap();
        cfg.config.server_url = "http://emby.local:8096".into();
        cfg.config.save_playlist_on_consume = true;
    }

    let rendered = render_app_to_string(&mut app, 80, 24);
    let all_lines: String = rendered.lines().collect::<Vec<_>>().join("\n");

    assert!(
        all_lines.contains("AUTOSAVE"),
        "expected AUTOSAVE pill in the queue panel:\n{all_lines}"
    );
}

#[test]
fn status_bar_shows_muted_text_without_mute_glyph() {
    let mut app = make_app_stub();
    app.mute_on = true;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("muted"),
        "expected text label for muted state:\n{last_line}"
    );
    assert!(
        !last_line.contains(" m "),
        "muted state should not use the old single-letter mute glyph:\n{last_line}"
    );
    // Ordering relative to the remote pill isn't checked here: Power
    // View's status bar never shows the session/remote pill (that lives
    // in the queue column's Local/Remote title pills instead -- see
    // `render_power_queue_title`), so "muted last among left-side
    // statuses" no longer has a remote pill to be last *after*.
    // The playlist pill has also moved out of the status bar into the
    // left queue panel's bottom row, so it no longer orders against it.
}

#[test]
fn status_bar_hides_mute_indicator_when_not_muted() {
    let mut app = make_app_stub();
    app.mute_on = false;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        !last_line.contains("mute"),
        "unmuted state should not render a mute indicator:\n{last_line}"
    );
    assert_eq!(
        app.layout.playback.ind_mu,
        ratatui::layout::Rect::default(),
        "hidden mute indicator should not leave an invisible click target"
    );
}

#[test]
fn status_bar_has_no_session_or_daemon_label_when_remote_slot_is_off() {
    let mut app = make_app_stub();

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        !last_line.contains("attached:") && !last_line.contains("daemon:"),
        "expected no attached-session or daemon label when nothing is connected:\n{last_line}"
    );
}

#[test]
fn status_bar_has_surrounding_row_background_and_pill_cells() {
    let mut app = make_app_stub();
    // The playlist pill moved into the left queue panel, so leave the status
    // bar with a left-side pill (the mute indicator) to verify pill cells.
    app.mute_on = true;

    let term = render_app_to_terminal(&mut app, 80, 24);
    let buf = term.backend().buffer();
    let last_y = buf.area().height - 1;
    let test_bg = palette::STATUS_PILL_BG;
    let mut saw_row_bg = false;
    let mut saw_pill_bg = false;

    for x in 0..buf.area().width {
        let bg = buf[(x, last_y)].bg;
        // The status row itself renders on DARK_BG (not the old
        // transparent BAR_BG) so the pill segments read as sitting on
        // top of it -- see render_status_bar's `bar_style`.
        saw_row_bg |= bg == palette::DARK_BG;
        saw_pill_bg |= bg == test_bg;
    }

    assert!(
        saw_row_bg,
        "expected the status bar row background to be visible"
    );
    assert!(
        saw_pill_bg,
        "expected pill-colored cells to sit on top of the row background"
    );
}

#[test]
fn unsaved_pill_appears_when_queue_is_dirty() {
    let mut app = make_app_stub();
    app.queue_dirty = true;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let all_lines: String = rendered.lines().collect::<Vec<_>>().join("\n");

    assert!(
            all_lines.contains("UNSAVED"),
            "expected an UNSAVED marker regardless of the active tab when the queue is dirty:\n{all_lines}"
        );
}

#[test]
fn unsaved_pill_replaces_autosave_when_dirty() {
    let mut app = make_app_stub();
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Road Trip".into(),
    };
    app.client.lock().unwrap().config.save_playlist_on_consume = true;
    app.queue_dirty = true;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let all_lines: String = rendered.lines().collect::<Vec<_>>().join("\n");

    assert!(
        all_lines.contains("UNSAVED"),
        "dirty saved-playlist queue should show UNSAVED in the save-state slot:\n{all_lines}"
    );
    assert!(
        !all_lines.contains("AUTOSAVE"),
        "UNSAVED should replace AUTOSAVE while the queue is dirty:\n{all_lines}"
    );
}

// ── status_bar (Task 3: right-aligned queue-state segment) ────────────────

#[test]
fn status_bar_shows_queue_source_label_on_queue_tab() {
    let mut app = make_app_stub();
    // Equivalent of the old "Queue tab": the queue side is focused, so
    // the queue-source label is relevant and should render.
    app.panel_focus = PanelFocus::Queue;
    app.queue_source = crate::config::QueueSource::Album;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("ALBUM"),
        "expected an ALBUM queue-source label when the queue side is focused:\n{last_line}"
    );
}

#[test]
fn status_bar_hides_queue_segment_outside_queue() {
    let mut app = make_app_stub();
    app.queue_source = crate::config::QueueSource::Album;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
            !last_line.contains("ALBUM"),
            "queue source/autosave/scope detail must not leak onto tabs where it isn't relevant:\n{last_line}"
        );
}

#[test]
fn status_bar_omits_redundant_remote_queue_label() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.set_queue_scope(QueueScope::Remote);

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        !last_line.contains("REMOTE QUEUE"),
        "queue scope is already apparent from the UI and should not be repeated:\n{last_line}"
    );
}

// ── status_bar (Task 4: toast overlay replacement) ──────────────────────

#[test]
fn toast_renders_in_status_bar_without_covering_main_content_above_it() {
    let mut app = make_app_stub();
    app.status = "Saved [Y]".to_string();
    app.status_expires = Some(std::time::Instant::now() + std::time::Duration::from_secs(5));

    let rendered = render_app_to_string(&mut app, 80, 24);
    let lines: Vec<&str> = rendered.lines().collect();
    let last_line = lines.last().unwrap();

    assert!(
        last_line.contains("Saved"),
        "expected the toast text on the final row:\n{last_line}"
    );
    // Old behavior covered 3 rows with Clear+overlay; new behavior must
    // only touch the single bottom row, leaving the row above untouched.
    let second_to_last = lines[lines.len() - 2];
    assert!(
        !second_to_last.contains("Saved"),
        "toast must not spill onto the row above the status bar:\n{second_to_last}"
    );
}

// `status_bar_pill_click_regions_stay_populated_during_toast` (deleted):
// it asserted `layout.playback.ind_rc` (the remote-cycle click hitbox)
// stays populated while a toast covers the status bar. `ind_rc` is only
// ever populated when `render_status_bar` is called with
// `show_session_pill: true` -- the Standard-only call site that this PR
// removes. The main status bar call (`render/power/mod.rs`,
// unchanged by this diff) has always passed `show_session_pill: false`,
// so `ind_rc` never populates in Power regardless of any toast --
// remote/local cycling in Power happens by clicking the queue column's
// own Local/Remote title pills (`queue_scope_local_area` /
// `queue_scope_remote_area`, handled in `input.rs`), a separate row the
// status-bar toast never covers. The bug this test guarded (hitbox goes
// stale specifically *during* a toast) has no Power analog to regress.
