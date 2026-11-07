# Design: extract-client-core

## Context

See `proposal.md` for the reason. The requirements are in `specs/client-interaction-core` and the two deltas.

This change is the toolkit-neutral part of the earlier `add-gui-front-end` work. That work also built a GUI program on one toolkit. The user dropped that toolkit, so this change keeps only what any GUI Client needs. The decision numbers stay the same as in that work, because code comments cite them. D8 (the GUI toolkit) belonged to the GUI program and is not in this change.

Current state that shaped the approach:

- The Owner process has no UI. Every Client talks to it over ctrl (`docs/architecture/owner-client-operation-split.md`). A GUI Client needs no Owner change.
- The root `mbv` package was binary-only. The shell `App`, dispatch, the router, and Owner launch and attach all lived in that binary.
- `Model` in `src/app/shell.rs` is `App` plus TuiRealm `Application<ComponentId, Msg, UserEvent>`. `App` held `PanelFocus`. About 51 shell call sites called TuiRealm `active`/`mount`/`umount`, mostly in `shell/overlays/`.
- `Model::run` (`src/app/shell/run.rs`) owns a crossterm `Terminal` and interleaved runtime drains, Owner snapshot adoption, the TuiRealm tick, and drawing.
- The router (`src/app/input/router.rs`, `key_policy.rs`, `resolver.rs`) read `crossterm::event` types. `mbv-keybinds` has its own chord types and no crossterm dependency.
- `crates/mbv-components/src/list/{cursor,viewport,row_flow,marks}.rs` were already free of ratatui and TuiRealm.
- Other framework leaks: `mbv-ui-msg` used `tuirealm::event::KeyEvent`, `mbv-ui-model` used `ratatui::layout::Rect` in `context_menu.rs`, and `mbv-theme` returned ratatui `Color` from `Role::color`.

## Goals / Non-Goals

**Goals:**

- Extract the Client-core seam on one real slice (Queue, Queue playback, Status bar).
- Make the core focus stack the one focus authority for the whole TUI, not only for the slice.
- Keep every TUI behaviour unchanged. The existing tick-integration and component tests are the guard.

**Non-Goals:**

- Any GUI program, toolkit, or painter.
- Moving `App` out of the root package into its own crate.
- Any change to ctrl, the Owner process, or mpv.

## Decisions

### D1. Terms: TUI Client, GUI Client, Client core, Surface state

`CONTEXT.md` lists "frontend" as an *Avoid* term for Local process, so "front end" cannot name the two kinds of Client. The terms are **TUI Client** and **GUI Client** (kinds of Client), **Client core** (the shared, front-end-neutral layer), and **Surface state** (the neutral interaction state of one interactive surface). The `Client` definition changes from "a terminal UI" to "a TUI Client or GUI Client".

### D2. Crate layout

```text
mbv-ui-msg      neutral KeyEvent, PointerEvent, ComponentId, Msg   (drops tuirealm)
mbv-ui-model    presentation models                                (drops ratatui)
mbv-theme       Role -> neutral Rgb; ratatui conversion behind a `ratatui` feature
mbv-surfaces    NEW: Surface state, FocusStack, HitMap, capacity, gesture policy, breakpoints
mbv-components  TuiRealm adapters over mbv-surfaces
mbv-render      ratatui painters (unchanged role)
mbv (root)      NEW lib target: App, dispatch, router, ClientCore, launch/attach; bin `mbv`
```

A GUI Client program depends on the `mbv` library. That library still depends on ratatui, crossterm, and TuiRealm, so a GUI program links them. The specs constrain only `mbv-surfaces` (no Client library). Moving `App` into its own Client crate would remove that link, but it moves about 72k lines. That belongs to a later change.

### D3. The core focus stack is the one focus authority

`mbv-surfaces::FocusStack` owns the mounted set, Panel focus, and the overlay stack, keyed by `ComponentId`. `App`'s `PanelFocus` folds into it. Shell code requests a change on the stack instead of calling TuiRealm `active`/`mount`/`umount`. One sync step in the shell's synchronisation pass (`sync_mounted_surfaces`) applies the stack to TuiRealm. That step is the only TuiRealm focus caller. This replaces the focus part of ADR 0022 and needs a new ADR.

*Alternative:* keep TuiRealm as focus authority and mirror it into a GUI. Rejected: two focus authorities drift, and focus is the core of the interaction contract.

### D4. Neutral input at the edge

`mbv-ui-msg` defines `KeyEvent { code, modifiers }` and `PointerEvent { kind, position, modifiers }`. The TUI translates crossterm events at the event listener edge. A GUI Client translates its toolkit's events at its own edge. The router, key policy, resolver, and `LeafKeyResult` take only the neutral type. Each translator has one table test of its mapping. `src/app/input/gui.rs` is the neutral routing entry for a GUI Client: it takes `GuiInput` (a neutral key or pointer event) and a `GuiSurfaceLeaf`, and it applies the router outcome to the `App`.

### D5. Surface state for the slice

The Queue component's cursor, marks, selection, viewport, and key rules move into `mbv-surfaces` together with the framework-free list modules. `QueueComponent` stays in `mbv-components` as a TuiRealm adapter: it forwards neutral events to the Surface state and paints from it. The Queue playback panel and Status bar get the same split, for the state they own (transport focus, seek target, prefix-armed display). A component adapter holds no copy of a Surface state value. One Queue Surface projection and one neutral row projection serve every Client.

### D6. Capacity and hit maps

Each painter returns, for the frame, the whole items that fit in each list and a `HitMap<Target>`: a list of regions with semantic targets. A region is a rectangle in `f32` logical units. The TUI converts cell rectangles to these units, and a GUI uses its toolkit's logical points. The core does only point-in-region tests. It never computes geometry. Paging stride comes only from the reported capacity.

### D7. A Client-neutral tick

The `App`-side phases of the run loop (runtime and channel drains, Owner snapshot adoption, effects, toast expiry) move into `ClientCore::tick(now) -> TickOutcome`. `Model::run` calls it, then the TuiRealm tick and draw. A GUI Client calls it at the start of each frame. The Owner-snapshot links feed their drains through an app-side relay thread that forwards each event in order and then calls the waker the Client installed — a shared `ClientWaker` slot that the `App` and the `ClientCore` hold, so the install reaches every relay, including the one the initial attach already spawned. A GUI Client installs its toolkit's repaint request. The TUI installs a no-op waker and keeps its poll cadence. Every other channel relies on the idle cadence `CLIENT_IDLE_CADENCE` (50 ms), so timers and worker results still land when no event wakes the Client.

### D9. GUI Owner launch

A GUI Client reuses the library's Owner find, start, and attach code. The library entry takes an `owner_exe` override: the `mbv` program that the GUI Client resolved. The Owner is never the GUI program, so the Owner process never maps a window stack (`headless-playback-memory-footprint`). A GUI Client calls `declare_gui_client` at start-up: its connections send a plain Client Hello and never take part in Pin swap, because the swap's launch snapshot is a TUI shell query. The order in which a GUI program resolves `mbv` is part of the GUI change.

### D10. Shared breakpoint classes

`mbv-surfaces` defines `BreakpointClass` (`Mini`, `Narrow`, `Wide`), `classify`, and the core Wide-hero rule. Each Client maps its own size to a class with its own threshold constants. The class's behaviour comes from the core. Threshold values are a presentation choice and are not in the specs. No TUI threshold moves in this change.

### D11. Tests

- Surface state public-API tests go in `crates/mbv-surfaces/tests/<name>.rs`. Existing queue and list tests move with the code. They are not duplicated.
- TUI composition stays proven by the existing `src/app/tests/tick_integration/` tests. A new tick test proves that TuiRealm focus follows the core stack.
- Each input translator has one `#[case]` table.

## Risks / Trade-offs

- [The focus-stack move touches every overlay path] → Do it as its own step. The tick-integration focus and overlay tests must pass unchanged.
- [A GUI program links ratatui and TuiRealm through the `mbv` library] → Accepted for now (D2). A later change can move `App` to a Client crate.
- [Library items exist that only a future GUI Client calls] → Accepted. They are the seam the GUI change builds on.

## Migration Plan

There is no user data or protocol migration. The steps land in this order, and each one keeps the TUI green: neutral types, focus stack, Surface state extraction, Client-neutral tick, library target, shared projections and breakpoint classes.

## Open Questions

- None.
