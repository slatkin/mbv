# Proposal: extract-client-core

## Why

The TUI ties mbv's keyboard-first interaction model to the character-cell grid. The model does not need the grid. A later change adds a GUI Client in a native window. To keep two Clients from drifting apart, behaviour must live once in a toolkit-neutral Client core that both Clients share. This change extracts that core from the TUI and keeps every TUI behaviour unchanged. It adds no GUI program.

## What Changes

- Add a front-end-neutral Client core:
  - Surface state: cursor, selection, marks, viewport counted in items, and local focus for each interactive surface, with no ratatui or TuiRealm dependency.
  - A core-owned focus, mount, and overlay stack. This is the one focus authority.
  - A neutral key and pointer event type. The one keyboard router resolves chords on that type.
  - Capacity and semantic hit maps that each Client reports from its own painting. The core resolves pointer targets and movement strides from them.
  - The pointer gesture policy (double-click window, wheel step, drag anchor, toggle or range selection).
  - Shared breakpoint classes (`Mini`, `Narrow`, `Wide`) with the core Wide-hero rule.
  - A Client-neutral tick (`ClientCore::tick`) with an installable waker (`ClientWaker`).
- TuiRealm becomes the TUI Client's adapter. It keeps terminal event delivery, subscriptions, and the render entry point, and it follows the core focus stack. A new ADR supersedes the focus-authority part of ADR 0022.
- The root `mbv` package gets a library target. It exposes the shell `App`, the core tick, the router entry, a neutral GUI routing entry, and the Owner find, start, and attach steps, so that a later GUI Client program can link them.
- Not in scope: any GUI program, GUI toolkit, or GUI painter. A separate change adds the GUI Client.

## Capabilities

### New Capabilities

- `client-interaction-core`: the front-end-neutral behaviour that every Client shares: Surface state, the core focus and overlay stack, neutral input events, one router, the capacity and hit-map contract between the core and each Client's painting, and shared breakpoint classes.

### Modified Capabilities

- `interactive-component-framework`: TuiRealm stops being the focus and mount authority. Interactive Components over shared surfaces become TUI adapters over Surface state. The core focus stack becomes the one focus authority.
- `local-daemon-thin-client`: "Clients present the full terminal UI" names the TUI Client, because a Client is now a TUI Client or a GUI Client.

## Impact

- New crate: `crates/mbv-surfaces` (Surface state, focus stack, hit map, gesture policy, breakpoint classes; no ratatui, TuiRealm, or crossterm).
- The root `mbv` package gets a library target. The `mbv` binary is a thin `main` over it.
- `crates/mbv-components`: the Queue, Queue playback, and Status bar components become thin TuiRealm adapters over Surface state. The framework-free list modules move to `mbv-surfaces`.
- `src/app/input/`: the router and key policy move from `crossterm::event` to the neutral key type. crossterm keys are translated at the TUI edge.
- `src/app/shell/`: the run loop splits into a Client-neutral tick and a TUI driver. The TuiRealm `active`/`mount`/`umount` call sites follow the core focus stack.
- `crates/mbv-theme`: roles resolve to a neutral RGB value. ratatui colour conversion moves behind a feature at the TUI edge.
- Docs: a new ADR, `CONTEXT.md` terms (GUI Client, TUI Client, Client core, Surface state), `docs/architecture/tui-frontend.md`, and the interactive surface ledger.
