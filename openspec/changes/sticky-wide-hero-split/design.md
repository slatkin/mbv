## Context

See proposal.md for motivation. Two drag-resize paths exist with opposite durability:

- Queue sidebar: `QueueRequest::ResizeColumnLive` (memory only) + `ResizeColumnEnd` (memory + `save_prefs()` to `prefs.json`), loaded in `construct.rs` with legacy-key fallback, clamped at paint via `normalize_queue_column_width` (`src/app/queue_column_width.rs`, `shell_queue.rs:292-300`, `input.rs:106-127`).
- Wide hero split: `ShellRequest::ResizeListPaneLive` only; `DragEnd` is an explicit no-op (`panel.rs:split_gesture_msg`); startup hard-codes `None` (`construct.rs:162`); library refresh clears to `None` (`library_load_actions.rs:89`). The clamp already stores-raw/normalizes-on-read (`list_pane_width.rs`), which is exactly the shape persistence needs.

`save_prefs` rewrites the whole `prefs.json` object each call (preserving `home_section`), so adding one key needs no migration — a missing key means "no override," identical to today's `None`.

## Goals / Non-Goals

**Goals:**

- Mirror the queue live-then-persist gesture split for the wide hero drag: zero filesystem writes mid-drag, one write on release.
- Store the raw override and keep clamping at paint time, so narrow terminals and narrower wide surfaces degrade without load-time fixups.
- Make refresh a full reset (memory + disk) so refresh-then-restart cannot resurrect a cleared width.

**Non-Goals:**

- No keyboard control of the split; no per-surface widths (one shared width, as today); no new clamp logic; no config.toml involvement (prefs.json, like the queue width).

## Decisions

### D1: Add `ResizeListPaneEnd(u16)` alongside the existing `Live` variant

The boundary component already recognizes `DragEnd` via the shared `MouseGestureState`; today `split_gesture_msg` maps it to `None`. Emit `End` carrying the final resolved width only when the gesture changed it (same guard as the queue boundary: press-without-motion emits nothing, so no write). Alternative (persist on every `Live`) rejected: filesystem write per motion event, which D3 of the queue-column design already ruled out.

### D2: Persist as nullable `list_pane_width` in prefs.json

`save_prefs` gains `"list_pane_width": self.list_pane_width` (serde_json `Option` serializes as number-or-null); load maps a missing/null/non-numeric value to `None`. No legacy fallback needed (no prior key existed). Alternative (sentinel `0`) rejected: `None` already means "default ratio" throughout the paint path, so null round-trips with no translation.

### D3: Refresh writes the clear through

`refresh_current_view`'s library arm sets `None` (existing) and calls `save_prefs()` (new). Without the write-through, disk would disagree with memory after every refresh. The queue-focus arm is untouched. Alternative (leave disk stale until next drag-end/quit) rejected: quit also calls `save_prefs()`, which would persist the in-memory... actually quit saves `None` correctly since memory was cleared — but any restart between refresh and quit would resurrect the width. Explicit write-through closes the window entirely.

### D4: No load-time normalization

Like the queue width's terminal-relative max (applied in `clamp_queue_column_width`, not at load), the stored split is trusted raw and clamped by `normalize_list_pane_width` against each surface's content width at paint. A width saved on a 200-column terminal on a 90-column one simply clamps; shrinking back restores it. No migration plan needed: old binaries ignore the new key, new binaries treat its absence as default.

## Risks / Trade-offs

- [A hand-edited absurd prefs value (e.g. 65000)] → Clamped at every read by the existing normalizer; `u16` parse failure falls back to `None`.
- [`save_prefs` races with concurrent writers] → Same whole-file rewrite the queue path already uses; the split adds no new writer, only one more key on existing writes.
- [Refresh write-through adds a disk write to F5] → One small JSON rewrite on an explicit user action; same cost class as every other `save_prefs` trigger.

## Open Questions

None — the refresh-clears-persisted-state ruling (user, 2026-09-19) closed the one spec-shaping question.
