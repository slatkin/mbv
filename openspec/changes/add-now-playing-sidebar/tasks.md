## 1. Sidebar geometry and strip reservation

- [ ] 1.1 Add `header_height` to `QueuePanelInputs` and offset `queue_panel_geometry` by it alongside
  `card_height`/`narrow_player_height`, so the queue panel and its 1-row gap sit below the header.
  Verify: unit test in `src/app/render/arrangements/queue.rs` proving the queue panel starts one row
  below the header and that the gap appears only when card/player/header occupy rows above.
- [ ] 1.2 Move the strip's reservation with its paint: `right_area` adds `PLAYER_BOX_HEIGHT` only when
  the queue column is hidden, and `player_area` is published only then.
  Verify: `src/app/render/tests_queue.rs` buffer proof that `both` reserves no strip rows (the library
  starts directly below the tab bar), `library-only` reserves exactly `PLAYER_BOX_HEIGHT`, and
  `queue-only` reserves none.
- [ ] 1.3 Keep the left column's short-terminal budget intact with the extra header row.
  Verify: `short_window_keeps_queue_in_left_column` and `short_queue_panel_drops_padding_before_rows`
  (`src/app/render/tests_queue.rs`) pass at 24 rows, with the queue title row and pill row threshold
  assertions re-derived rather than loosened.

## 2. Header row projection and paint

- [ ] 2.1 Derive `NowPlayingStatus { Playing, Paused, Idle }` once per frame next to
  `effective_playback_state()`. Verify: unit test covering `active && !paused` → `Playing`,
  `active && paused` → `Paused`, `!active` → `Idle` (including the unreachable `!active && paused`
  input mapping to `Idle`).
- [ ] 2.2 Extract one `playback_host_label()` from `queue_title_model` and have the queue title use
  it too, without the tracking suffix or the queue title's uppercasing.
  Verify: `src/app/render/queue_title_characterization_tests.rs` stays green unchanged (the queue
  title's rendered text does not move), and a new assertion shows the extracted label carries no
  ` · TRACKING` text for an attached session.
- [ ] 2.3 Paint the header row in `render_main`'s left-column path (status left, `on <host>` right, on
  `SURFACE_CHROME`, no new theme role). Verify: buffer tests in `src/app/render/tests_queue.rs` at 80
  columns and 100+ columns showing `PLAYING` / `PAUSED` / `IDLE` and the target text, and no header
  row in library-only.
- [ ] 2.4 Prove the header follows the playback target rather than the viewed queue scope.
  Verify: a queue-visible frame attached to a remote session with the Local scope selected paints the
  remote target in the header.

## 3. One painter for the playback panel

- [ ] 3.1 Publish the live panel rect into `layout.playback.player_area` from `render_main` (sidebar
  slot when the queue column is visible) and delete the base-frame `render_player_panel` calls in the
  queue-only branch; make `narrow_player` mean "this is the sidebar panel".
  Verify: `player_chrome_legacy_base_frame_publishes_geometry_but_paints_no_panel`
  (`src/app/render/tests.rs`) asserts the sidebar slot instead of the strip, and
  `wide_active_queue_starts_below_panel_rows` stays green.
- [ ] 3.2 Prove through the real draw pass that exactly one painter paints each layout.
  Verify: `Application::tick()` integration test in `src/app/tests_tick_integration*.rs` drawing
  `both`, `queue-only`, and `library-only` and asserting the panel content appears in the published
  rect only after the component view, with the base frame leaving those cells untouched.
- [ ] 3.3 Give the sidebar panel working pointer geometry.
  Verify: tick integration test clicking the sidebar panel's play/pause glyph and seekbar in `both`
  and in mini-view `queue-only` emits `TogglePlayPause` / `SeekTo`, and a click in the collapsed
  panel's rows emits nothing.

## 4. Idle collapse and the idle-feed gate

- [ ] 4.1 Delete the connected-idle exception: idle collapses the visual slot and the panel in every
  queue-visible layout, leaving the header as the only idle indicator.
  Verify: `connected_idle_queue_only_keeps_panel_but_collapses_card` becomes the inverse assertion,
  and `idle_queue_only_hides_card_and_panel_at_both_widths`,
  `idle_both_hides_card_and_reclaims_queue_rows`, and
  `idle_queue_only_reclaims_card_and_panel_rows_until_playback_starts` pass at narrow, 80, and 100+
  columns.
- [ ] 4.2 Re-point the idle-feed key gate at the panel's presence instead of the panel mode.
  Verify: `src/app/action_tests.rs` and the routing-matrix rows in
  `src/app/tests_routing_matrix_playback.rs` show the open-link key suppressed in idle `both` and
  `queue-only` and firing in idle `library-only`.
- [ ] 4.3 Keep paused-keeps-both and playback-start-restores-both at both breakpoints.
  Verify: `paused_queue_only_keeps_card_and_panel` plus a `both`-mode counterpart assert the visual
  slot, the panel, and the header in the same frame; a start-from-idle frame asserts the same.

## 5. Docs and spec sync

- [ ] 5.1 Update the surface ledger's Root playback row and Queue row, and add the sidebar and strip
  terms to `CONTEXT.md`. Verify: rows describe one painter per layout and name the new term; the
  ledger's mouse-geometry column no longer claims an empty rect in queue-only.
- [ ] 5.2 At archive, sync the deltas and confirm the retired capability's main spec is deleted.
  Verify: `openspec validate add-now-playing-sidebar --strict` clean, `openspec/specs/` has no
  `queue-only-playback` directory, and `panel-mode` / `idle-feed-rotation` carry the modified
  requirements.

## 6. Whole-change verification

- [ ] 6.1 Run the narrow/wide visual sweep across all four layouts (`both`, `queue-only`,
  `library-only`, narrow mini view) with playback idle, paused, and playing.
  Verify: exactly one playback panel is visible in every frame, the header reads the right status and
  target, and no queue row is painted over.
- [ ] 6.2 Run the gates. Verify: `cargo nextest run -p mbv` green, `cargo clippy --workspace
  --all-targets` clean, `ast-grep scan` clean, `cargo fmt --all -- --check` clean.
