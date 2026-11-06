# Tasks

Implementation starts after `extract-client-core` (PR #984) merges.

## 1. Scaffold

- [x] 1.1 Pin every dependency to the exact version in design D2 (Cargo.toml and package.json, with lockfiles committed). Verify that `pnpm install` reports no peer-dependency warning.
- [x] 1.2 Add `crates/mbv-gui` (Tauri 2 binary `mbv-gui`) as a workspace member outside `default-members`, with `tauri-build`, a minimal `tauri.conf.json` (one window, bundled `frontendDist`, CSP and capability file per D11), and a `main` that opens an empty window. Verify with `cargo clippy -p mbv-gui --all-targets -- -D warnings` and `cargo tree -p mbv` showing no `tauri`, `wry`, or `webkit2gtk`.
- [x] 1.3 Add `crates/mbv-gui/ui/` with pnpm, Vite, Svelte 5, TypeScript, Tailwind CSS 4, shadcn-svelte (init, no components yet), `@lucide/svelte`, and vitest with `@testing-library/svelte`. Add `make` targets for install, build, check (svelte-check), lint, and test. Verify with those targets.

## 2. Core thread and bridge

- [x] 2.1 Add the bridge DTOs (`GuiFrame`, `DomKey`, `DomPointer`, `LayoutReport`) per D4, built from the library's slice projections. Verify with a test that builds a `GuiFrame` from a stubbed `App` with a playing queue and one from an empty queue.
- [x] 2.2 Add the DOM-key-to-`KeyEvent` translator per D6. Verify with one `#[case]` table test.
- [x] 2.3 Add the core thread per D3: it creates the `App`, attaches to the Owner per D9, installs a `ClientWaker` that feeds its channel, runs `ClientCore::tick` on input, wake, or `CLIENT_IDLE_CADENCE`, implements `ClientTickHost` over the slice's Surface state, and emits a `GuiFrame` after a tick that changed state and after every `report_layout` (D3). Verify with a step test on a mocked channel: one input yields one frame, and a waker wake yields a tick.
- [x] 2.4 Apply `LayoutReport` to the Surface state's capacity and `HitMap`, and route `key` through `App::route_gui_input` with the focused `GuiSurfaceLeaf`. Route `pointer` per D4: resolve the point against the combined slice `HitMap`, apply the TUI click's focus change, and pass the surface that owns the region as the leaf to `App::route_gui_pointer`. (The width classification that this task added is superseded by D10 and removed in task 6.3.) Verify with tests: a reported capacity of 14 sets the paging stride, a press resolves against the last reported regions, and a press on the play/pause region while the Queue panel has focus dispatches the playback request.
- [x] 2.5 Register the three Tauri commands and the `mbv://frame` event. The commands only push into the core channel. Verify with clippy and the existing tests.
- [x] 2.6 Add the attach outcomes per D9 on the core thread: the two refusal texts, the wait for an Owner that shuts down, the build mismatch with Restart (R) and Quit (Q), and the attach-failed text. Verify with one `#[case]` table: one row for each outcome and its window state.

## 3. Front end

- [x] 3.1 Add the frame store and the input layer: listen to `mbv://frame`, keep the last frame, send every keydown through `key` with the default action prevented (D6), and send pointer and wheel events through `pointer`. Verify with vitest: a key event calls `key` with the DOM fields, and a frame event updates the store.
- [x] 3.2 Add the theme layer per D7: role colours from the frame go into CSS custom properties, and the shadcn-svelte variables point at them. Verify with a vitest test that a frame's theme sets the properties.
- [x] 3.3 (Superseded by 6.5: built to the TUI-parity target.) Add the Queue panel per D5: fixed-height rows for the frame's viewport, the cursor, marks, and the active slot with a Lucide marker, the empty-queue state, and the footer pills. Report capacity and row regions through `report_layout`. Verify with vitest: a frame with slots renders its rows, an empty frame renders the empty state, and a layout change sends one report.
- [x] 3.4 (Superseded by 6.4 and 6.6: built to the TUI-parity target.) Add the Queue playback panel (active item, progress, transport and seek controls as Lucide icons, playback-target indicator) and the Status bar (prefix-armed state, toasts), each reporting its regions. Verify with vitest: each renders a given frame, and its controls appear in the reported regions.
- [x] 3.5 Add the attach states per D9: a waiting state before the first frame and while an Owner shuts down, the two refusal texts, the build mismatch with Restart and Quit buttons that report their regions, and the attach-failed text. Verify with vitest: each state renders, and the two buttons appear in the reported regions.

## 4. Docs

- [x] 4.1 Update `AGENTS.md`: the repository map line for `crates/mbv-gui`, and `mbv-gui` in the tokio edge list (D12). Update the `CONTEXT.md` GUI Client definition if it names a toolkit.
- [x] 4.2 Add `docs/architecture/gui-client.md`: the core thread, the bridge contract, layout reports and the hit map, theme mapping, how to add a surface to the GUI, and the `WEBKIT_DISABLE_DMABUF_RENDERER` note. Link it from `docs/architecture/tui-frontend.md`.
- [x] 4.3 Update `docs/recipes.md` where a recipe (keybind action, surface projection) now has a GUI site.

## 5. Integration checks

- [x] 5.1 Build `ui/`, then run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, `cargo nextest run -p mbv-gui`, and the front-end `make` check, lint, and test targets.
- [x] 5.2 Update CI per D14: add `webkit2gtk-4.1` and pnpm to the pacman installs of the `build` and feature-matrix jobs, and build `ui/` before their clippy and `cargo hack` steps. Add one GUI job that runs `cargo nextest run -p mbv-gui` and the front-end check, lint, and test targets.
- [x] 5.3 Add a `make dev` target that builds `ui/` and runs the GUI Client (`cargo run -p mbv-gui`), so a developer starts the window with one command. Document it in `docs/architecture/gui-client.md`.

## 6. Retarget to the design canvas

The target is the design canvas (design D15). Groups 1 to 5 were built to TUI parity. Each task below replaces or extends that work, and the canvas wins where they differ. Task 6.1 comes first, because the other tasks need its answers.

- [ ] 6.1 Resolve the Open Questions of the design that gate data: plan the source of the cast choices, chapters and intro/credits marks, trickplay, the track lists, and finish clocks from the Emby API docs, open-source clients, and the Owner's queue and playback state (no live probe). Record the source of each datum in the design. Report each datum that the Owner cannot supply, for the user to rule on. Verify: the design names a source or a ruled gap for every datum of D4.
- [ ] 6.2 Add the missing fields to the library projections (the source-of-truth types), then to the bridge DTOs and `GuiFrame` per D4: the queue summary, the right-column values, the playback fields, the track chips, the open menu or popover with its key hints, and the collapse and fold toggles. Add the core-side state and routing for the toggles (`z`, `q`), the menu and popover, and the filter. Verify with a `GuiFrame` test for a queue with a partial-progress row, a watched row, and an open popover, and one for the empty queue.
- [ ] 6.3 Replace the width breakpoint with the panel states per D10: remove the width classification and its constant from the core thread, report the container size in `LayoutReport`, and apply it to capacity and the hit map. Verify with a test that a reported container size changes the capacity and the regions.
- [ ] 6.4 Build the player panel of the expanded artboard in the front end: header with the playback target and chooser, artwork, title, scrubber (chapters, marks, thumb, hover preview, press to seek), transport, track chips and the subtitle popover, and the footer line (prefix-armed state, toasts). Verify with vitest: each part renders from a frame, and a press on the scrubber, a chip, and a popover item calls `pointer` with the right payload.
- [ ] 6.5 Build the queue of the canvas: header (count, finish clock, Length / Ends at switch, filter), 30 px rows with the watched, active, and partial-progress states, the row hover (grip and actions button), the row menu with key hints, and the drag ghost and insertion line. Add the core drag state and the move request to the Owner process, and the `Alt+J` / `Alt+K` actions. Verify with Rust tests (drop index from a pointer move, one move request on release, the menu items) and with vitest (a frame renders the rows and the open menu).
- [ ] 6.6 Build the collapsed states: compact strip, expanded without queue, and rail, switched by container queries, and the `z` and `q` toggles. Verify with vitest: each state renders from a frame and a container size.
- [ ] 6.7 Replace the SUSE font with the two bundled faces per D8, add the artwork and trickplay delivery with the CSP change of D11, and restyle the empty-queue and attach states (D9) in the canvas's type, colour, and spacing. Verify with vitest that the theme and fonts apply, and with `pnpm` build output that `ui/dist` holds both faces and no remote font URL.
- [ ] 6.8 Update `docs/architecture/gui-client.md` for the new frame fields, the panel states, drag, and fonts, and `docs/recipes.md` where a recipe gained a site. Update `AGENTS.md` and `CONTEXT.md` only if they describe the old slice. Then run the gates of task 5.1 again.

## Workflow follow-up

- Sync and archive this change only when the user approves the PR for merge, as the last commit before the merge.
