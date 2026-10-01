# Design

## Context

See `proposal.md` for motivation and `specs/queue-playback-panel/spec.md` for the behavior contract.

Observed in the current code:

- `chrome_player.rs::seek_fill()` clamps the tick-derived ratio and rounds it to whole cells.
  Both the Library and Queue painters currently call it.
- `title/queue_band.rs::render_queue_seek_row()` already owns the Queue bar rectangle, time
  formatting, spacing, narrow fallback, and returned seek region. It paints dense-shade
  filled characters and light-shade unplayed characters.
- The Library painter independently paints upper-line characters in `chrome_player.rs`.
- `mbv-render` already depends on Ratatui 0.30. The installed `ratatui-widgets` 0.3.2 Gauge
  supports `.use_unicode(true)`: full blocks plus a fractional leading block rounded to
  eighths. It supplies a percentage label by default unless explicitly disabled.
- Existing Queue interactive tests in `queue_playback_panel/tests.rs` assert time placement,
  quarter fill, foreground roles, and geometry using the old shade characters. Separate
  mouse tests own semantic seek requests. There is no fractional-progress guard.

A design artifact is warranted to settle Queue-only reuse, track styling, redraw behavior,
and test ownership before replacing the painter's existing mixed time/bar line.

## Goals / Non-Goals

**Goals:** Reuse the installed Gauge only inside the Queue bar's existing rectangle; preserve
geometry and define how its full-height track and partial edge use existing theme roles.

**Non-Goals:** Generalize the two seekbars, alter shared progress state, or introduce a
fractional-fill algorithm, render abstraction, dependency, or configurable presentation.

## Decisions

### 1. Reuse Gauge locally; leave the Library helper alone

Replace only the Queue painter's whole-cell shade spans with `Gauge`, using the clamped
position/runtime ratio, `.use_unicode(true)`, an explicitly empty label, and no border.
Compute the ratio directly from the supplied ticks; never first truncate position to
seconds or whole cells. A non-positive runtime yields zero. With a positive runtime,
clamp position/runtime to `[0, 1]` before passing it to Gauge.

Keep `seek_fill()` and `render_seekbar()` unchanged in `chrome_player.rs`. Remove only
Queue's now-unused `seek_fill` import. This respects the explicitly confirmed Queue-only
scope without a mode flag in a shared helper.

Alternative: reproduce the article's block lookup manually. Rejected because Gauge
already handles the partial cell, width bounds, and full endpoint. Alternative: change
the shared helper to return fractional fill. Rejected because the Library bar is out of
scope and cannot use the same full-height characters without a visual change.

### 2. Paint a solid muted track, not a mismatched shaded partial cell

Use `ACCENT` as the Gauge foreground and `SURFACE_BACKDROP` (`#272e33`) as its background.
This gives a solid slate rail underneath full blocks and underneath the unfilled portion of
the edge cell. The time labels and surrounding spaces keep their current metadata foreground
and panel background. The shared theme values and Library `PROGRESS_TRACK` role remain
unchanged.

Keep the existing label measurement and bar rectangle. Paint the full time row with a
space placeholder occupying the bar span, then render the Gauge into that span. This
clears old fill on every paint before Gauge applies the new fill, including a backward
seek or resize; Gauge alone does not clear every old symbol in its unfilled area. Retain
the existing inactive branch and time-only fallback without introducing a seek target.

Alternative: keep the light-shade unplayed glyphs. Rejected because a fractional block
cannot simultaneously contain an unplayed shade glyph within the same cell, causing a
visually inconsistent leading edge. No percentage, border, or additional row is needed.

### 3. Separate visual precision from input precision

The Interactive Component continues receiving exactly the bar rectangle the painter uses.
Its terminal-column click fraction remains unchanged. Eighth-cell glyphs improve displayed
position, not mouse event granularity. No player, projection, input-routing, protocol,
persistence, or Service code needs modification.

For width `W` and positive runtime `D`, adjacent eighth-cell states are separated by
approximately `D / (8 * W)` seconds of playback. Rounded fill first becomes visible at
approximately `D / (16 * W)`: about 7 seconds for a 24-cell, 45-minute timeline, versus
56 seconds with the existing whole-cell rounding. These are rendering thresholds, not
promises about when a playback target supplies its next status update.

### 4. Test the mbv integration, not Gauge's implementation

The Render Component owns fractional glyphs, colours, time placement, and paint-local
bar geometry. Add a small hermetic render test module at `title/queue_band/tests.rs` and
move affected existing paint assertions there rather than retaining duplicates in the
Interactive Component tests. Cite this change for the early-progress regression and
retain the existing `7fdb7fee8` provenance for the time/geometry regression.

The key new guard paints the actual Queue seek row at 10 seconds of 45 minutes with a
24-cell bar and verifies a one-eighth accent edge over the track. Whole-cell rounding
would leave it empty. Repaint that same fixture from a later position to the early
position to catch stale accent fill. Keep fixture construction in helpers, not branches
or loops in test bodies. Do not enumerate all eight Gauge characters or test Ratatui's
own rounding routine.

Existing Interactive Component pointer tests retain semantic seek/no-seek ownership;
remove or adapt helpers that identify hit geometry by obsolete shade glyphs. A focused
render guard for the unchanged Library thin bar protects the explicit scope boundary
without duplicating Library pointer coverage. No real player, live server, sleep,
filesystem state, or whole-application snapshot is needed.

## Risks / Trade-offs

- Fractional blocks are font-dependent and visually heavier than shade characters ->
  use standard Unicode Block Elements, already usable in the terminal UI; verify appearance
  manually with the user, without adding terminal probing or a configuration switch.
- A default Gauge percentage or stale symbols would corrupt the seek row -> explicitly
  disable its label and clear the row before drawing the Gauge.
- Reusing the shared whole-cell helper would lose the improvement or widen scope ->
  Queue gets the raw clamped ratio; Library's implementation remains unchanged.
- Very long media or very short bars still have perceptible steps -> document eightfold
  resolution, not continuous or immediate fill. Do not fabricate a minimum fill or animate
  between playback reports.
- Solid track changes the visual texture of the Queue bar -> confine it to the active
  Queue bar; labels, layout, palette roles, and the Library line remain unchanged.

## Migration Plan

No data or configuration migration is required. Ship the painter change with its focused
regression coverage as one implementation slice. After review and acceptance, sync the
delta into `queue-playback-panel` and archive the change using the normal OpenSpec workflow.
Rollback restores the previous Queue painter and corresponding assertions; no persisted
state or wire compatibility is involved.
