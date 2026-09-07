## Why

In the mini view showing the queue (full-width queue view via `x`, including narrow-terminal mini view), an idle player with nothing playing still reserves the top of the left column for the queue card (now-playing image, placeholder, or empty visualizer box) plus the in-queue playback panel (seekbar, title, controls). The card falls back to the cursor-selected item, so it shows a stale image that reads as "now playing" when nothing is. Those rows belong to the queue list.

## What Changes

- In queue-only effective mode (`effective_panel_mode() == QueueOnly`) when global playback is inactive (`effective_playback_state().active == false`):
  - The queue card (artwork, placeholder, loading reservation, empty visualizer box) is not rendered and reserves zero rows. No idle card image fetch or neighbour prefetch is issued.
  - The in-queue playback panel (seekbar, title row, controls row, idle feed title) is not rendered and reserves zero rows.
  - The queue list takes the freed rows; its existing single separator row is unchanged.
- Paused playback still counts as active: image and controls stay while paused.
- A remote session or cast that is *connected but not playing* keeps today's panel (its title can still carry a real now-playing name); only the card collapses there. See design.
- The `v` artwork/visualizer selection persists while idle but takes effect on the next playback; pressing `v` while idle does not create a box and does not start system-audio capture.
- The idle feed's `o` open-link command stops firing in queue-only idle, where its title is no longer displayed. `Both` is unchanged.
- The normal two-panel (`Both`) layout and library-only layout are unchanged by this change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `queue-only-playback`: the in-queue playback panel and queue visual slot are no longer rendered when playback is idle in queue-only; the queue list takes those rows.
- `system-audio-visualizer`: the "empty visualizer rectangle remains with no playback" rule no longer applies in queue-only; the card collapses to zero rows instead. Card-display scenarios gain the "card is rendered" precondition the collapse makes necessary.
- `idle-feed-rotation`: the idle feed title is no longer visible in queue-only idle (its playback-panel host row is gone there) and its open-link command does not fire there; it still shows in the two-panel layout as today.

## Impact

- Render path: `src/app/shell_draw.rs` (`render_main`: skip the `render_card` call and the in-queue `render_player_panel` calls, pass zero geometry inputs — the collapse lives at the call site, not inside `card.rs`), `src/app/visualizer.rs` (`visualizer_should_run` gains the queue-only-idle condition), `src/app/action.rs` + `src/app/key_policy.rs` (`idle_feed_command_for_key` gate and its `RouterSnapshot` field).
- Docs: `docs/architecture/interactive-surface-ledger.md` (Queue row 105, Root playback-chrome row 104) and the **Idle feed** term in `CONTEXT.md`.
- Specs: delta edits to `queue-only-playback`, `system-audio-visualizer`, `idle-feed-rotation` as above.
- Tests: new `render_main`-level queue-only idle proofs at narrow and wide, a paused-keeps-visuals proof, and a visualizer-capture-stays-down test. The existing `card.rs` unit tests call `render_card` directly at the stub's 80-column width and keep their `Both`-mode expectations unchanged.
- Related in-progress changes overlapping this area: `unify-queue-playback-authority`, `add-queue-drag-reorder` (possible merge conflicts in queue geometry/projection).
