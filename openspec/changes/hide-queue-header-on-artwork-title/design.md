# Design

## Context

The Queue playback panel's header band is a constant: `QUEUE_PLAYBACK_HEADER_ROWS = 2`
(`crates/mbv-render/src/arrangements/chrome.rs`) is baked into the root placements
(`queue_column_geometry` → `queue_panel_geometry` via `header_height`), and into the slot-region
offsets of `sync_queue_playback_panel` and `render_queue_playback_panel`
(`src/app/shell/chrome_panels.rs`). The panel component
(`crates/mbv-components/src/queue_playback_panel.rs`) picks its header content from
`transport.title_site`: `Header` paints the title, `Artwork` paints the `[mbv] ... PLAYING:<host>`
brand row (`artwork_brand_spans` in `chrome_player/title/header.rs`).

The title-site decision already exists as `title_site_skip_reason`
(`src/app/state/projection/card.rs`) — an enumerated list of blockers with a logging gate — and
`resolve_title_site` compares the composed variant key against the last painted overlay key.
`sync_queue()` calls `refresh_queue_card_image()` (which updates
`app.queue_card_projection`) before `sync_queue_card_geometry()` and
`sync_queue_playback_panel()` compute the frame, in the same sync pass, so fresh projection state
is available to geometry without a staleness workaround (unlike `card_height`, which is
last-frame-published by necessity).

See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**
- Zero reserved rows and zero painting for the header band while playing on the artwork title
  site, including the overlay-compose window after track changes.
- The queue list reclaims the two rows.
- The header stays exactly as today while idle or in persistent fallback states.

**Non-Goals:**
- No re-homing of the `PLAYING:<host>` remote-host label (the library column's status-bar pill
  remains the remote indicator; the label dies with the row).
- No change to the transport band, visual slot painting, the Library playback panel, or the
  overlay composition pipeline itself.
- No new keybind or setting: the behavior is unconditional.

## Decisions

### D1: Split the skip reasons into "unreachable" vs "pending", and drive one projected flag

Header visibility while playing is decided by whether the overlay can **ever** arrive, not by
whether it has painted:

- **Unreachable** (header visible, carries the title): images disabled, visualizer active,
  halfblock configured, image protocol disabled, visual slot hidden by the user, and titles the
  overlay font cannot cover. These are user/terminal/title-level states — stable for the session
  or the track.
- **Pending** (header hidden, title nowhere until the overlay lands): art not yet projected or
  decoded, card size unmeasured while the protocol is enabled, no active item, no title, overlay
  composed but not yet painted, column-resize drag. These are art-loading/composition windows
  that resolve toward the artwork site.

The existing `title_site_skip_reason` enumeration conflates the two classes and must be
decomposed, not consumed as-is: `NoSlot` ORs `!visual_slot_shown()` (unreachable) with
`cache_key.is_none()` (pending), and `NoBaseProtocolSize` ORs `!protocol_enabled()`
(unreachable) with a zero card size (pending). The classification reads those inputs separately.

`refresh_queue_card_image` resolves the classification and publishes a single boolean on the
projection the shell consumes — `queue_card_projection.header_visible` (or equivalent field next
to `title_site` in `mbv-ui-model`'s projection types). Idle is not part of the classification: the
shell ORs idle in, because the idle header is governed by the idle rule, not the title rule.

**The classification must run every sync pass, ungated.** `sync_queue` currently gates
`refresh_queue_card_image` on `visual_slot_shown()`, so with the slot hidden the projection (and
any flag inside it) goes stale. Split the refresh: the header-visibility classification runs on
live App state every sync pass (it needs only playback state, images/protocol/visualizer config,
slot visibility, and the title text); the fetch and overlay-compose work stays gated on the slot
being shown, exactly as today.

Alternative considered: keep using the painted-reality `title_site` for header presence. Rejected
— that is exactly the transient-flash behavior this change removes (the site reads `Header` until
the overlay paints, so the header would reappear on every track change). The painted/expected
distinction collapses once the header cannot exist in the artwork state: the panel painter no
longer needs `title_site` at all (see D3).

`NoTitle` classifies as pending: there is no title to carry, so a hidden header loses nothing
(the transport indicators still state playback). Art loading or absent also resolves to pending:
a track whose artwork never loads shows its title nowhere. That is the accepted
consequence of no transient re-expansion, and the spec records it.

### D2: Geometry takes a `header_rows` input; the shell computes it once

`ChromeGeometryInput` gains `header_rows: u16`, fed by one App-level helper (e.g.
`queue_header_rows()`) that returns `QUEUE_PLAYBACK_HEADER_ROWS` when idle or `header_visible`,
else `0`. Every `ChromeGeometryInput` construction site consumes that helper — `App::
compute_chrome_geometry` (`src/app/shell/draw.rs`, which `sync_chrome_root` and the paint path
both route through) and `right_panel_lib_area` (`src/app/state/projection/tv_wide.rs`) —
so the two breakpoints cannot disagree. `queue_column_geometry` forwards the value as
`queue_panel_geometry`'s existing `header_height` input (no new geometry math — the arrangement
already treats header height as an input). The slot-region offsets in
`sync_queue_playback_panel` and `render_queue_playback_panel` read the same helper rather than
the const, so sync, paint, and geometry cannot drift.

Alternative considered: pass a boolean and let the arrangement apply the const. Rejected — the
arrangements stay policy-free; the mount/visibility rule is shell-owned projection state (same
boundary the interactive framework draws: components and painters consume projected decisions,
they don't make them).

`queue_playback_rows()` is untouched: it already excludes header rows; the playback placement's
height becomes `header_rows + queue_playback_rows(...)`, and `placed_when`'s zero-height filter
keeps working (idle reserves the 2-row header-only placement; artwork-site playing reserves
slot+transport rows only).

### D3: Delete the artwork-site header painter path, don't branch on it

With the header absent whenever the artwork carries the title, `render_header_title`'s
`Artwork` branch — `artwork_brand_spans`, the `PLAYING:<host>` brand row, and the
`HeaderTitle.title_site`/`host`/`host_is_remote` fields feeding it — becomes dead. Delete the
branch and the fields. The panel's `view()` gates on the projected visibility flag: paint the
header row iff visible; the branch content is the existing title/status-word logic.

This also resolves the pre-existing spec/code drift in one direction: the spec's artwork-site
"Now Playing" label wording and the code's brand row both describe a row that no longer exists.
`NowPlayingTitleSite` itself may shrink back to a single-meaning projection detail; if nothing
else consumes it after the painter cleanup, fold its remaining signal into the new flag and
delete the enum.

### D4: Mount rule wording, not mount mechanics

The `QueuePlaybackPanel` stays mounted to the `queue_playback` placement in every queue-visible
layout (the placement exists whenever slot/transport rows or the idle header exist). Only the
D10-derived comment "mounted … because the header is always painted" changes — the mount is
placement-driven, and placement presence now varies with `header_rows`. No TuiRealm lifecycle
change.

## Risks / Trade-offs

- [Per-track 2-row layout shifts for tracks that flip the fallback] → A queue mixing
  art-ful and art-less (or glyph-uncoverable) tracks shifts the queue panel by two rows between
  tracks. Inherent to reclaiming the rows; the spec's track-change scenario pins the
  no-flash-while-composing rule, and fallback flips (visualizer, images toggle) are
  user-initiated. Accept.
- [Projection-to-geometry ordering regression] → `sync_queue` must keep refreshing the card
  projection before `sync_queue_card_geometry`/`sync_queue_playback_panel`; if a future reorders
  the sync pass, header visibility silently lags a frame. Guard with the projection-order note
  in `run.rs`'s sync list comment.
- [Title nowhere on art-less tracks] → Chosen (D1); the overlay pipeline's
  `queue.title_overlay.decision` log gate already records per-track skip reasons, so a confused
  user can be diagnosed from the log without new instrumentation.
- [Marquee state reset] → The panel's marquee buffers survive the header's absence unmounted
  content; they are component-local and re-seed from text. No action.

## Migration Plan

Single-binary behavior change, no persisted state, no protocol impact: deploy and roll back by
reverting. Geometry is recomputed every sync pass from projected state, so there is no stored
layout to migrate.
