## Context

See proposal.md (Why). Current state:

- `render_card` (`src/app/render/components/card.rs`) resolves `active_source.or_else(selected_source)`: with nothing playing it falls through to the queue cursor and fetches that item's image, so the card shows a stale "now playing" image while idle.
- `compose_base_frame` (`src/app/shell_draw.rs`) derives `show_controls` from local-active OR session-connected OR cast-attached, and always reserves `PLAYER_BOX_HEIGHT` rows; in queue-only mode the in-queue playback panel paints seekbar/title/controls (or the idle feed title) even with nothing playing.
- `render_main` (`src/app/shell_draw.rs`) stacks card then in-queue panel then queue list; `queue_panel_geometry` (`src/app/render/arrangements/queue.rs`) already accepts `(card_height, narrow_player_height)` inputs, so `(0, 0)` moves the list to the top with no new geometry.
- The hide signal already exists: `effective_playback_state().active` (`src/app/playback_target.rs`) folds local + remote-session + cast into one flag, with paused counting as active in all three transports. `show_controls` in `compose_base_frame` does not use it today (it uses connection presence, not now-playing presence).
- Because `show_controls` and the new gate read different things, they can disagree: a connected-but-idle Emby session, or a cast attachment with an empty `dispatched`, has `show_controls == true` and `active == false`. The idle panel is not blank there — `chrome_player.rs:46` paints the `→` track and the title row still carries the cast/remote now-playing title.
- `layout.playback.idle_feed_link_area` is written only inside `render_player_panel` and read by nothing; skipping the panel leaves a write-only stale field, which is harmless. What is not harmless: `idle_feed_command_for_key` (`src/app/action.rs:105`) opens the feed link on `!player_active && !has_connected_session && link_available` with no render condition, so `o` keeps working in queue-only idle after the title it describes has been hidden.
- `card_reserved_rect` (`src/app/render/components/card.rs`) reserves `max_h - 2` rows (≈22 at terminal height > 30, ≈10 at ≤ 30) while `last_card_height == 0` and a fetch is in flight, and `last_card_height` afterwards — so once any image has rendered this session, the idle-to-active transition reserves the *previous* geometry, not a full-height blank box.

## Goals / Non-Goals

**Goals:**

- Idle queue-only (effective mode QueueOnly, covers both the `x` toggle at 80+ columns and narrow-terminal mini view) shows only the queue list, with no card or panel rows above it.
- Paused keeps image and controls; only fully idle collapses.
- The `v` selection survives idle invisibly and applies on next playback.
- A remote session or cast that is connected but not playing keeps the current panel behaviour, unchanged by this change.

**Non-Goals:**

- No change to the two-panel (`Both`) layout: idle card, controls, and idle feed title stay as today.
- No change to queue rows, projection, cursor, or scope behavior (see `queue-canonical-list`).
- No wide-vs-narrow idle variant: both card and panel go to zero, so no replacement sizing is needed.
- No change to the `o` idle-feed key binding. It still opens the current feed link while queue-only idle, with no visible title to justify it. See decisions; this is a product call to confirm, not an oversight.
- No change to the right-column `player_area` reservation in `chrome_geometry` — it is already empty in queue-only (`right_visible == false`) and belongs to the `Both` layout.

## Decisions

- Gate on global `effective_playback_state().active == false` (not displayed-scope state): the card follows what is making sound, and paused stays visible. Matches the user's chosen rule A and the existing card behavior of following global active.
- Collapse at the single call site in `render_main`, not inside `render_card`. `render_card` has exactly one production caller (`src/app/shell_draw.rs`, inside the `is_queue_only` branch), so a mode check inside it is unreachable for every path but that one — and it is untestable there: the card tests call `render_card` directly on a stub whose `terminal_width` is 80 (`src/app/render/tests.rs`), so `effective_panel_mode()` returns the stored `panel_mode` and never the narrow mini-view value. Skipping the call at the caller also keeps `card.rs` (746 lines against the 800 cap, tests in-file) from growing and leaves its idle-selection behaviour intact for the `Both` layout.
- When idle in queue-only, skip both paints and pass `QueuePanelInputs { card_height: 0, narrow_player_height: 0 }`. Zero geometry alone is not enough: `render_player_panel` sizes its content from the `player_h` constant (4), not from the area it is given, so leaving the calls in place with zero-height areas would still paint four panel rows over the top of the queue list. `queue_panel_geometry` itself needs no change.
- Leave the existing `last_card_height/width` memory untouched on collapse. It is not per-playback state: it already supplies the reservation for every card frame, so clearing it would make the first playing frame reserve the full-height blank box instead of the previous geometry, and reusing it buys nothing beyond not writing a line.
- Suppress idle card fetches (main + 3-ahead/1-behind prefetch) when the card will not render: honest UI — no network for an invisible image. The cost is a taller reservation on the first playing frame after a cold start (≈22 rows above 30 terminal rows, ≈10 at or below), shrinking once the image lands. Accepted; see the alternative in the next section, which this trade-off is the point of.
- Keep the connected-but-not-playing case rendering the panel as today, and gate only on `active`. The disagreement with `show_controls` is deliberate: with a remote session or cast attached the panel title can still carry a real now-playing name, so the panel is information rather than decoration, while the card has nothing to illustrate. `show_controls` is not changed by this work.
- Gate the idle feed key with the same signal as the paint. `o` currently opens the feed link from `idle_feed_command_for_key` (`src/app/action.rs`) on "idle and a link exists", independent of whether the title is drawn; after this change it would keep firing for text that is no longer displayed. The RouterSnapshot already carries what it needs (panel mode / a render-effective idle flag), so this is one condition, not new plumbing.
- Accept losing the idle feed title in queue-only: it lives in the hidden panel's title row there, while `Both` keeps it. Recorded as a spec delta on `idle-feed-rotation`, not a separate display surface.
- Record the collapse in `docs/architecture/interactive-surface-ledger.md` (the Queue row at line 105 and the Root playback-chrome row at 104 both describe who paints the queue-only slot) and amend the **Idle feed** definition in `CONTEXT.md`, which currently states flatly that the feed is displayed in the playback panel when idle.

Alternatives considered:

- Displayed-scope gating (hide while browsing the non-playing scope even when the other scope plays): rejected, harder to explain and changes playing-remote-while-browsing-local behavior the card has today.
- Keeping the controls while hiding only the image: rejected by the user; with nothing playing the seekbar/buttons operate on nothing and `Enter` on a row is the play action.
- Keeping an empty visualizer box when `v` is selected: rejected; it preserves the exact confusing reservation this change removes.
- Keeping the cursor-item fetch while idle and suppressing only the neighbour prefetch: the alternative that removes the cold-start row jump entirely, at the price of one network request per cursor move with nothing to show for it. Rejected for the same reason as the ticket loss — honest UI — and so the change does not need a `system-audio-visualizer` requirement about fetch timing. If the jump reads badly in practice, this is the lever to pull.

## Risks / Trade-offs

- [Risk] `queue-only-playback` scenario "Playback panel visible when idle", the `system-audio-visualizer` "No playback can supply samples → rectangle remains" scenario, and `panel-mode`'s library-only mention of the card all collide with the collapse → Mitigation: delta specs rewrite the first two; the third is a library-only assertion and is unaffected, which the implementation should confirm rather than assume by reading the archived text.
- [Risk] Card tests that call `render_card` directly (`stopped_playback_uses_visible_queue_selection`, `prefetch_centers_on_selected_source_when_stopped`, `stopped_local_selection_with_empty_remote`, `completed_no_art_uses_card_placeholder`, Audiobookshelf-cover-on-stop, images-off and selected-visualizer geometry) cannot express queue-only idle at the stub's 80-column width → Mitigation: none of them need updating. They exercise the `Both` path, which this change leaves alone; the new behaviour is proven at the `render_main` call site instead.
- [Risk] Overlap with in-progress `unify-queue-playback-authority` (playback state shape) and `add-queue-drag-reorder` (queue geometry/projection) → Mitigation: rebase onto their latest before implementing; conflict surface is small (state reader + geometry inputs).
- [Risk] First playing frame after a cold start reserves ~22 rows (≈10 on terminals ≤ 30 rows) of blank card, then shrinks when the image lands — fetch suppression removed the warm image → Mitigation: accepted trade-off, same reservation that already exists for any pending fetch. Revisit via the prefetch-only alternative if it reads badly.
- [Non-risk, verify only] `v` pressed while idle running `App::toggle_visualizer` → `sync_visualizer` and starting system-audio capture behind a box that no longer exists → it cannot: `visualizer_should_run` (`src/app/visualizer.rs`) already requires local `active`, so idle never starts the worker and `render_interval` never drops to its 16 ms visualizer cadence. No code change; this change just needs a test that keeps the claim honest.
- [Risk] The `o` feed-link key keeps working in queue-only idle after the title it acts on is hidden — a key with no visible referent → Mitigation: gate `idle_feed_command_for_key` on the same signal; flagged here because the alternative (leave it working, undocumented) is defensible and is a product call.

## Migration Plan

No data migration. Visual change only in queue-only idle. Rollback is reverting the change; specs restore automatically on archive.
