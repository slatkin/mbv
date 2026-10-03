# Proposal

## Why

While the queue artwork carries the now-playing title (title burned into the art), the queue
column's header band is pure decoration: its `[mbv] ... PLAYING:<host>` row shows nothing the
artwork and transport don't, and it permanently spends two rows the queue list could use. The
header is only meaningful when it is load-bearing — idle state, or the fallback title home on
terminals/setup where the artwork cannot carry the title.

## What Changes

- The Queue playback panel's header band (recess padding row + painted header row,
  `QUEUE_PLAYBACK_HEADER_ROWS = 2`) is removed — zero rows reserved, nothing painted — while
  playback is active and the artwork is the title site.
- The visual slot, transport, and Queue panel move up: the playback region starts at the queue
  column's top content row, and the queue list reclaims the two rows.
- The header remains exactly as today in every other state: idle (brand row + `IDLE`), and
  playing with the title on the header — the fallback states: non-kitty/sixel protocol,
  halfblock, images disabled, visualizer active, visual slot hidden, or title glyphs the overlay
  font cannot render. Art still loading or absent is *not* a fallback: the header stays hidden
  and the title is unshown until the overlay lands (or for the whole track if the art never
  arrives).
- No transient re-expansion: on every track change the overlay composes asynchronously, and the
  header does not flash back in while it composes. During that window the title is painted
  nowhere; the header stays hidden until the overlay lands. The same applies while a
  column-resize drag temporarily reverts the art to plain.
- The artwork-site header content (`[mbv] ... PLAYING:<host>` brand row, and the spec's stale
  "Now Playing" label wording) is deleted outright. The `PLAYING:<host>` remote-host label goes
  with it and is not re-homed: the library column's status-bar pill remains the in-app remote
  target indicator.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `queue-playback-panel`: the header row is no longer always painted. New rule: painted (and its
  two rows reserved) only while idle or while the header carries the now-playing title; absent
  with zero reserved rows while the artwork carries (or will carry) the title. The
  "always-painted header" mount rationale, the artwork-site header scenarios, and the
  visual-slot transport-position clauses change accordingly.
- `queue-artwork-title-overlay`: the one-rule requirement's tie-breaking changes. Uncertainty
  about whether the overlay can ever arrive (protocol, images, visualizer, slot, glyphs, art)
  still resolves to the header. But once the overlay is expected — playback active on a capable
  setup — the header is gone even before the overlay paints, so the title may be absent from
  both sites for a transient window; it is never split across both.

## Impact

- `crates/mbv-render/src/arrangements/chrome.rs` — `QUEUE_PLAYBACK_HEADER_ROWS` becomes a
  conditional input to the root placements (`ChromeGeometryInput`, `queue_column_geometry`).
- `src/app/shell/chrome_panels.rs` — `sync_queue_playback_panel` / `render_queue_playback_panel`
  slot-region offsets and the mount comment.
- `crates/mbv-components/src/queue_playback_panel.rs` — header painting gated on the new state.
- `crates/mbv-render/src/components/chrome_player/title/header.rs` — the artwork-site brand-row
  path deleted (resolves the existing spec/code drift).
- `src/app/state/projection/card.rs` — `title_site_skip_reason` classification extended to
  distinguish "overlay unreachable" (header carries the title) from "overlay pending" (header
  hidden); the projected state the shell feeds the geometry.
- `crates/mbv-ui-model/src/playback.rs` — the projected header-visibility state
  (likely a `NowPlayingTitleSite` extension or sibling field on the transport projection).
- Tests: `title_site_tests`, `tick_integration/playback_title_parts`, queue geometry and panel
  mount tests updated to the new rule.
