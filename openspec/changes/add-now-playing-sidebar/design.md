## Context

See `proposal.md` — Why. Facts that shape the approach, verified at `71750a26`:

- The left column is a single row budget. `FrameChromeGeometry.left_content` is `left_area + (2,1)`
  with height `-2` (`src/app/render/arrangements/chrome.rs:100-110`), so the window's first row is
  already blank and the card starts on row 1 (`src/app/shell_draw.rs:262-272`). The queue panel's
  rows, title row and pill row are all derived from `queue_panel_geometry` (`arrangements/queue.rs:13-45`),
  whose 1-row `gap` exists only when the card or panel occupies rows above it
  (`src/app/shell_draw.rs:332`).
- The right column reserves the strip unconditionally: `right_area.y` adds `TAB_BAR_BOX_HEIGHT +
  PLAYER_BOX_HEIGHT` and its height subtracts the same (`chrome.rs:95-108`), while `player_area` is
  gated on `right_visible` (`chrome.rs:114-122`) = `panel_mode != QueueOnly`.
- `render_player_panel` (`src/app/render/components/chrome_player.rs:31`) has exactly two callers:
  `PlaybackComponent::view` (`src/app/components/playback.rs:147-172`) at
  `layout.playback.player_area`, and `render_main`'s queue-only branch
  (`src/app/shell_draw.rs:277-330`). The component's view runs after `compose_base_frame` in the same
  draw (`src/app/shell_run.rs:79,86`), so a rect published in that pass is the rect the component
  paints.
- In `QueueOnly` `player_area` is empty on purpose (`shell_draw.rs:390`; ledger row 104, D5), so the
  component's four published rects stay `Rect::default()` and the base-frame-painted panel has no hit
  geometry at all.
- `narrow_player` is `effective_panel_mode() == QueueOnly` (`src/app/shell_playback.rs:52`). It does
  not change the glyphs: it moves the title out of the title row and paints an `On Now: <title>`
  line on the panel's bottom row (`chrome_player.rs:126-165`), and narrows the right-hand indicators
  (`chrome_player.rs:338-343`).
- Host text: `queue_title_model` (`src/app/render/components/queue.rs:62-150`) reads
  `remote_status_spans` (`chrome_status.rs:30-56`) — local → `device_name()`, attached session →
  device name then host, direct remote → `route:<name>` then the direct label then the daemon
  endpoint — uppercases it, and appends the tracking suffix to the remote label only.
- `PlaybackState { active, paused, .. }` (`src/app/types_playback.rs:30-37`);
  `effective_playback_state` prefers cast, then connected session, then local
  (`src/app/playback_target.rs:133`). `!active && paused` is unreachable.
- The idle-feed gate is `idle_feed_command_for_key(chord, player_active, has_connected_session,
  queue_only_idle, link_available)` (`src/app/action.rs:104-118`), with `queue_only_idle` computed as
  `effective_panel_mode() == PanelMode::QueueOnly` (`src/app/shell.rs:284`).
- The queue column is at least `LEFT_WIDTH_DEFAULT = 40` columns
  (`src/app/mod.rs:227`, `queue_column_width.rs:3-7`), so the sidebar panel is never the full right
  column's width.
- `SURFACE_CHROME` is already `DARK_BG` = `#1e2326` (`src/app/render/theme/mod.rs:15`,
  `theme/primitives.rs:22`).

## Goals / Non-Goals

**Goals:**

- One composition for the queue column's now-playing sidebar, produced in one place: header, visual
  slot, panel, and the queue panel below them.
- One painter for the playback panel in every layout, publishing hit geometry from the panel it
  actually painted.
- One derivation of the header's status word and one of its target label, both from shell-owned
  state, so no second painter can disagree.

**Non-Goals:**

- Removing or restyling the queue title bar. It keeps its host/connection text and tracking suffix.
- Any change to the queue list's rows, scope pills, or cursor behaviour, and any change to the
  artwork cache, prefetch, or visualizer rules (`render/components/card.rs`).
- A user toggle for the sidebar or the header: no keybind, config field, or persisted preference.
- Converging the two remaining panel presentations (sidebar vs strip) beyond the `narrow_player`
  input described in D8.

## Decisions

**D1 — The sidebar is a composition painted by the existing left-column path, not a new component.**
The header and the visual slot stay in the base frame's left-column path, and the panel stays with
the mounted `PlaybackComponent`. *Alternatives rejected:* a `NowPlayingSidebarComponent` owning the
whole slot would have to take over artwork caching, fetching, prefetch and the visualizer from the
shell (`card.rs`), which is a separate migration with its own risk and is not needed for the
requested behaviour; and the AGENTS.md rule that a destination paints "its owned surfaces" does not
require one painter per screen region when the regions are distinct surfaces.

**D2 — The header row is the first row of `left_content`, always, in queue-visible layouts.**
`QueuePanelInputs` gains `header_height` (1 when the header paints, 0 when the queue column is
hidden) rather than folding the row into `card_height`, so the visual slot's own height and the
"is anything above the queue panel" question stay separate. Consequence: the queue list gives up
exactly one row in every queue-visible layout, including idle.

**D3 — A new `NowPlayingStatus { Playing, Paused, Idle }` is derived once per frame**, next to
`effective_playback_state()`, and projected to the header. *Rationale:* `active`/`paused` are two
flags describing one three-state reading, and the header is now the second consumer of that reading
(the panel is the first). *Alternatives rejected:* passing the bool pair to the painter repeats the
derivation at the paint site; adding the enum to `PlaybackState` would store a projection as a new
fact in the type the runtime/daemon boundary already defines.

**D4 — One host-label resolver, extracted from `queue_title_model`.** A single
`playback_host_label()` returns the target label without the tracking suffix and without the queue
title's uppercasing; the queue title and the header each style it. *Rationale:* the queue title
builds its label by editing status-bar spans by index (`local_spans.get_mut(2)`), which is fragile
to reuse; a second consumer is exactly the point to extract rather than to duplicate.
*Alternatives rejected:* the header calling `remote_status_spans` itself (two derivations of one fact
will drift); the header reading the queue title model's already-processed strings (couples the
header to the queue title bar that is slated for deletion).

**D5 — The header follows the effective playback target, not the viewed queue scope.** With the
Local pill selected while attached to a session, the header names the remote target. *Rationale:*
the header is the total-state line, and the scope pills already say which queue is being viewed.

**D6 — The strip paints exactly when the queue column is hidden, and its rows are reserved only when
it paints.** In practice the strip's gate becomes `panel_mode == LibraryOnly`, which covers wide
library-only and both mini-view library states; `right_area` adds `PLAYER_BOX_HEIGHT` only then.
*Alternatives rejected:* "strip only in narrow mini view" would leave wide library-only with no
transport at all; reserving the rows unconditionally leaves a 4-row hole in the library in `both`.

**D7 — One painter, through the existing `player_area` handoff.** `render_main` publishes the live
panel rect into `layout.playback.player_area` — the sidebar slot when the queue column is visible,
and the strip rect `chrome_geometry` already computes otherwise — and the base-frame
`render_player_panel` calls in the queue-only branch are deleted. The component's
`play_pause_area` / `stop_area` / `next_area` / `seekbar_area` then come from the panel it actually
painted, which also gives the mini-view queue-only panel the hit geometry it has never had.
*Alternatives rejected:* keeping the base-frame painter and publishing the rect only for clicks
(two painters, one geometry, guaranteed drift); mounting a second component per slot (two owners of
one surface, against the one-owner rule).

**D8 — `narrow_player` becomes "this is the sidebar panel"** (true whenever the queue column is
visible) instead of "the panel mode is queue-only". *Rationale:* the panel the user is converting is
today's narrow mini-view panel, and the sidebar is only ~40+ columns wide in `both` too, so the same
presentation applies in every queue-visible layout; making it mode-dependent would reintroduce the
per-layout divergence this change exists to remove. The strip keeps the full-width presentation.

**D9 — Panel placement inside the sidebar keeps the retired capability's arithmetic.** Below the
header: stacked below the visual slot under 100 columns; side by side at 100+ with the visual slot
left-aligned, a 2-cell gap, panel height `max(visual slot height, player height)`, content
top-aligned, panel background filling the remainder. The panel's background follows the queue
column's focus resolution as it does today.

**D10 — Idle means the visual slot and the panel, in every queue-visible layout.** The
`show_controls` exception in `render_main` (`shell_draw.rs:276`) and the matching spec requirement
are deleted: a connected-but-idle transport keeps only the header. The idle-feed gate's input becomes
"the panel is collapsed" rather than "the panel mode is queue-only" (`shell.rs:284`,
`action.rs:104-118`), and `idle-feed-rotation` is deltaed accordingly.

**D11 — No new theme role.** The header paints on `SURFACE_CHROME`, which already carries this
meaning and this value (`#1e2326`). *Alternative rejected:* a `SURFACE_NOW_PLAYING_HEADER` alias of
the same primitive would be a second name for an existing role; the design-system rule to add a role
applies when no existing role fits, which is not the case here. The same reasoning applies to the
header's text: reuse `PLAYBACK_VALUE_FG` for the status word and `TEXT_METADATA` for the target.

**D12 — `queue-only-playback` is retired rather than renamed.** OpenSpec has no capability rename,
the capability's subject ("exists only in queue-only") no longer exists, and the archive workflow
deletes a main spec whose last requirement is removed. Its surviving rules are re-homed into
`now-playing-sidebar` with their meaning intact.

## Risks / Trade-offs

- **The idle feed title loses its host row in every queue-visible layout** (it can render only where
  the panel renders, which is now the strip) → accepted: rotation continues, the strip still shows
  it in library-only, and the open-link command follows the display. Giving the idle feed a home in
  `both` is a follow-up that would change the header's contract, so it is deliberately not guessed
  at here.
- **The queue list loses one row and the library shifts by 4 rows on `x`** (the strip reservation is
  no longer constant) → mitigations: the header row is counted inside `queue_panel_geometry`'s inputs
  so the queue panel cannot be painted over, and narrow terminals must be checked against the
  `title_reserved` / `status_overhead` thresholds at 24 rows (`arrangements/queue.rs:32-36`), where
  one lost row can drop the queue title row.
- **The mouse fix changes behaviour in mini-view queue-only** (clicks that did nothing now act) →
  intended; pin it with a test so a later refactor cannot silently drop the published geometry again.
- **Two panel presentations remain** (`narrow_player` true/false), so a future change to one will not
  reach the other → accepted: they are genuinely different widths, and the flag is the single place
  that records it.
- **File overlap with in-flight changes** `unify-wide-hero-content-box-frame` (left-column content
  area, queue-column width) and `add-media-list-multi-select` (Queue row gestures) → no requirement
  conflict; sequence after them or rebase.
- **Two delta scenario names are retained while their outcome flips** (`Idle queue-only shows no feed
  title`, `Open-link unchanged in two-panel idle`): the OpenSpec delta format requires a MODIFIED
  requirement to re-state the scenarios the main spec already has, so the names persist even where
  the new rule deliberately changes what they assert. Their bodies state the new outcome; the names
  are pre-existing and are not worth a separate rename change.

## Migration Plan

No persisted state, protocol, or config migrates: panel mode and the sidebar are per-session, and
nothing new is written to disk. Rollback is reverting the change's commits. Within the change the
painter move must be atomic per layout (publish the rect and delete the base-frame call in the same
step), because a half-applied state paints the panel twice.

## Open Questions

- Whether the header later absorbs the queue title's host/connection text so the queue title bar can
  be deleted. Deferrable: it is a subtraction of an existing surface and needs no change to this
  change's specs, approach, or tasks.
