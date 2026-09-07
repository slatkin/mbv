## Why

The queue now-playing row is crowded: it renders the title, an inline FOAM progress percentage, and a green `elapsed / total` duration at once — three competing elements where every other row renders two. Standardising the now-playing row (no elapsed, progress moved right to replace the duration, braille throbber prefix) gives "what's playing" one unmistakable geometry and makes the row projection cheap to fingerprint, since active-row strings stop depending on wall-clock position.

## What Changes

- **Elapsed removed**: the now-playing row no longer shows elapsed time; its duration slot is empty.
- **Progress moves right**: live progress renders in the right-aligned slot, replacing the duration entirely, only for the now-playing row. All other rows keep progress inline where it is.
- **Braille throbber**: an 8-frame braille spinner (`⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧`) precedes the progress in the right slot, painted aqua (new theme role over the AQUA primitive). Unknown runtime shows the throbber alone with no percentage.
- **New `NowPlaying` row variant**: `MediaSemanticState::NowPlaying { progress }` distinguishes live playback from stored resume state. Queue projects it; browser resume rows keep projecting `Active` with unchanged inline rendering.
- **Paint-time animation**: the shell resolves the current throbber glyph (existing 300 ms advance, gated on active && !paused) and passes it as a paint-time parameter through `render_wide_media_list` → `wide_media_row`. Animation frames never invalidate the row projection.
- **Authoritative revision + fingerprint-gated projection**: `QueueRevision` becomes authoritative for every row-affecting mutation (missing bumps added in `mbv-core`, including refresh/item/progress paths; replacement invalidates). Queue rows rebuild only when the `(viewed_scope, revision, playback.active, projected active target, progress-% bucket)` fingerprint changes; the no-change tick path performs no slot clone, row projection, or row push. On a %‑bucket change only, the active row is patched in place. Cursor pushes and scope/chrome/title delivery are split out of the row path and never gated.
- **Out of scope**: lighting up the now-playing style in other media lists (Home, Feeds, browser, TV, music, Audiobookshelf) — recorded follow-up, lands separately. The legacy two-column `render_plain_rows` path is untouched.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `queue-canonical-list`: now-playing row projection changes (no elapsed, `NowPlaying` state, right-slot progress replacing duration), live progress source, predicted-selection no-progress rule, fingerprint-gated refresh.
- `canonical-media-lists`: new `NowPlaying` semantic variant with bounded progress, painter rule for the now-playing geometry (right-slot throbber + progress, duration suppressed), paint-time throbber parameter, aqua throbber theme role.

## Impact

- `src/app/components/queue.rs` (`queue_media_rows`, `queue_row_time_text` active path), `src/app/components/media_list/mod.rs` (new variant), `src/app/render/components/media_list/wide.rs` + `wide_row.rs` (throbber param, `NowPlaying` geometry), `src/app/shell_queue.rs` (fingerprint gate), `src/app/shell_run.rs` (throbber advance already exists — reused), `src/app/render/theme/mod.rs` (new role), queue/component/row buffer tests.
- Tracks issue #675 (unconditional per-tick queue re-projection): this change implements the efficient path for the queue rows it touches rather than rebuilding every tick.
