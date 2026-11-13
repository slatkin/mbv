# Proposal

## Why

The GUI Client runs in a Tauri 2 window: a WebKitGTK webview that runs a Svelte front end. The webview is most of the window's memory, and it brings a second toolchain (Node, pnpm, Vite, Svelte, Tailwind) into the build. The Blitz spike (`blitz-queue-panel-spike`, never merged) painted the Queue panel from the GUI core in process with Dioxus Native on Blitz, and the user accepted it. This change replaces the whole front end on that stack, with the same look and the same behaviour.

## What Changes

- The GUI Client's window becomes a Dioxus Native window (`dioxus-native` 0.8.0-alpha.3, Blitz 0.3.0-beta.4) that runs in the `mbv-gui` process. No webview, no JavaScript, and no web engine process.
- Every Svelte component and TypeScript module of `crates/mbv-gui/ui/` is ported to Rust in `crates/mbv-gui/src/window/`. The port is accurate: each component keeps its markup structure, its Tailwind class strings, its state rules, and its keys. The Library panel, the Settings screen, the player panel, the Queue panel, the Status bar, the overlays, and the attach screens all move.
- The front end and the core thread share one process with no wire. Section batches reach the window as Rust values through an in-process channel. Keys, pointer events, layout reports, and Library and Settings commands go into the core channel as `CoreInput` values. The serde wire and the Tauri commands go.
- Artwork, fonts, and icons load in process. A custom Blitz net provider answers `mbv-art` requests from the core's image cache and answers the bundled fonts. No HTTP request and no `data:` URL is used.
- The CSS comes from the same Tailwind classes. The Tailwind v4 standalone CLI (4.3.5, one binary, no Node) builds the stylesheet from the Rust sources, and the output is committed. A normal `cargo build` needs no CSS tool.
- mbv adds the ellipsis that Blitz does not paint (`text-overflow`, Blitz gap G1): a `truncate` text is measured with parley and cut before it enters the DOM.
- The dragged panel widths move from the webview's `localStorage` to a GUI state file beside the GUI launch state. **BREAKING** (local only): the widths saved by the Tauri build are not read, so the dividers open once at their defaults.
- A window-content reload no longer exists. The window asks for every section when it opens, as before.
- At the end of the change, Tauri, WebKitGTK, the Svelte front end, `tauri.conf.json`, the Node build, and the pnpm and webkit2gtk steps in CI and packaging are deleted. **BREAKING** (packaging): the packages no longer depend on WebKitGTK 4.1. They depend on the Vulkan loader and fontconfig.
- Until that last step, the Tauri program still builds behind a default Cargo feature, so the GUI stays usable while the port runs.
- The `blitz-queue-panel-spike` change leaves the repository: its directory under `openspec/changes/` is deleted, and its branch and PR are not merged.
- A new ADR records the move to Dioxus Native on Blitz. It keeps the decisions of ADRs 0036, 0037, and 0038 and replaces their webview transport.

## Capabilities

### New Capabilities

None. The GUI Client's behaviour stays in `gui-client` and `gui-settings-screen`.

### Modified Capabilities

- `gui-client`: the program carries no webview stack and runs as one process; the packages declare the Vulkan loader and fontconfig instead of WebKitGTK 4.1; the canvas colours come from the window's stylesheet, not `app.css`; the window-content reload scenarios go, because no reload exists. Three requirements that carry a reload scenario move to new names with the same rule and no reload.
- `gui-settings-screen`: the window permissions sentence goes (no Tauri capability exists, every Settings request is an in-process input); the reload rule and scenario go, and the interaction-state requirement moves to a new name without them.

## Impact

- **Code:** `crates/mbv-gui` gains the `window/` module tree, a Blitz launch copy, the net provider, and a new `main.rs`. `crates/mbv-gui/ui/`, `tauri.conf.json`, `build.rs`, `capabilities/`, and the Tauri-only tests go. The bridge's serde derives go where nothing reads them. `mbv-config` gains the panel-widths file.
- **Dependencies:** `dioxus-native` 0.8.0-alpha.3, `blitz-shell` 0.3.0-beta.4, `blitz-traits` 0.3.0-beta.4, `parley` 0.12.0, `futures-channel` and `futures-util` 0.3.34 (design D2). Tauri 2.12.5, `tauri-build`, and the Node packages go.
- **Tokio:** `dioxus-native` brings tokio 1.54.1 with no switch to turn it off (design D2). `mbv-gui` already is a tokio edge crate (Tauri's runtime); the edge stays, and no mbv code uses a tokio API.
- **Licences:** Stylo and other Servo crates are MPL-2.0. The packages ship the MPL notice and the source offer (design D17).
- **Docs:** `docs/architecture/gui-client.md`, `AGENTS.md`, `CONTEXT.md`, `docs/recipes.md`, `docs/architecture/performance.md`, and the invariants that name the webview.
- **Not in this change:** memory measurements, a CPU renderer, an X11-only or Wayland-only build, and upstream Blitz reports.
