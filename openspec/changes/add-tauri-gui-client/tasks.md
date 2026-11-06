# Tasks

Implementation starts after `extract-client-core` (PR #984) merges.

## 1. Scaffold

- [ ] 1.1 Pin every dependency to the exact version in design D2 (Cargo.toml and package.json, with lockfiles committed). Verify that `pnpm install` reports no peer-dependency warning.
- [ ] 1.2 Add `crates/mbv-gui` (Tauri 2 binary `mbv-gui`) as a workspace member outside `default-members`, with `tauri-build`, a minimal `tauri.conf.json` (one window, bundled `frontendDist`, CSP and capability file per D11), and a `main` that opens an empty window. Verify with `cargo clippy -p mbv-gui --all-targets -- -D warnings` and `cargo tree -p mbv` showing no `tauri`, `wry`, or `webkit2gtk`.
- [ ] 1.3 Add `crates/mbv-gui/ui/` with pnpm, Vite, Svelte 5, TypeScript, Tailwind CSS 4, shadcn-svelte (init, no components yet), `@lucide/svelte`, and vitest with `@testing-library/svelte`. Add `make` targets for install, build, check (svelte-check), lint, and test. Verify with those targets.

## 2. Core thread and bridge

- [ ] 2.1 Add the bridge DTOs (`GuiFrame`, `DomKey`, `DomPointer`, `LayoutReport`) per D4, built from the library's slice projections. Verify with a test that builds a `GuiFrame` from a stubbed `App` with a playing queue and one from an empty queue.
- [ ] 2.2 Add the DOM-key-to-`KeyEvent` translator per D6. Verify with one `#[case]` table test.
- [ ] 2.3 Add the core thread per D3: it creates the `App`, attaches to the Owner per D9, installs a `ClientWaker` that feeds its channel, runs `ClientCore::tick` on input, wake, or `CLIENT_IDLE_CADENCE`, implements `ClientTickHost` over the slice's Surface state, and emits a `GuiFrame` after a tick that changed state. Verify with a step test on a mocked channel: one input yields one frame, and a waker wake yields a tick.
- [ ] 2.4 Apply `LayoutReport` to the Surface state's capacity and `HitMap`, and route `key` and `pointer` through `App::route_gui_input` and `App::route_gui_pointer` with the focused `GuiSurfaceLeaf`. Classify the reported width with the shared breakpoint classes and a GUI threshold constant (D10). Verify with tests: a reported capacity of 14 sets the paging stride, and a press resolves against the last reported regions.
- [ ] 2.5 Register the three Tauri commands and the `mbv://frame` event. The commands only push into the core channel. Verify with clippy and the existing tests.

## 3. Front end

- [ ] 3.1 Add the frame store and the input layer: listen to `mbv://frame`, keep the last frame, send every keydown through `key` with the default action prevented (D6), and send pointer and wheel events through `pointer`. Verify with vitest: a key event calls `key` with the DOM fields, and a frame event updates the store.
- [ ] 3.2 Add the theme layer per D7: role colours from the frame go into CSS custom properties, and the shadcn-svelte variables point at them. Verify with a vitest test that a frame's theme sets the properties.
- [ ] 3.3 Add the Queue panel per D5: fixed-height rows for the frame's viewport, the cursor, marks, and the active slot with a Lucide marker, the empty-queue state, and the footer pills. Report capacity and row regions through `report_layout`. Verify with vitest: a frame with slots renders its rows, an empty frame renders the empty state, and a layout change sends one report.
- [ ] 3.4 Add the Queue playback panel (active item, progress, transport and seek controls as Lucide icons, playback-target indicator) and the Status bar (prefix-armed state, toasts), each reporting its regions. Verify with vitest: each renders a given frame, and its controls appear in the reported regions.
- [ ] 3.5 Add the attach states: the window shows the refusal text when the Owner refuses the Client, and a waiting state before the first frame. Verify with vitest for both states.

## 4. Docs

- [ ] 4.1 Update `AGENTS.md`: the repository map line for `crates/mbv-gui`, and `mbv-gui` in the tokio edge list (D12). Update the `CONTEXT.md` GUI Client definition if it names a toolkit.
- [ ] 4.2 Add `docs/architecture/gui-client.md`: the core thread, the bridge contract, layout reports and the hit map, theme mapping, how to add a surface to the GUI, and the `WEBKIT_DISABLE_DMABUF_RENDERER` note. Link it from `docs/architecture/tui-frontend.md`.
- [ ] 4.3 Update `docs/recipes.md` where a recipe (keybind action, surface projection) now has a GUI site.

## 5. Integration checks

- [ ] 5.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, `cargo nextest run -p mbv-gui`, and the front-end `make` check, lint, and test targets.
- [ ] 5.2 Add one CI job that installs the WebKitGTK build dependencies and pnpm, and runs the front-end check, lint, and test targets.

## Workflow follow-up

- Sync and archive this change only when the user approves the PR for merge, as the last commit before the merge.
