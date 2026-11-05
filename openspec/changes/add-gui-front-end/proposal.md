# Proposal: add-gui-front-end

## Why

The TUI gives mbv a controlled, keyboard-first interaction model. But it ties that model to the character-cell grid: one font size, fixed cell rows, column breakpoints, and images that go through terminal graphics protocols. The interaction model does not need the grid. A native window can keep the model and drop the grid. To do this without two products that drift apart, behaviour must live once and be shared by a terminal Client and a windowed Client.

## What Changes

- Add a GUI Client: a separate `mbv-gui` binary that opens a native window (egui through eframe). It attaches to the Owner process over ctrl like every other Client. It never touches mpv. Video still plays in mpv's own window.
- The first GUI slice presents the Queue panel, the Queue playback panel, and the Status bar. Other surfaces are absent from the GUI until later changes port them. The TUI keeps every surface and keeps working unchanged.
- Add a front-end-neutral Client core that both Clients share:
  - Surface state: cursor, selection, marks, viewport counted in items, and local focus for each interactive surface, with no ratatui or TuiRealm dependency.
  - A core-owned focus, mount, and overlay stack. This is the one focus authority for both Clients.
  - A neutral key and pointer event type. The one keyboard router resolves chords on that type.
  - Capacity and semantic hit maps that each Client reports from its own painting. The core resolves pointer targets and movement strides from them.
- TuiRealm becomes the TUI Client's adapter. It keeps terminal event delivery, subscriptions, and the render entry point, and it follows the core focus stack. A new ADR supersedes the focus-authority part of ADR 0022.
- The GUI Client uses the same `[keys]` configuration, theme roles, and Owner launch and attach rules as the TUI.
- Not in scope: Library, search, overlays other than those the slice needs, artwork and the visual slot, queue drag-reorder and column resize, `--pin` for the GUI, desktop entry, release packaging of `mbv-gui`, and removal of the TUI.

## Capabilities

### New Capabilities

- `gui-client`: the windowed Client: launch and Owner attachment, the surfaces it presents, its interaction contract (one visible focus, every action on a key, no hover-only affordance, scroll follows the cursor, motion never delays input, fixed-row lists), and its pointer behaviour.
- `client-interaction-core`: the front-end-neutral behaviour that both Clients share: Surface state, the core focus and overlay stack, neutral input events, one router, and the capacity and hit-map contract between the core and each Client's painting.

### Modified Capabilities

- `interactive-component-framework`: TuiRealm stops being the focus and mount authority. Interactive Components become TUI adapters over shared Surface state. The core focus stack becomes the one focus authority.
- `local-daemon-thin-client`: "Clients present the full terminal UI" changes so that a GUI Client presents its own window and the surfaces of the `gui-client` capability, while a TUI Client keeps the full terminal UI.

## Impact

- New crates: `crates/mbv-surfaces` (Surface state and focus stack, no ratatui, TuiRealm, or crossterm) and `crates/mbv-gui` (binary, eframe/egui, wgpu or glow).
- The root `mbv` package gets a library target so `mbv-gui` can share the shell `App`, dispatch, router, and Owner launch and attach code. The `mbv` binary does not link eframe, egui, or wgpu.
- `crates/mbv-components`: the Queue, Queue playback, and Status bar components become thin TuiRealm adapters over Surface state. The framework-free list modules (`list/cursor.rs`, `list/viewport.rs`, `list/row_flow.rs`, `list/marks.rs`) move to `mbv-surfaces`.
- `src/app/input/`: the router and key policy move from `crossterm::event` to the neutral key type. crossterm keys are translated at the TUI edge.
- `src/app/shell/`: the run loop splits into a Client-neutral tick (runtime drains, Owner snapshot adoption, effects) and a TUI driver. About 51 TuiRealm `active`/`mount`/`umount` call sites start to follow the core focus stack.
- `crates/mbv-theme`: roles resolve to a neutral RGB value. ratatui colour conversion moves to the TUI edge.
- Docs: a new ADR, `CONTEXT.md` terms (GUI Client, TUI Client, Client core, Surface state), `docs/architecture/tui-frontend.md`, and the interactive surface ledger.
- CI build time grows by the eframe/wgpu dependency tree for the workspace build.
