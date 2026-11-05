# Tasks

## 1. Neutral vocabulary

- [x] 1.1 Add neutral `KeyEvent` (code, modifiers) and `PointerEvent` (kind, position in `f32` logical units) to `mbv-ui-msg`. Replace `TuiKeyEvent` in `UserEvent::Key` and `LeafKeyResult`, and remove the `tuirealm` dependency from `crates/mbv-ui-msg/Cargo.toml`. Verify with `cargo check --workspace` and `cargo tree -p mbv-ui-msg | rg tuirealm` (no output).
- [x] 1.2 Replace `ratatui::layout::Rect` in `crates/mbv-ui-model/src/context_menu.rs` with a neutral rectangle type and remove the ratatui dependency from `mbv-ui-model`. Convert at the TUI painter edge. Verify with `cargo check --workspace` and the existing context-menu tests.
- [x] 1.3 Make `mbv-theme` roles and surface rows resolve to a neutral `Rgb`. Put the ratatui conversion behind a `ratatui` cargo feature that only the TUI crates enable. Keep `Slot`, the theme value, and `active()` crate-private. Verify with `cargo check --workspace` and `cargo nextest run -p mbv-theme`.

## 2. Router on neutral keys

- [x] 2.1 Translate crossterm key and mouse events to the neutral types at the TUI event-listener edge. Add one `#[case]` table test of the mapping (modifier lowercase rule included). Verify that the table test passes.
- [x] 2.2 Move `src/app/input/router.rs`, `key_policy.rs`, `resolver.rs`, and `confirm_keys.rs` from `crossterm::event` to the neutral key type. Verify that `cargo nextest run -p mbv` passes with the routing-matrix and tick-integration tests unchanged.

## 3. Core focus stack

- [x] 3.1 Create `crates/mbv-surfaces` (no ratatui, TuiRealm, or crossterm dependency) and add it to the workspace. Verify with `cargo tree -p mbv-surfaces | rg 'ratatui|tuirealm|crossterm'` (no output).
- [x] 3.2 Add `FocusStack` to `mbv-surfaces`: mounted set, Panel focus, overlay stack keyed by `ComponentId`, with deterministic restore on dismiss. Add public-API tests in `crates/mbv-surfaces/tests/surfaces.rs` for push, dismiss-restore independent of mount order, and Panel focus moves. Verify that they pass.
- [x] 3.3 Fold `App`'s `PanelFocus` into the shell-held `FocusStack`. Add the one sync step in `sync_mounted_surfaces` that applies the stack to TuiRealm. Verify with the existing panel-focus tick-integration tests.
- [x] 3.4 Route the modal mount, dismiss, and focus call sites in `src/app/shell/overlays/modals.rs` through `FocusStack`. Verify with the existing modal tick-integration tests.
- [x] 3.5 Route the sidebar and menu call sites (`shell/overlays/sidebars.rs`, `shell/overlays/menus.rs`) through `FocusStack`. Verify with the existing sidebar and context-menu tick-integration tests.
- [x] 3.6 Route the remaining TuiRealm `active`/`mount`/`umount` call sites in `src/app/shell/` (root, library, queue, feeds_manage, arbitration, twitch_url, and others) through `FocusStack`, so that the sync step is the only TuiRealm focus caller. Verify with `rg -n '\.active\(|\.blur\(' src/app/shell -g '!**/tests/**'`: it reports only the sync step. Verify also that the full `cargo nextest run -p mbv` passes.
- [x] 3.7 Add one tick-integration test, "TuiRealm focus follows the core stack": a stack change is the focused TuiRealm component after the sync pass. Verify that it passes.
- [x] 3.8 Write the ADR for the decision "the Client core owns focus; TuiRealm is the TUI adapter" (read `docs/adr/README.md` first). Mark the focus part of ADR 0022 as superseded. Verify that the ADR follows the README amendment rules.
- [x] 3.9 Add the `CONTEXT.md` terms TUI Client, GUI Client, Client core, and Surface state, and change the `Client` definition to cover both kinds (design D1). Verify that no new term uses an *Avoid* word.

## 4. Surface state for the Queue slice

- [x] 4.1 Move `crates/mbv-components/src/list/{cursor,viewport,row_flow,marks}.rs` and their tests into `mbv-surfaces`. Re-export them from `mbv-components` only where needed. Verify that `cargo nextest run -p mbv-surfaces -p mbv-components` passes with the moved tests and no duplicates.
- [x] 4.2 Add `HitMap<Target>` (regions in `f32` logical units, point-in-region resolution, no geometry computation) and a per-list capacity report to `mbv-surfaces`. Add public-API tests for hit, miss, and resolve-against-last-frame. Verify that they pass.
- [x] 4.3 Extract the Queue cursor, marks, selection, viewport, and key rules from `crates/mbv-components/src/queue.rs` and `queue/keys.rs` into a Queue Surface state in `mbv-surfaces`. Move the matching tests. Verify with `cargo nextest run -p mbv-surfaces -p mbv-components`.
- [x] 4.4 Make `QueueComponent` a TuiRealm adapter over the Queue Surface state: it forwards neutral events, paints from the state, and reports capacity and a `HitMap` from `queue/pointer.rs` geometry. It holds no copy of state values. Verify with the queue tick-integration and mouse tests (wheel, row click, obscured-queue rejection).
- [ ] 4.5 Extract the state that the Queue playback panel owns (transport focus, seek target) into Surface state and make `QueuePlaybackPanel` an adapter. Verify with `queue_playback_panel` tests and the playback tick-integration tests.
- [ ] 4.6 Extract the state that the Status bar owns (prefix-armed display, toast presentation) into Surface state and make `status_bar_panel.rs` an adapter. Verify with the existing status bar and prefix-mode tests.
- [ ] 4.7 Update `docs/architecture/interactive-surface-ledger.md` and `docs/architecture/tui-frontend.md` for the adapter and Surface state split (where state lives now, and the "behaviour lives once" rule). Verify that the ledger rows for Queue, Queue playback, and Status bar name their Surface state.

## 5. Client-neutral tick and library target

- [ ] 5.1 Move the `App`-side run-loop phases (runtime and channel drains, Owner snapshot adoption, effects, toast expiry) from `src/app/shell/run.rs` into `ClientCore::tick(now) -> TickOutcome`, with an installable waker callback. `Model::run` calls it before the TuiRealm tick. Verify that the full `cargo nextest run -p mbv` passes.
- [ ] 5.2 Add a library target to the root `mbv` package (`src/lib.rs`) that exposes `ClientCore`, the Owner find, start, and attach entry with an override for the `mbv` executable path, and the router entry. The `mbv` binary uses the library. Verify with `cargo build -p mbv` and `cargo tree -p mbv -e normal | rg 'eframe|egui|wgpu|glow'` (no output).
- [ ] 5.3 Update the `AGENTS.md` repository map for `mbv-surfaces`, `mbv-gui`, and the root library target. Verify that each new crate has one line.

## 6. GUI Client program

- [ ] 6.1 Create `crates/mbv-gui` (binary, eframe with the glow renderer). Open one window and construct `ClientCore`, installing a waker that calls `request_repaint`. Call `ClientCore::tick` at the start of each `update` and `request_repaint_after` at the TUI idle cadence. Verify that `cargo build -p mbv-gui` succeeds.
- [ ] 6.2 Find, start, and attach to the Owner process through the library entry. Resolve `mbv` from the GUI executable's directory, then `PATH`. Show the same refusal text as the TUI when an exclusive Owner refuses. Verify with a unit test of the executable-resolution order.
- [ ] 6.3 Translate egui `RawInput` key and pointer events to the neutral types and route keys through the shared router. Add one `#[case]` table test of the key mapping. Verify that it passes.
- [ ] 6.4 Paint the Queue panel from the Queue Surface state with the egui `Painter`: fixed-row list, active-slot mark, cursor, marks, empty-queue state, theme roles through `Rgb`. Report capacity and a `HitMap` each frame. Use no stateful egui widget for interaction state. Verify with a headless egui test (`Context::run`, synthetic `RawInput`): a page-down moves the cursor by the reported capacity.
- [ ] 6.5 Paint the Queue playback panel (active item, progress, transport and seek controls) and the Status bar (prefix-armed state, toasts) from their Surface state, with hit maps for the controls. Verify with a headless test: a click on the play/pause region sends the transport request.
- [ ] 6.6 Map window width to the shared breakpoint classes with a GUI threshold constant, and apply the core's class behaviour to the Queue playback arrangement. Verify with a headless test at two widths that asserts the class, not the geometry.
- [ ] 6.7 Enforce the interaction contract: hover never moves focus or cursor, scroll follows the cursor after every key move, and no animation delays a key. Verify with headless tests: hover with no press leaves the cursor unchanged, and a move past the last visible row keeps the cursor row fully visible.
- [ ] 6.8 Add a GUI section to `docs/architecture/tui-frontend.md` (or a sibling `gui-client.md` that it links): where GUI painters live, the interaction contract, and the rule that GUI code holds no interaction rule. Verify that the doc names each contract rule from `specs/gui-client`.

## 7. Integration checks

- [ ] 7.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`, and verify that all pass.
- [ ] 7.2 Measure the CI workspace build time with `mbv-gui`, and decide whether `mbv-gui` goes in `default-members` (design Open Questions). Verify that the decision is recorded in `design.md`.
- [ ] 7.3 Run `make check-code-file-lines` before the push, and verify that no file exceeds 800 lines.

## Workflow follow-up

- The user runs the live check: GUI and TUI attached to one Owner, keys and clicks on the Queue slice, and video in the mpv window.
- When the user approves the PR for merge, sync the deltas into `openspec/specs/` and archive the change as the last commit before the merge.
