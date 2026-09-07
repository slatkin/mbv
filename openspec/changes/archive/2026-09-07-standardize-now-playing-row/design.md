## Context

See proposal.md (Why) for motivation. Current state: `queue_media_rows` (`src/app/components/queue.rs`) projects the active row as `MediaSemanticState::Active` with `trailing: None` (the painter synthesises inline FOAM `%`) plus `duration: "elapsed / total"` via `queue_row_time_text(pos, dur, show_elapsed=true)`. `wide_media_row` (`src/app/render/components/media_list/wide_row.rs`) is a pure function of `(row, selected, focused, bg, width, scrollbar)` — no clock reaches it. `sync_queue` (`src/app/shell_queue.rs`) runs every tick via `sync_mounted_surfaces` and unconditionally clones all slots, rebuilds all rows, and pushes them through `WideMediaList::set_content` (issue #675). The shell already advances `now_playing_throbber_index` every 300 ms gated on `active && !paused` (`src/app/shell_run.rs`) for the chrome throbber. Browser resume rows project `Active` from stored ticks (`emby_semantic_state`) and must not change.

## Goals / Non-Goals

**Goals:**

- The queue now-playing row shows title + right-aligned `glyph pct`, no elapsed, no inline progress; all other rows byte-identical.
- Animation costs one `char` per paint; the no-change tick performs no slot clone, no row projection, and no row push. Terminal repaint itself is unchanged (still repaints on the render cadence) — only projection work is gated.
- Resume-progress rows (browser `Active`) render exactly as today.
- `QueueRevision` is authoritative for every row-affecting mutation, so the fingerprint cannot go stale.

**Non-Goals:**

- Now-playing styling in non-queue lists (recorded follow-up, separate change).
- The legacy two-column `render_plain_rows` path (untouched; queue never uses it).
- Gating terminal repaint (out of scope; only projection is gated).

## Decisions

- **`NowPlaying` variant, not a flag.** `MediaSemanticState::NowPlaying { progress }` alongside `Active`. Live playback and stored resume are different semantics sharing a format string today; a variant makes every `match` on the enum fail to compile until updated, so resume-vs-live can never silently re-merge. Alternative (bool on the row) was rejected: it leaves the ambiguity in the type.
- **Throbber glyph resolved in the shell, passed as a paint-time `Option<char>`.** `render_wide_media_list` and `wide_media_row` gain a trailing `throbber` param; non-queue callers pass `None`. Alternative (bake `"⠋ 47%"` into the projected string at sync time) was rejected: it makes every 300 ms animation frame a projection invalidation, which is exactly the per-tick rebuild cost #675 exists to remove.
- **Glyph aqua, percentage FOAM.** The throbber gets a new theme role over the AQUA primitive (`render/theme`); the percentage keeps `TEXT_METADATA` (FOAM). So the right slot is two spans: aqua glyph + FOAM pct. Colour grammar stays intact (blue = progress, aqua = liveness, green = duration — and green no longer appears on this row at all).
- **Revision made authoritative first, then the gate.** `update_slot_item`, `apply_progress`, `merge_fetched_slot`, and any other row-affecting `PlaybackQueue` mutator that does not bump revision gains a bump; queue replacement invalidates previously issued revisions. Only then does the gate compose `(viewed_scope, revision, playback.active, projected active target, progress-% bucket)` from `&PlayerTab` before any clone. Alternative (gate on revision-as-is plus live scalars, no `mbv-core` change) was rejected by review: unbumped mutators change projected titles/durations/resume badges, so the gate could leave stale rows; scope switches and replacements can reuse revision/slot IDs, so scope and replacement-invalidation must be in the fingerprint.
- **Bucket-change patches one row.** A new `ListCore` in-place item update (targeted at the now-playing display row) handles the progress-bucket-only case; structural changes (revision/active-index) take the full `set_content` rebuild. List identity reconciliation (`selected_target` preservation) is untouched.
- **Elapsed removal is what makes the fingerprint cheap.** After the active row projects `duration: None` and progress only as a bucketed integer, no active-row string depends on wall-clock position — so the fingerprint is stable between %‑bucket crossings by construction.

## Risks / Trade-offs

- [Risk] Right-slot content changes title truncation widths; existing golden buffer tests (wide-row regression, queue component tests) will need output updates → Mitigation: add the red-before-green now-playing buffer test first; update only goldens whose rows are now-playing rows, and treat any other golden change as a bug.
- [Risk] Browser `Active` (resume) shares the painter match; a careless arm could alter resume rows → Mitigation: the `Active` arm is not touched, and existing browser resume tests act as the regression net (pinned in tasks).
- [Risk] 8-frame braille width: braille cells are single-column in `unicode_width`, but verify — a double-width glyph would break the right-align math → Mitigation: assert single width in the painter test.
- [Risk] A future queue mutator changes projected rows without bumping revision and the list goes stale → Mitigation: revision-authoritative is pinned by a mutation-matrix test (every production mutator enumerated; each row-affecting one must advance the generation); a code comment at the gate site requires new mutators to bump revision.
- [Trade-off] Two spans (aqua + FOAM) in the right slot cost one extra span vs a single-style slot; accepted — colour carries meaning here.

## Migration Plan

No persistence, wire, or config changes. Rollback is a revert; no migration steps.
