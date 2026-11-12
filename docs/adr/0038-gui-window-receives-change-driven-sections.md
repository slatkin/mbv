---
status: proposed
---

# The GUI Window Receives Change-Driven Sections

The GUI Client's core thread sends the window only the sections whose content
changed, each with its own revision, in one batch for each tick. It sends no
update because time passed. The window keeps one store for each section,
predicts the playhead between anchors, and loads artwork by URL from the core's
image cache.

## Decision

- **Batches of sections.** One `mbv://sections` event carries the window
  sections that changed in one tick. A section that the batch does not name is
  unchanged. A batch is atomic: the sections it carries agree with each other.
- **Change, not time, decides an emit.** The core thread keeps its tick timer,
  because the shared tick has time-based phases. A tick that changed no section
  sends nothing. Small sections are built on each tick and compared. Expensive
  sections are built only when a dirty mark says that one of their sources
  changed. Inputs, the tick host's drain hooks, named deadlines, and the
  playhead marks set the marks. A test-only audit rebuilds the clean sections
  and fails on a missed mark.
- **The window asks for a snapshot.** The window calls `sync` when it mounts,
  and the core answers with every present section. A layout report applies the
  layout and re-arms nothing.
- **The playhead is predicted in the window.** The core sends a *Playhead
  anchor* only on a discontinuity: a track change, a pause or resume, a runtime
  change, or a new position sample past a fixed tolerance. The window stamps the
  anchor on receipt and advances it. Facts with a minute or percent grain stay
  Rust projections.
- **Artwork is a URL.** Sections carry an art reference. The core serves the
  bytes through the `mbv-art` URI scheme from its image cache. The window makes
  no network request, and no event carries image bytes.

## Context

The first GUI slice copied the TUI's repaint loop: tick on input, wake, or the
render cadence, and emit one full frame that the window replaces whole. The
cadence existed mostly to move the playhead, because the Owner applies progress
without an event that a drain sees. Each frame rebuilt and re-serialized every
projection, the Library content and Settings included, and carried artwork as
base64 `data:` URLs. Four "carry once" memos (the theme, the card artwork, the
trickplay sheet, the Library content revision) existed only to cut the size of
that snapshot. Issue #997 and the efficiency review of the GUI Settings screen
listed the per-frame costs.

## Considered options

- **Keep the full frame and memoize more projections (rejected).** It keeps
  the timer-driven emit and the per-frame serialize, and each new section adds
  another memo.
- **One event for each section (rejected).** It gives per-section stores, but
  a track change then arrives as three events, and the window can paint the new
  active slot in one section and the old one in another, unless a commit marker
  is added.
- **Source revisions inside `App` (rejected).** Every mutation site of every
  Service store would bump a counter. The TUI shares those sites, and a missed
  bump leaves the GUI stale with no error. Core-side marks keep the TUI
  untouched, and the audit finds a missed mark in tests.
- **A keyed `data:` URL section with an LRU (rejected).** It keeps base64 and
  large JSON events, and the core must mirror what the window evicted.
- **Change-driven batches, a predicted playhead, and art by URL (chosen).** It
  follows how a web UI works: one store for each kind of data, a local clock
  for time-based values, images by URL with the browser's cache.

## Consequences

- An idle window gets no events. A playing window gets an event only when a
  section changes, not on every render interval.
- A missed dirty mark shows as a stale section until the next input.
  Invariant 32 owns the rule, and the audit runs in every core-thread test.
- The window formats the elapsed and remaining clocks itself, with a port of
  the shared duration formatter. The finish clocks stay in Rust.
- The `mbv-art` scheme is a new input into the core thread. It answers only
  from the image cache, and the CSP admits only that scheme beside `'self'`.
- `openspec/specs/gui-client/spec.md` owns the window's update contract.
  `docs/architecture/gui-client.md` describes the sections, the marks, and the
  scheme.
