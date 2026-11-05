# Design: add-gui-front-end

## Context

See `proposal.md` for the reason. The requirements are in `specs/gui-client`, `specs/client-interaction-core`, and the two deltas.

Current state that shapes the approach:

- The Owner process has no UI. Every Client talks to it over ctrl (`docs/architecture/owner-client-operation-split.md`). A GUI Client needs no Owner change.
- The root `mbv` package is binary-only. The shell `App`, dispatch, the router, and Owner launch and attach all live in that binary.
- `Model` in `src/app/shell.rs` is `App` plus TuiRealm `Application<ComponentId, Msg, UserEvent>`. `App` already holds `PanelFocus`. About 51 shell call sites call TuiRealm `active`/`mount`/`umount`, mostly in `shell/overlays/`.
- `Model::run` (`src/app/shell/run.rs`) owns a crossterm `Terminal` and interleaves runtime drains, Owner snapshot adoption, the TuiRealm tick, and drawing.
- The router (`src/app/input/router.rs`, `key_policy.rs`, `resolver.rs`) reads `crossterm::event` types. `mbv-keybinds` has its own chord types and no crossterm dependency.
- `crates/mbv-components/src/list/{cursor,viewport,row_flow,marks}.rs` are already free of ratatui and TuiRealm. `viewport.rs` already counts in items (`viewport_len`).
- Other framework leaks: `mbv-ui-msg` uses `tuirealm::event::KeyEvent`, `mbv-ui-model` uses `ratatui::layout::Rect` in `context_menu.rs`, and `mbv-theme` returns ratatui `Color` from `Role::color`.

## Goals / Non-Goals

**Goals:**

- Prove the Client-core seam on one real slice (Queue, Queue playback, Status bar) in both Clients.
- Make the core focus stack the one focus authority for the whole TUI, not only for the slice.
- Keep every TUI behaviour unchanged. The existing tick-integration and component tests are the guard.

**Non-Goals:**

- Moving `App` out of the root package into its own crate.
- GUI images, artwork, the visual slot, the visualizer, text entry, overlays other than those the slice reaches, drag, and column resize.
- Any change to ctrl, the Owner process, or mpv.

## Decisions

### D1. Terms: TUI Client, GUI Client, Client core, Surface state

`CONTEXT.md` lists "frontend" as an *Avoid* term for Local process, so "front end" cannot name the two kinds of Client. The terms are **TUI Client** and **GUI Client** (kinds of Client), **Client core** (the shared, front-end-neutral layer), and **Surface state** (the neutral interaction state of one interactive surface). The `Client` definition changes from "a terminal UI" to "a TUI Client or GUI Client". The change name keeps "front-end" only as a working label.

### D2. Crate layout

```text
mbv-ui-msg      neutral KeyEvent, PointerEvent, ComponentId, Msg   (drops tuirealm)
mbv-ui-model    presentation models                                (drops ratatui)
mbv-theme       Role -> neutral Rgb; ratatui conversion behind a `ratatui` feature
mbv-surfaces    NEW: Surface state, FocusStack, HitMap, capacity   (no ratatui/tuirealm/crossterm)
mbv-components  TuiRealm adapters over mbv-surfaces
mbv-render      ratatui painters (unchanged role)
mbv (root)      NEW lib target: App, dispatch, router, ClientCore, launch/attach; bin `mbv`
mbv-gui         NEW bin: eframe app, egui painters, raw-input translation
```

`mbv-gui` depends on the `mbv` library. That library still depends on ratatui, crossterm, and TuiRealm, so the GUI program links them. The specs constrain only the `mbv` program (no window stack) and `mbv-surfaces` (no Client library). Moving `App` into its own Client crate would remove that link, but it moves about 72k lines. That belongs to a later change, if the TUI is ever dropped.

*Alternative:* one `mbv` binary with `--gui`. Rejected by the user: every TUI build would carry the window stack.

### D3. The core focus stack is the one focus authority

`mbv-surfaces::FocusStack` owns the mounted set, Panel focus, and the overlay stack, keyed by `ComponentId`. `App`'s `PanelFocus` folds into it. Shell code that today calls TuiRealm `active`/`mount`/`umount` instead requests a change on the stack. One sync step in the shell's synchronisation pass (`sync_mounted_surfaces`) then applies the stack to TuiRealm. That step is the only TuiRealm focus caller. This replaces the focus part of ADR 0022 and needs a new ADR.

*Alternative:* keep TuiRealm as focus authority and mirror it into the GUI. Rejected: two focus authorities drift, and focus is the core of the interaction contract.

### D4. Neutral input at the edge

`mbv-ui-msg` defines `KeyEvent { code, modifiers }` and `PointerEvent { kind, position }`. The TUI translates crossterm events at the event listener edge. The GUI translates egui `RawInput` events in `mbv-gui`. The router, key policy, resolver, and `LeafKeyResult` take only the neutral type. Each translator has one table test of its mapping.

### D5. Surface state for the slice

The Queue component's cursor, marks, selection, viewport, and key rules move into `mbv-surfaces` together with the framework-free list modules. `QueueComponent` stays in `mbv-components` as a TuiRealm adapter: it forwards neutral events to the Surface state and paints from it. The Queue playback panel and Status bar get the same split, for the state they own (transport focus, seek target, prefix-armed display). A component adapter holds no copy of a Surface state value.

### D6. Capacity and hit maps

Each painter returns, for the frame, the whole items that fit in each list and a `HitMap<Target>`: a list of regions with semantic targets. A region is a rectangle in `f32` logical units. The TUI converts cell rectangles to these units, and the GUI uses egui points. The core does only point-in-region tests. It never computes geometry. Paging stride comes only from the reported capacity. This keeps the existing rule that "the stride source is the painted geometry".

### D7. A Client-neutral tick

The `App`-side phases of the run loop (runtime and channel drains, Owner snapshot adoption, effects, toast expiry) move into `ClientCore::tick(now) -> TickOutcome`. `Model::run` calls it, then the TuiRealm tick and draw. The GUI calls it at the start of each eframe `update`. Worker threads wake the GUI through a waker callback that the GUI installs at construction (it calls `egui::Context::request_repaint`). The TUI installs a no-op waker and keeps its poll cadence. The GUI also calls `request_repaint_after` with the same idle cadence as the TUI, so timers still fire when no event arrives.

### D8. Toolkit: eframe with the glow renderer, custom painting

eframe's default glow (OpenGL) renderer keeps the dependency tree smaller than wgpu, and it runs on Wayland and X11 through winit. GUI lists are painted with the egui `Painter`, not with `ScrollArea` or other stateful egui widgets, because scroll and cursor belong to Surface state. egui's own widget memory is not used for any interaction state.

### D9. GUI Owner launch

The GUI reuses the library's Owner find, start, and attach code. It starts the Owner from the `mbv` program: first from the directory of its own executable, then from `PATH`. The Owner is never the GUI program, so the Owner process never maps the window stack (`headless-playback-memory-footprint`).

### D10. GUI breakpoint

`mbv-gui` maps window width in logical points to the shared breakpoint classes with its own threshold constant. The class's behaviour comes from the core. The threshold value is a presentation choice and is not in the specs.

### D11. Tests

- Surface state public-API tests go in `crates/mbv-surfaces/tests/<name>.rs`. Existing queue and list tests move with the code. They are not duplicated.
- TUI composition stays proven by the existing `src/app/tests/tick_integration/` tests. A new tick test proves that TuiRealm focus follows the core stack.
- GUI composition tests run egui headless (`egui::Context::run` with synthetic `RawInput`), with no window and no GPU. They assert core state and delivery, never appearance.
- Each input translator has one `#[case]` table.

## Risks / Trade-offs

- [The focus-stack move touches every overlay path] → Do it as its own step before any GUI code. The tick-integration focus and overlay tests must pass unchanged.
- [Two Clients double the cost of each UI change] → The user accepts this until parity. The "behaviour lives once" rule keeps the doubled part to painting only.
- [The GUI links ratatui and TuiRealm through the `mbv` library] → Accepted for now (D2). A later change can move `App` to a Client crate.
- [The TUI's Nerd Font glyphs may be missing in the GUI font] → The GUI painters choose their own icons. Specs pin no glyphs.
- [eframe owns the main thread] → The GUI program has no terminal loop to conflict with. All mbv work runs in `ClientCore::tick` inside `update`.
- [Workspace build time in CI grows] → Measure it in the PR. `mbv-gui` stays out of `default-members` if local `cargo` runs get too slow.
- [GUI live behaviour on Wayland cannot be unit tested] → The user runs the live check.

## Migration Plan

There is no user data or protocol migration. `mbv-gui` is not packaged or released in this change. The steps land in this order, and each one keeps the TUI green: neutral types, focus stack, Surface state extraction, Client-neutral tick, library target, then the GUI program. Rollback is the revert of the GUI crate. The earlier steps are TUI refactors that stand on their own.

## Open Questions

- Which font the GUI bundles, if any. This is a presentation choice.
- Whether `mbv-gui` belongs in `default-members`. Decide from the measured build time.
