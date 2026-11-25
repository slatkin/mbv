# mbv-gui Agent Guide

`crates/mbv-gui/` is the GUI Client: the `mbv` program, a Dioxus Native
window rendered by Blitz in the same process as its core thread (ADR 0039).
It links `mbv-client-core` for the shared state and rules, and it never links
`mbv-tui`. Read the root `AGENTS.md` first for the workspace, the shared core,
the coding standards, and the test rules. This guide holds what applies to the
GUI edge; `docs/architecture/gui-client.md` is the full behaviour guide.

## Crate map

- `src/main.rs` — installs the app log, starts the core thread, launches the
  window, and joins the core thread after the event loop ends.
- `src/core_thread.rs` — the one dedicated, synchronous thread. It loads the
  configuration, runs the Owner attach campaign (`src/attach.rs`), and runs
  `ClientCore::tick` in a loop over a `GuiCore { app, gui }`.
- `src/gui_core.rs` + `src/gui_core/` — `GuiCore` and every GUI-only module:
  the Library panel, the Settings screen, the Playlists screen,
  `GuiClientState`, and the GUI routing entry (`gui_core::input`).
- `src/bridge.rs` + `src/bridge/` — the DTOs and the DOM-input translation.
  A bridge value is the batch that crosses the in-process channel, never a
  serialized wire.
- `src/window.rs` + `src/window/` — the Dioxus component tree: the root, the
  panel components, the leaves, the layout registry, the clocks, and the
  net provider.
- `src/art.rs` — the `mbv-art` URL builder; the core answers each load from
  its image cache.
- `src/sections.rs` + `src/sections/` — the section ledger, the mark rows, and
  the mark sources.
- `src/leaves.rs` — the `GuiSurfaceLeaf` adapters that resolve reported
  regions into the semantic `Msg`s the TUI's pointer path emits.

The GUI Client is a workspace member outside `default-members`: the default
`cargo build` and nextest runs skip it, while `--workspace` clippy and fmt
cover it. The package is `mbv-gui`; its program is `mbv`. `make dev` builds
`mbv`, `mbv-tui`, and `mbv-owner` together before it starts the window,
because the GUI starts its Owner from the sibling `mbv-owner` program and the
handshake refuses an Owner with a different build id.

## Process rules

- One process, two threads. The window's event loop runs on the main thread.
  The core thread is synchronous and owns the `App`; no mbv code in `mbv-gui`
  is `async`. The `dioxus-native` asset resolver is the only tokio edge.
- The window's event handlers never touch the `App`. They translate the input
  and push it into the core channel through `CoreLink`. A handler that needs a
  core answer sends the request and reads the next batch.
- The GUI Library panel and the Settings screen own their interaction state in
  the window (ADRs 0036 and 0037); the core supplies their data and their
  actions. The Queue panel and the Status bar share Surface state with the TUI:
  never reimplement an interaction rule here, and never hold a second copy of
  a value the Surface state owns.
- A surface the GUI does not present stays absent: no stub, no placeholder.
  The design canvas is the reference, not the TUI.
- Artwork loads in process. The `mbv-art` scheme answers from the core's image
  cache, and the bundled fonts load through the window's net provider. Artwork
  bytes never cross the channel.

## Keyboard and pointer

- The window's own key router runs first (`src/window/`): a key the window
  consumes never reaches the core, and a key it does not consume goes to the
  core's Keyboard Router (`mbv_client_core::input`, mounted through
  `resolve_router_outcome`). The GUI Library panel takes its own keys here,
  before the shared router (ADR 0036).
- Never add a keyboard routing site in the core for the GUI. A GUI-only chord
  gets its `KeyPolicyGate` routing gate and its router capability fact, and it
  still resolves in the core's three router files. `KeyGate` is the
  presentation bucket the keybind registry carries; the routing gate is the
  core's `KeyPolicyGate`.
- Every action a pointer can reach keeps a key chord (the `gui-client` spec).
  A control whose TUI target the GUI has not ported is a documented no-op,
  never a wildcard.
- A pointer target follows the pointer (ADR 0040). A leaf resolves gestures
  against the regions the last frame reported; it never invents coordinates.

## Sections and layout

- The core sends at most one `WindowEvent::Batch` per tick that changed state,
  through `SectionLedger::collect`. A section is present only when its value
  differs from the one the window holds; an omitted section leaves the
  window's value alone. A gated section builds only while a mark names it, so a
  new drain that changes a gated source needs its mark row or mark source.
- The window reports every mounted surface's regions through
  `LayoutReporter`; the layout registry is the single place that names them.
  A target id mirrors the shared hit-target enum variant for variant.
- The window has no `ResizeObserver` and no `requestAnimationFrame`. It reads
  client rectangles after a layout epoch changes and runs its timers through
  the `Clock` seam. No test sleeps.

## Style

- The stylesheet is `assets/app.css`; the committed Tailwind CSS v4 output is
  `assets/app.generated.css`. A class added to the Rust sources needs
  `make gui-css` before its commit. Never hand-edit the generated file.
- Icons are Lucide only, drawn as inline SVG. The ellipsis is drawn by mbv:
  Blitz does not paint `text-overflow`, so a box that can overflow uses
  `Truncate`. Never use `:has()`: one invalid selector drops its whole
  selector list.
- The crate's log goes to `mbv-gui.log` in the mbv state directory, not the
  TUI's `mbv.log`.

## Tests

- The Rust tests in `tests/` mock the channel, the batch sink, and the Owner
  steps. A test asserts a mapping the code owns: region to `Msg`, capacity
  applied, a batch built. It never opens a window.
- The window tests sit beside their modules in `src/window/**/tests.rs`. Each
  is stateless: it builds its input, calls once, and asserts on the return
  value or the painted output. A component test renders a fixture batch
  through `DioxusDocument` and asserts on the painted structure, not on
  appearance.
- No test opens a window, sleeps, or reaches an Owner or a network. Live
  behaviour in the real window is the user's manual check. Never add a browser
  test, a Playwright run, or a headless-Chromium probe.
- A mapping that is new gets one test at the layer that owns it; a painter
  that changed gets a headless painted-structure assertion, not a screenshot.
