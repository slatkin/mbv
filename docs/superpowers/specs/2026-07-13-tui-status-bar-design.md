# TUI status bar — design

Issue: #193

## Problem

`App::render` (`src/app/render/mod.rs`) has no row anchoring the bottom of
the terminal. The vertical layout is `tabs_area` → `seek_area` → `title_area`
→ `main_area`, and `main_area` uses `Constraint::Min(0)`, which consumes
every remaining row down to the literal bottom edge of the terminal. Nothing
terminates the screen visually.

The only thing that currently touches that space is the toast mechanism
(`self.status: String`, `self.status_expires: Option<Instant>`,
`toast_line()`): when a message is set, `render()` draws a 3-row `Clear` +
IRIS-background overlay pinned to `area.y + area.height - 3`, floating on
top of whatever `main_area` content is underneath. It's transient and
covers content rather than living in reserved space.

Separately, some app state that's genuinely useful to see at a glance has
no persistent on-screen representation, or only a low-signal one:

- `RemoteSlotState` (`Off` / `AttachedSession` / `DirectRemote` /
  `LocalDaemon`) — today represented only by the color/boldness of the `⇌`
  glyph in the control pill, no text label.
- Stay-alive mode, suspended-local-session.
- Queue state: dirty flag (unsaved changes), saved-playlist vs. ad-hoc
  queue, autosave-on-consume, local vs. remote queue scope — today these
  surface only indirectly via tab pills.

## Goals

- Add a persistent bottom row that's always present, regardless of tab or
  playback state, giving the screen a visual floor.
- Absorb the toast mechanism into that row instead of overlaying content.
- Surface session/connection state and queue state that are currently
  invisible or low-signal, without duplicating information that's already
  obvious elsewhere (e.g. current tab/breadcrumbs are not repeated here).

## Non-goals

- Moving the res/audio/subtitle indicator chips (title row) — those stay
  put; only the control pill relocates.
- Moving the `VOL` badge — stays in the tab row.
- Per-tab navigation context (breadcrumbs, item counts, sort/filter state)
  — already visible in the relevant panel, would be redundant here.

## Design

### Layout

Add one `Constraint::Length(1)` row at the bottom of the vertical layout in
`App::render`, after `main_area`:

```
tabs_area, seek_area(0/1), title_area(0/1), controls_area(0/1), main_area(Min(0)), status_bar_area(Length(1))
```

`main_area` shrinks by 1 row to make room. The row is unconditional — it
renders every frame, on every tab, whether or not anything is playing.

### Content: two segments, vim-statusline style

**Left segment (always shown):**

1. The control pill, relocated from the far-left of `tabs_area`
   (`render_control_pill`) to the far-left of the new status bar: `m`
   (mute) / `⇌` (remote-control mode) / `≡` (playlist-backed queue).
2. A text label for session/connection state next to the pill:
   `RemoteSlotState` (Off is blank/omitted, AttachedSession /
   DirectRemote / LocalDaemon get a short label) plus stay-alive mode when
   active. This gives the `⇌` glyph the text label it currently lacks.

**Right segment (shown only on the Queue tab and Power View, blank on
other tabs):**

Queue state: dirty flag (unsaved-changes marker), saved-playlist vs.
ad-hoc queue indicator, autosave-on-consume status, and queue scope
(local vs. remote) when a direct-remote queue exists.

### Toast behavior

When `self.status` is non-empty and unexpired, it takes over the **full
width** of the status bar temporarily (matching today's visual weight),
replacing both segments. `toast_line()` renders unchanged (same
bracket/paren-highlighting logic), just targeting the new 1-row rect
instead of the old 3-row overlay rect. `status_expires` continues to drive
when it reverts to the normal two-segment content. The old `Clear` +
3-row-overlay block in `render()` is deleted.

### What moves, what doesn't

| Element | Today | After |
|---|---|---|
| Control pill (`m ⇌ ≡`) | far-left of `tabs_area` | far-left of status bar |
| Toast (`self.status`) | 3-row overlay covering `main_area` | full-width status bar content, same row |
| Res/audio/sub chips | title row | unchanged |
| `VOL` badge | tab row, right-aligned | unchanged |

## Open questions for the implementation plan

- Exact text labels/abbreviations for each `RemoteSlotState` variant and
  stay-alive mode (short enough to fit alongside the pill on narrow
  terminals).
- Exact glyphs/labels for the queue-state segment (dirty marker, saved vs.
  ad-hoc, autosave-on-consume, scope).
- Narrow-terminal truncation order when both segments would overflow the
  width (which segment yields first).
