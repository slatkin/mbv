---
status: proposed
---

# The GUI Client Renders With Dioxus Native on Blitz

The GUI Client, `mbv-gui`, renders its window with Dioxus Native on Blitz, in
the same process as its core thread. It has no webview. This decision amends
the transport of [ADR 0036](0036-gui-library-panel-owns-its-interaction-state.md),
[ADR 0037](0037-gui-settings-screen-owns-its-interaction-state.md), and
[ADR 0038](0038-gui-window-receives-change-driven-sections.md). Their rules for
which side owns which state stand. Only the wire between the window and the
core changes.

## Decision

- **Dioxus Native on Blitz.** `mbv-gui` uses `dioxus-native` 0.8.0-alpha.3 on
  Blitz 0.3.0-beta.4 (`blitz-dom`, `blitz-shell`, `blitz-traits`) and the
  `vello-hybrid` renderer, which draws through wgpu (Vulkan or GL). The window
  has no webview, no WebKitGTK, and no JavaScript runtime. A normal build runs
  no Node tool.
- **The window is Rust.** The window content is a Dioxus component tree under
  `crates/mbv-gui/src/window/`. It takes no Svelte component and no TypeScript
  module. The window holds the interaction state that ADRs 0036 and 0037 give
  it, and it derives the playhead and the clocks that ADR 0038 gives it.
- **Values cross an in-process channel.** The core thread sends section batches
  and window events on an unbounded channel. The window sends its inputs to the
  core through a typed link to the core's input channel. No serde wire,
  no Tauri command, and no `mbv://sections` event remain.
- **Artwork and fonts load in process.** A net provider serves the `mbv-art`
  scheme from the core's image cache and serves the bundled fonts. Artwork
  bytes do not pass through an event.
- **The stylesheet is compiled ahead of the build.** Tailwind CSS v4 output is
  committed as `crates/mbv-gui/assets/app.generated.css`, and the build includes
  it. The standalone Tailwind CLI runs only when the developer regenerates it.
- **The window keeps a minimum size.** The window attributes set a 400×200
  logical-pixel floor, as the Tauri window did (`minWidth` 400, `minHeight`
  200), so a floating window cannot shrink below the size at which the compact
  strip lays out.
- **The ellipsis is drawn by mbv.** Blitz does not paint `text-overflow`. The
  window measures text with the same bundled faces and cuts it with `…`.
- **Exact pins.** The Blitz stack is pinned with exact version requirements, so
  an upstream release does not change the build.

## Context

The first GUI slice used Tauri 2 with WebKitGTK and a Svelte front end. Its
window content held real interaction state (ADRs 0036 and 0037), and that
state sat on the far side of a serde wire from the core thread. Each section
crossed that wire as a serialized event. The window also depended on the webview's
layout, its `ResizeObserver`, its reload, and a Node build.

The Blitz stack was tested in a spike (branch `feat/blitz-queue-panel-spike`,
commit `2a0510d2a`). It ran a fixture window with artwork, fonts, and a section
batch from another thread, and the window repainted after one poll.

## Considered options

- **Keep Tauri, WebKitGTK, and Svelte (rejected).** The state stays split
  across a wire, and the build keeps a webview and a Node toolchain.
- **GPUI (rejected).** The GPUI change record was marked rejected in commit
  `402aa1163`.
- **Dioxus Native on Blitz (chosen).** The Rust window and the core share one
  set of types. The 0.8 line is pre-release. The stable line (`dioxus-native`
  0.7.10 on `blitz-dom` 0.2.4) was not used: its `get_client_rect` returns
  `NotSupported`, and the layout reports need it.

## Consequences

- The pins move only as a deliberate change. An upgrade of the pre-release
  stack needs a fresh check of the event kinds and the net provider.
- `dioxus-native-dom` panics on some DOM event kinds (drag, clipboard, media,
  and others). The window installs no handler for them.
- Blitz has no error callback on its net handler. An artwork miss reaches the
  window as a failure event for the URL, and the window retries with a capped
  backoff.
- The window has no `ResizeObserver`. It reads client rectangles after a
  layout epoch changes.
- A window content reload no longer exists. A code change takes a restart of
  the program.
- Without a GPU, the surface cannot start. The packages depend on the Vulkan
  loader.
- The crate split changes what the GUI links: `mbv-gui` depends on
  `mbv-client-core`, not on the TUI crate, so the core thread and the window
  build without `ratatui`, `crossterm`, or TuiRealm
  ([ADR 0043](0043-clients-are-separate-crates-over-one-client-core.md)).
- The core thread's behaviour and the TUI Client do not change. The
  `openspec/specs/gui-client/spec.md` spec owns the window's contract.
