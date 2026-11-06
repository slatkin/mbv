# Design: add-tauri-gui-client

## Context

See `proposal.md` for the reason and `specs/gui-client` for the requirements.

The current state that shapes the approach (after `extract-client-core`, PR #984):

- The root `mbv` package has a library target. It exports `App`, `ClientCore`, `ClientTickHost`, `ClientWaker`, `CLIENT_IDLE_CADENCE`, the slice projections (`QueueSurfaceProjection`, `QueuePlaybackProjection`, `QueueFooterProjection`, `StatusBarProjection`), the GUI routing entry (`GuiInput`, `GuiSurfaceLeaf`, `App::route_gui_input`, `App::route_gui_pointer`), and the Owner steps (`find_owner`, `start_owner`, `attach_owner` with its `owner_exe` override, `declare_gui_client`).
- `mbv-surfaces` holds the Surface state of the slice, the focus stack, `HitMap`, the pointer gesture policy, and the shared breakpoint classes.
- Every Client honours the `client-interaction-core` spec: behaviour lives once, neutral input, capacity in items, a per-frame semantic hit map, and shared breakpoint classes.
- Tauri 2 owns the main thread and its event loop (tao, then GTK on Linux). Its command handlers and event emitters run under tokio. On Linux the webview is WebKitGTK 4.1, which draws text through fontconfig and FreeType.

## Goals / Non-Goals

**Goals:**

- A GUI Client that presents the first slice (Queue, Queue playback, Status bar) with the TUI's behaviour, through the shared Client core.
- Text and icons that match the user's desktop, because the webview uses the system font stack.
- A thin front end: it paints projections and reports input, capacity, and hit regions. It holds no interaction rules.

**Non-Goals:**

- Surfaces outside the slice, artwork, the visualizer, drag, column resize, `--pin`, tray, GUI-specific MPRIS work, packaging. The proposal lists them.
- Any change to ctrl, the Owner process, mpv, or the TUI.
- A web or remote UI. The webview loads only bundled assets.

## Decisions

### D1. Crate and front-end layout

```text
crates/mbv-gui/            Tauri 2 binary `mbv-gui`; links the `mbv` library
  src/main.rs              Tauri builder, command registration, core thread start
  src/core_thread.rs       the Client core loop (D3)
  src/bridge/              serde DTOs for projections and input (D4), key translator (D6)
  ui/                      Svelte 5 + Vite front end (pnpm), built into Tauri's frontendDist
```

`mbv-gui` is a workspace member and not in `default-members`, so the default build and nextest skip it. CI clippy and fmt cover it through `--workspace`.

*Alternative:* SvelteKit with `adapter-static`. Rejected: the GUI is one window with no routes, so SvelteKit adds a router and a build adapter for nothing. shadcn-svelte supports plain Vite.

### D2. Versions

Every dependency starts at its latest stable release, checked on 2026-11-06: `tauri` 2.12.5 (MSRV 1.90) and `tauri-build` 2.7.4; `@tauri-apps/api` and `@tauri-apps/cli` 2.12.4; `svelte` 5.57.3; `vite` 8.3.6 with `@sveltejs/vite-plugin-svelte` 7.3.1; `tailwindcss` 4.3.4; `shadcn-svelte` 1.8.0 (on `bits-ui` 2.20.6); `@lucide/svelte` 1.70.0; `vitest` 5.1.0 with `@testing-library/svelte` 5.4.2; `svelte-check` 4.7.8; pnpm 12. The peer ranges of these releases fit one another (checked 2026-11-06).

One exception: `typescript` is 6.0.3, the latest 6.x, not the latest release 7.0.2. `svelte-check` 4.7.8 accepts only `typescript` `^5.0.0 || ^6.0.0`, and the type check is a gate (task 1.3). Move to 7.x when a `svelte-check` release accepts it.

The scaffold uses exactly these versions. A peer conflict that appears at scaffold time is a design gap: the implementer reports it and does not pick a version.

### D3. The Client core runs on its own thread

The Tauri main thread runs the window event loop. One dedicated, synchronous **core thread** creates the `App`, attaches to the Owner, and runs `ClientCore::tick` in a loop. Tauri command handlers never touch the `App`. They push a `CoreInput` value into a `std::sync::mpsc` channel to the core thread. The core thread waits on that channel with a timeout of `CLIENT_IDLE_CADENCE`. The installed `ClientWaker` sends a wake value into the same channel, so Owner events and timers wake it as they wake the TUI.

After each tick that changed something, the core thread builds a `GuiFrame` (D4) and emits it to the window with `AppHandle::emit`, which is safe to call from any thread. tokio stays inside Tauri. The core thread and the `mbv` library never see it.

The core thread also emits the current frame, changed or not, after every `report_layout`. The front end sends `report_layout` when it mounts. So the first frame, and the frame after a reload of the window content, never wait for a state change.

*Alternative:* hold the `App` in Tauri managed state behind a `Mutex` and tick from a timer. Rejected: command handlers on tokio workers would then run `App` code, and the tick would depend on an async timer.

### D4. The bridge contract

Rust to front end, one event `mbv://frame` that carries a `GuiFrame`: the focus (surface and Panel), the Queue rows inside the current viewport with their stable `QueueSlotId`, cursor, marks, and the active slot; the Queue playback projection; the footer pills; the Status bar projection; the breakpoint class; and the theme (D7) when it changes. Each projection is a serde DTO in `mbv-gui/src/bridge/`, built from the library projections. The front end keeps only the last frame.

Front end to Rust, three commands:

- `key(DomKey)`: the DOM `KeyboardEvent` fields (`key`, `code`, modifier flags, `repeat`). The Rust side translates them to a neutral `KeyEvent` (D6).
- `pointer(DomPointer)`: kind (press, release, move, wheel), button, the position in logical pixels relative to the window, and the wheel delta.
- `report_layout(LayoutReport)`: for each list, the number of whole visible rows, and the painted regions with their semantic target ids. The front end sends it after each paint that moved a region (a Svelte `$effect` after the DOM update, plus a `ResizeObserver`).

The core thread applies a `LayoutReport` to the Surface state's capacity and `HitMap`. `App::route_gui_pointer` resolves a gesture only against the leaf it receives. So the core thread first resolves the point against one combined `HitMap` of all painted slice surfaces from the last `LayoutReport`. Then it applies the focus change that the matching TUI click applies, and passes the surface that owns the region as the leaf to `App::route_gui_pointer`. A point in no region resolves to nothing (`client-interaction-core`: pointer targets resolve from a semantic hit map). A press on a transport control therefore works while the Queue panel has focus. The front end never decides what a click means.

*Alternative:* the DOM resolves the clicked element and sends the target id. Rejected: it moves gesture rules (double-click window, range selection) into TypeScript, and the spec requires that the core resolve against the hit map.

### D5. Lists paint only the viewport

A GUI list is a fixed-row-height column with no native DOM scroll. The core's viewport decides which rows the frame carries. Wheel input goes to the core like any pointer event. So scroll follows the cursor through the same Surface state rules as the TUI. The front end measures the list's height and reports whole rows as capacity.

*Alternative:* native DOM scrolling with a virtual list. Rejected: the DOM would then own scroll position, which is a second scroll authority.

### D6. Keys translate on the Rust side

The DOM key fields are translated to `KeyEvent` in Rust, in `bridge/keys.rs`. A `#[case]` table test covers the mapping, as the TUI's crossterm translator has. The front end prevents the default action for every key while the window has focus. Before it does, it offers no shortcut of its own, so webview defaults such as reload, find, zoom, and back navigation never run.

*Alternative:* translate in TypeScript. Rejected: then two languages own the translation tables, and the Rust nextest gate would not cover them.

### D7. Theme

`mbv-theme` resolves each role to a neutral `Rgb`. The frame carries a role-to-colour map. The front end writes it into CSS custom properties on `:root`, and the shadcn-svelte theme variables (`--background`, `--foreground`, `--primary`, `--muted`, `--accent`, `--border`, `--ring`, and the rest) point at mbv role properties. shadcn-svelte components therefore paint in the mbv palette with no second palette.

### D8. Fonts and icons

The front end uses the system UI font through CSS (`font-family: system-ui, sans-serif`) and the system monospace font for monospace text, so WebKitGTK renders text with the user's fontconfig settings. The bundle embeds no font file. Icons come from `@lucide/svelte`. The GUI paints no emoji or Nerd Font glyph. The playback-target indicator, the transport controls, and the playing-row marker are Lucide icons.

### D9. Owner launch

The core thread calls `declare_gui_client`, then `find_owner`. When no Owner runs, it starts one from the `mbv` program: first from the directory of the `mbv-gui` executable, then from `PATH`, through the `owner_exe` override of `attach_owner`. The Owner is never the GUI program.

Each attach outcome gets the response that `startup::run_local_instance` gives the TUI Client. The core thread classifies an attach error with the public `RemotePlayerError` predicates:

- `OwnerState::Refuse` (the Owner holds the lock but does not accept connections): the window shows `refusal_text()`.
- `is_exclusive_owner` (Stay-alive disabled and a Client attached): the window shows `exclusive_owner_refusal_text(&error)`.
- `is_owner_shutting_down`: the window shows a waiting state. The core thread waits for the Owner to exit, then starts an Owner and attaches.
- `is_owner_build_mismatch`: the window shows the mismatch text with **Restart** (key R) and **Quit** (key Q). Restart calls `mbv_daemon::signal_owner` on the owner lock path, waits for the Owner to exit, starts an Owner, and attaches. A mismatch after a requested restart waits again, as the TUI does. Quit closes the window.
- Any other error: the window shows "failed to attach to Owner process: {error}".

Before attach there is no `App`. In these states the core thread handles R and Q itself, and the two buttons resolve through the `LayoutReport` regions. The front end still interprets no key.

The core thread does not run the TUI's post-attach service bootstrap. `spawn_startup_services`, `fetch_home_at_startup`, `init_idle_feed`, and capability registration (`src/app/shell/run/drains.rs:521`, `src/app/shell/run.rs:319-324`) belong to the TUI shell `Model`, not to `App`. The first slice presents no library surface (Non-Goals), so the core thread does not run those steps, and a configured-but-uncached provider does not bootstrap for the GUI Client. A later change that ports a library surface must first factor those steps into a shared Client step; it must not duplicate them in `mbv-gui`.

### D10. Breakpoint

The front end reports the window's inner width in logical pixels in `LayoutReport`. The core thread classifies it with the shared breakpoint classes and a GUI threshold constant in `mbv-gui`. The class's behaviour comes from the core. The classes differ only in Library and hero placement (`mbv-surfaces` `breakpoint.rs`), and the first slice presents neither. So in the first slice the class changes nothing visible. It is wired for later surfaces. The slice has no Narrow-specific layout of its own.

### D11. Webview hardening

The front end loads only the bundled assets. The CSP allows only `'self'` and Tauri's IPC origin. The Tauri capability file allows only the three mbv commands and the event listener. The webview's own context menu is turned off.

### D12. tokio edge

`mbv-gui` joins `mbv-desktop` in the AGENTS.md list of tokio edges. Only Tauri's own runtime uses tokio. No mbv code in `mbv-gui` is `async`, apart from what a Tauri command signature requires.

### D13. Tests

- Rust (nextest, `crates/mbv-gui/tests/`): the key translator table; `LayoutReport` applied to capacity and the hit map; a `GuiFrame` built from a stubbed `App` (the root `test` feature) for a queue with slots and for an empty queue; a core-thread step test with a mocked channel: input in, frame out, and a waker wake causes a tick; a press on the play/pause region while the Queue panel has focus dispatches the playback request (D4); and one `#[case]` table that maps each attach outcome to its window state (D9). No Tauri runtime, no webview, and no live Owner.
- Front end (vitest with `@testing-library/svelte`, `@tauri-apps/api` mocked): a component renders a given frame, and a key or pointer event calls the right command with the right payload. These tests assert delivery and structure, not appearance.
- Live behaviour in the real window is the user's manual check.

### D14. Build and CI

`tauri::generate_context!` panics at compile time when the `frontendDist` directory does not exist (`tauri-codegen`, `src/context.rs`). Every `--workspace` build compiles `mbv-gui`, so it needs WebKitGTK 4.1 and a built `ui/`. The `build` and feature-matrix jobs in `.github/workflows/build.yml` add `webkit2gtk-4.1` and pnpm to their pacman installs, and build `ui/` before `cargo clippy --workspace` and `cargo hack check --workspace`. CI nextest runs only `default-members`, so a new GUI job runs `cargo nextest run -p mbv-gui` and the front-end check, lint, and test targets. The local `cargo clippy --workspace` gate has the same prerequisites.

## Risks / Trade-offs

- [WebKitGTK on some GPU and Wayland setups shows a blank window (the DMA-BUF renderer)] → The user's live check covers their setup. The workaround, `WEBKIT_DISABLE_DMABUF_RENDERER=1`, is documented. The app does not set it by default.
- [IPC per key and per pointer event adds latency] → Tauri IPC is local and takes well under one frame. The spec requires only next-frame response.
- [`LayoutReport` traffic on resize] → The front end sends it only when a region moved, at most once per animation frame.
- [TypeScript stays one major behind its latest release] → D2 records the reason (the `svelte-check` peer range) and the condition to move.
- [Two toolchains (cargo and pnpm)] → `make` targets wrap the pnpm steps. D14 sets the CI jobs.
- [The GUI links ratatui, crossterm, and TuiRealm through the `mbv` library] → Accepted, as in `extract-client-core` D2.
- [The front end grows its own behaviour over time] → The `gui-client` requirement "The window content holds no interaction state" and the reload scenario make that drift a spec failure.

## Migration Plan

There is no user data or protocol migration. `mbv-gui` is not packaged. Implementation starts after PR #984 merges. The order is: crate and front-end scaffold, the core thread and bridge, the slice components, theme and icons, then docs. Rollback is to remove `crates/mbv-gui`.

## Open Questions

- The GUI threshold constants for the breakpoint classes. The implementer picks values and records them in `mbv-gui`. They do not change the specs.
