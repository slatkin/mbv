# Tasks

## 1. Projection: the header-visibility classification

- [x] 1.1 Add the header-visibility flag to the projection types (`QueueCardProjection` or its
  sibling in `mbv-ui-model`), and classify the title-site inputs in
  `src/app/state/projection/card.rs` per design D1: unreachable (images disabled, visualizer
  active, halfblock configured, protocol disabled, visual slot hidden, title glyphs the overlay
  font cannot cover — flag true) vs pending (art not projected or decoded, unmeasured card size
  with the protocol enabled, no active item, no title, overlay not yet painted, column-resize
  drag — flag false). Decompose the conflated skip reasons rather than consuming them as-is:
  `NoSlot` ORs slot-hidden with a missing cache key, `NoBaseProtocolSize` ORs protocol-disabled
  with a zero card size. Idle is not classified here; the shell ORs it in. Verify: extend
  `title_site_tests` with the contract "the flag reads the decomposed class, not the painted
  overlay": a not-yet-painted overlay on a capable setup keeps the flag false; images off /
  visualizer / halfblock / slot hidden / protocol disabled / uncovered glyphs flip it true.
- [x] 1.2 Make the classification run every sync pass: `sync_queue`
  (`src/app/shell/queue.rs`) currently gates `refresh_queue_card_image` on
  `visual_slot_shown()`, so with the slot hidden the projection goes stale. Split the refresh so
  the classification runs ungated on live App state while the fetch and overlay-compose work
  stays slot-gated, exactly as today (design D1). Verify: harness test with the contract
  "hiding the visual slot while the artwork carries the title flips the flag to true on the next
  sync pass" (the header returns — the fallback rule).
- [x] 1.3 Audit the remaining `NowPlayingTitleSite` consumers after the flag exists (transport
  projection field at `src/app/shell/playback.rs:82`, `HeaderTitle.title_site`,
  `title_site_tests`). If the painter cleanup in 3.2 leaves the enum without a live consumer,
  delete the enum and its projection field; otherwise leave it and record why. Verify:
  `cargo check --workspace` plus `cargo nextest run -p mbv-ui-model -p mbv` still pass.

## 2. Geometry: conditional header rows

- [x] 2.1 Add `header_rows: u16` to `ChromeGeometryInput` and forward it through
  `queue_column_geometry` into `queue_panel_geometry`'s existing `header_height` input
  (`crates/mbv-render/src/arrangements/chrome.rs`); delete the unconditional
  `QUEUE_PLAYBACK_HEADER_ROWS` use there. Feed it from one App-level `queue_header_rows()`
  helper consumed by every `ChromeGeometryInput` construction site — `App::compute_chrome_geometry`
  (`src/app/shell/draw.rs`) and `right_panel_lib_area`
  (`src/app/state/projection/tv_wide.rs`) — returning 2 when idle or the flag is true, else 0.
  Keep `sync_queue` refreshing the projection before `sync_queue_card_geometry`/
  `sync_queue_playback_panel` (note the ordering in the `run.rs` sync-list comment). Verify:
  arrangement-level tests only (pure `queue_panel_geometry`/`queue_column_geometry` inputs and
  outputs) with the contract "the playback region reserves zero header rows when `header_rows`
  is 0 — the queue placement starts at the column's top content row and gains the two rows —
  and keeps today's placements at 2".

## 3. Shell sync, paint, and the dead artwork-site header path

- [x] 3.1 Replace the `QUEUE_PLAYBACK_HEADER_ROWS` offsets in `sync_queue_playback_panel` and
  `render_queue_playback_panel` (`src/app/shell/chrome_panels.rs`) with the shared
  `queue_header_rows()`; update the mount-rule comment (mounted placement-driven, D10's
  "always-painted header" rationale gone). Verify: shell harness test (the panel mount/placement
  family) asserting shell wiring only — the mounted `QueuePlaybackPanel`'s projected
  `transport_area` and the queue panel placement shift by exactly two rows when the flag flips,
  and are unchanged while idle. (Geometry math itself is owned by 2.1's arrangement tests; do
  not duplicate them here.)
- [x] 3.2 Gate `QueuePlaybackPanel::view`'s header painting on the projected flag
  (`crates/mbv-components/src/queue_playback_panel.rs`): paint the header row iff visible, keep
  the existing title/status-word content otherwise. Delete `render_header_title`'s Artwork
  branch — `artwork_brand_spans`, the `HeaderTitle.title_site`/`host`/`host_is_remote` fields,
  and the brand-row import in `queue_playback.rs` if now unused (design D3; this also deletes
  the spec-drifted artwork-site row). Verify: `queue_playback_panel/tests.rs` contract "the
  header row paints iff the flag; no playing state paints a brand row or `PLAYING`/host text".

## 4. Behavior seams: track change and fallback flips

- [x] 4.1 Pin the no-flash rule end to end: on playback start and on track change with
  overlay-capable art, the header stays hidden from the first frame (no transient header while
  the overlay composes); toggling the visualizer or images off re-shows it; hiding the visual
  slot re-shows it; returning to artwork hides it again. Update
  `tick_integration/playback_title_parts.rs` expectations to the new rule. Verify: the harness
  tests assert header presence across the compose window (mock the overlay landing as an
  injected outcome — no real sleeps or image pipeline). This task owns the behavioral
  seam assertions only — geometry numbers belong to 2.1, shell wiring to 3.1.

## 5. Integration checks

- [ ] 5.1 Sweep tests that assume the always-painted header (`src/app/tests/panel_focus.rs`,
  `tick_integration/mouse/panels.rs`, `tick_integration/queue_playback.rs`, queue geometry
  tests, `queue_op.rs` now-playing assertions) and update them to the conditional rule. Verify:
  `cargo nextest run --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`
  pass; `cargo fmt`; files touched stay under 800 lines (`make check-code-file-lines`) —
  `card.rs` is at 661, the closest to the cap; if the classification pushes it over, split along
  the skip-reason/classification seam.
