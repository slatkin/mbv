# Design

## Context

See `proposal.md` (Why) for the motivation. This design starts from the state that `gui-change-driven-sections` leaves on `main` after its trim (proposal, Prerequisite): the core thread builds a `GuiSectionBatch` in `SectionLedger::collect`, dirty marks and the test audit guard the gated sections, `CoreInput::Sync` asks for a snapshot, and the core sends a *Playhead anchor* only on a discontinuity. The window side of that change (section stores, the predicted playhead, the derived values, the art scheme) is not on `main`. This change builds it in Rust.

Today `crates/mbv-gui` is about 9,300 lines of Rust (core thread, attach campaign, bridge DTOs, key and pointer translation, frame builders) and about 21,000 lines of Svelte and TypeScript in `ui/`. The Svelte side holds two kinds of code. The views paint what the core projects. The Library panel state (`ui/src/lib/library/`, about 6,100 lines) and the Settings screen state (`ui/src/lib/settings/`, about 1,300 lines) own interaction state in the window (ADR 0036, ADR 0037).

A probe at Zed revision `3c846c3d491095d75a76ca00a99fe348439736f6` built `gpui`, `gpui_platform` (Wayland only), `ui`, and `theme` with mbv's pinned toolchain (1.99.0). The probe binary links only `libc`, `libm`, `libgcc_s`, `libxkbcommon`, `libwayland-client`, `libvulkan`, and `libEGL`. Zed's workspace patches three crates (`async-task`, `calloop`, `khronos-egl`) to Zed forks, and GPUI does not build without them.

## Goals / Non-Goals

**Goals:**

- One process, one binary, no web engine. The GPUI main thread paints; the core thread stays the only owner of `App`.
- The current look and feel: the layout, the panel arrangement, the `mbv-theme` colours, the chrome fills, the canvas tokens, the two bundled fonts, and the Lucide icons.
- The same behaviour on every surface. The specs of `gui-client`, `gui-library-panel`, `gui-settings-screen`, and `client-interaction-core` stay true, except where this change's deltas say otherwise.
- Art crosses once, as encoded bytes, from the core's image cache into a GPU texture.

**Non-Goals:**

- No change to `ClientCore::tick`, the router, the hit map's semantics, or the section ledger's marks.
- No X11 backend, no macOS or Windows build.
- No change to the core's image cache. It still decodes every image for the TUI paths. A GUI-only cache mode is a separate change.
- No widget shape parity with the Svelte widgets (D7).
- No measurement spike before the port.

## Decisions

### D1. Dependencies come from one pinned Zed revision

`crates/mbv-gui/Cargo.toml` takes `gpui`, `gpui_platform`, `ui`, and `theme` as git dependencies at one `rev`. `gpui_platform` turns off its default features and turns on `wayland` only. The root `Cargo.toml` copies Zed's three `[patch.crates-io]` entries at the revisions Zed's `Cargo.lock` names. A Zed bump is one commit that changes the `rev`, the three patch revisions if Zed changed them, and the call sites that broke.

Only `mbv-gui` depends on Zed crates. The patches apply to the whole workspace, so the bump commit runs the full workspace check, not only `mbv-gui`.

`ui` and `theme` are GPL-3.0-or-later. `crates/mbv-gui/Cargo.toml` sets `license = "GPL-3.0-or-later"` in place of `license.workspace`, and the crate carries the GPL text. Every other crate stays MIT.

**Alternative rejected: crates.io `gpui` 0.2.2.** It is from October 2025, it has no `gpui_platform` split, and it has no wgpu renderer on Linux.

**Alternative rejected: `gpui-component`.** It is a second framework on top of GPUI with its own release cycle. Zed's `ui` is built and used at the same revision as `gpui`, so one pin covers both.

### D2. Two threads, two channels

GPUI owns the main thread: `main.rs` builds the GPUI `Application`, opens one window, and runs the event loop. The core thread starts before the window opens and keeps its current shape: synchronous, owner of `App`, waiting on the `std::sync::mpsc` channel of `CoreInput` with the `CLIENT_IDLE_CADENCE` timeout.

The core sends to the window through a `flume` channel of `WindowMsg`:

- `Batch(GuiSectionBatch)`: the sections that changed in one tick, as plain Rust values.
- `Art { art: GuiArtRef, bytes: Option<Vec<u8>> }`: the answer to an art request (D6).
- `Quit`: the run ended through the quit edge (the user quit, or the Owner announced a shutdown).

A GPUI foreground task awaits `recv_async` on that channel. `flume` needs no runtime, so no tokio enters `mbv-gui`. The task applies each message to the window state and calls `notify` on only the views that the message changed (D4). On `Quit` it calls `cx.quit()`.

The window sends to the core through the existing `Sender<CoreInput>`. A window close sends `CoreInput::Close`. `main.rs` joins the core thread after `Application::run` returns, as it joins it after the Tauri loop today, so the exit teardown completes before the process exits.

The serde derives, the Tauri commands, the `FrameSink` event emit, and the `base64` encode go away. The batch is moved, not serialized.

**Alternative rejected: run the core on GPUI's foreground executor.** The core blocks on its channel and runs synchronous drains. On the main thread it would stall painting. The current thread split is the one the TUI and the tick host already assume.

### D3. The window asks for a snapshot when it opens

When the root view is created, it sends `CoreInput::Sync`. The ledger marks every section "not held", and the next batch carries every present section. Before the window opens, the attach campaign answers `Sync` with `attach` and `theme`, as `gui-change-driven-sections` D4 says. A layout report applies the layout and sends nothing on its own.

There is no window-content reload in GPUI, so `Sync` is sent once per window. The requirement "The window opens with every section" owns this.

### D4. The window's module tree

`crates/mbv-gui/src/` gets two new trees.

- `window_state/`: GPUI-free Rust. The section stores (one value and its revision for each section), the playhead predictor, the derived values, the Library panel state, and the Settings screen state. The Library and Settings state are ports of `ui/src/lib/library/` and `ui/src/lib/settings/`. This tree has unit tests and no GPUI dependency, so its tests run without a window.
- `window/`: GPUI views and elements. One entity for each area that changes on its own: the player panel, the queue, the Status bar, the overlay layer, the Library panel, and the Settings screen. A root view composes them and holds the layout breakpoint. Each area entity renders as a cached view, so a batch that changes only `status` repaints only the Status bar.

A section store skips a write when the incoming revision equals the held revision. The area entity of a section is notified only when the write happens.

The rule from ADR 0022 to ADR 0024 stays. The views paint what the core projects for the shared surfaces and own no cursor, selection, or viewport of those surfaces. The Library panel and the Settings screen own their state in `window_state/`, as ADR 0036 and ADR 0037 say.

**Alternative rejected: one view for the whole window.** Every batch would rebuild the whole element tree, which is the per-tick cost the section ledger removed on the core side.

### D5. Input goes to the same router

The root view holds one focus handle and takes `on_key_down`. The window registers no GPUI keymap and no GPUI actions. `bridge/keys.rs` translates a GPUI `Keystroke` (key name and modifiers) into the router's key event, in place of the DOM `key` and `code`. The keys then go where they go today: the Library panel and the Settings screen take their own keys first in `window_state/` (ADR 0036, ADR 0037), and the rest go to the core as `CoreInput::Key`. `src/app/input/` stays the only keyboard routing site for the shared surfaces.

`bridge/pointer.rs` translates GPUI mouse down, up, move, and scroll events into the same `PointerEvent` gestures. The wheel conversion keeps the 16 px line that `LibraryVirtualList` uses, so a wheel step moves the same distance as today.

Layout reports come from the painted bounds. Each area that the hit map needs records its bounds in prepaint. After a frame, the root view builds a `LayoutReport` and sends `CoreInput::Layout` only when it differs from the last one sent. A resize therefore sends one report, and a repaint with no geometry change sends none.

### D6. Art is requested in process and decoded in the window

A section carries a `GuiArtRef`, never bytes. When a view paints a reference that the window cache does not hold, the window sends `CoreInput::Art(GuiArtRef)` one time and paints the loading state. The core answers with `WindowMsg::Art`. The bytes come from two new read-only queries: `App::gui_art_source(&GuiArtRef) -> Option<Vec<u8>>`, which copies the encoded bytes out of the image cache, and `App::trickplay_sheet_bytes(...)` for the scrubber sheets. `None` means that the cache no longer holds the image, and the window paints the loading state (spec scenario "A referenced image is no longer cached").

The window decodes the bytes on GPUI's background executor into a `RenderImage` and keeps it in a bounded LRU in `window_state/`, keyed by `GuiArtRef`. On eviction, the window calls `cx.drop_image`, so the texture leaves the GPU too. A second paint of the same reference uses the cached image and sends no request (spec scenario "The same image shows again").

The bytes are copied once from the cache into the message. They do not stay in the core after the send.

**Alternative rejected: share the image cache across threads.** It needs a lock or an `Arc` around a cache that the core mutates on every drain. The copy of one encoded image per first paint is cheaper than a lock on every tick.

### D7. Zed's widgets by default, mbv's own where the user rejects them

The views use Zed's `ui` widgets where a widget has no interaction state of its own: `Button`, `IconButton`, `Label`, `Icon`, `Divider`, `ListItem`, `Checkbox`, `Switch`, and `Tooltip`. These take their shapes from Zed and their colours from the active `Theme` (D8).

The views do not use Zed's stateful menus (`ContextMenu`, `PopoverMenu`, `DropdownMenu`, `RightClickMenu`) for the shared surfaces. Those menus own their selection and open state. For the row menu and the popovers that state lives in the Client core (requirement "The Client core holds the shared surfaces' interaction state"). mbv paints these menus from the core projection, with `ListItem` rows.

Where the user does not like a Zed widget, mbv replaces that widget with its own element in `window/widgets/`, built on GPUI `div()` and the same theme colours. A replacement is a local change: one widget module and its call sites. No widget is replaced in advance.

The text field is always mbv's own (`window/widgets/text_field.rs`). Zed's `ui_input` depends on Zed's `editor`, which is far too large for a single-line field. The field implements GPUI's `EntityInputHandler`, so typing, selection, the clipboard, and IME composition go through GPUI's Wayland text input. The Library filter field, Search, and the Settings fields use it.

Lists use GPUI's `uniform_list`. The Library rows have a fixed height, so `uniform_list` replaces `LibraryVirtualList` and its spacers. The scroll offset stays Library panel state in `window_state/` (ADR 0036).

### D8. The mbv palette becomes the active Zed theme

`window/theme.rs` builds a Zed `Theme` from the `theme` section: each `ThemeColors` and `StatusColors` field gets an mbv role, a chrome fill, or a canvas token. The mapping is one function with an exhaustive table, so a field that Zed adds in a bump shows in review. `GlobalTheme::update_theme` installs it when the `theme` section arrives. Before the first `theme` section, the window paints the attach screen with the same mapping from `mbv-theme`'s default palette, so no Zed colour shows at any time.

The canvas tokens move from `app.css` into a Rust table in `window/canvas.rs`, with the same names and values. Views that need a colour that has no `ThemeColors` field read the table directly.

mbv installs its own `ThemeSettingsProvider`. It gives IBM Plex Sans as the UI font, JetBrains Mono as the buffer font (Zed's widgets use it for monospace text), the font sizes of the current window, and `UiDensity::Default`. mbv does not use Zed's `settings` crate.

The two font families ship in the binary through `include_bytes!` and load with `text_system().add_fonts` before the window opens. The Lucide icons that the front end imports ship as SVG files through mbv's own `AssetSource` and paint through `Icon::from_path`. Zed's own icon set is not used.

### D9. The playhead and the derived values run in the window

`window_state/playhead.rs` keeps the last anchor and the `Instant` when the window received it. The anchor crosses in process, so the receive time is close to the send time with no IPC delay. The predicted position is the anchor's position plus the elapsed time, clamped to `[0, runtime]`, with no advance while paused.

While an anchor is active and not paused, the player panel requests the next animation frame and repaints only when a painted value changes (the elapsed and remaining clocks at second steps, the scrubber fill and the row progress at pixel steps). A minute timer on the foreground executor moves the finish clocks at each wall-clock minute, also while paused. Paused playback with no input paints nothing.

The derived values (elapsed, remaining, the finish clocks with the `+1d` suffix, the active row's progress, and the current Audiobooks chapter) are pure functions in `window_state/derived.rs`. They are ports of `gui-change-driven-sections` D5, with `fmt_duration_short` and `fmt_wall_clock` called from `mbv-ui-model` directly in place of TypeScript ports.

### D10. The cutover happens inside the change

The GPUI window is built beside the Tauri program in the same branch. While the port runs, `crates/mbv-gui` has a second binary target, `mbv-gui-gpui`, that starts the GPUI window over the same core thread. The Tauri binary keeps working, so each surface can be compared side by side in live tests. When every surface is ported, the last task group deletes Tauri, `ui/`, the Tauri config, the icon set, `build.rs`, and the pnpm and Node steps, and makes `mbv-gui` the GPUI binary. `mbv-gui-gpui` goes away in the same commit.

The ported Vitest tests move only when they own a contract that a Rust test does not cover already. Tests of the Library and Settings state become unit tests in `window_state/`. Tests that only checked Svelte markup do not move.

## Risks / Trade-offs

- [A Zed bump breaks call sites] → One pin and one bump commit at a time. `mbv-gui` uses a small part of `ui`, and the replacement path of D7 removes a widget that keeps breaking.
- [GPUI on Wayland with the NVIDIA driver] → The user's machine is the test target. Live tests on the user's machine come before the Tauri deletion task, and only the user runs them.
- [Wayland only] → A session without Wayland cannot open the window. mbv already depends on Wayland for `pinwin`, and the user's machines run Wayland.
- [IME and text input in mbv's own field] → The field uses GPUI's `EntityInputHandler`, which is the path Zed's editor uses for the same Wayland text-input protocol.
- [The binary becomes GPL] → Accepted. Only `mbv-gui` changes licence, and the release notes say so.
- [Two front ends in the branch for a while] → The branch builds both, so CI is slower until the deletion task. The `mbv-gui-gpui` target is temporary and has no packaging.
- [The workspace patches affect every crate] → The three patches replace `async-task`, `calloop`, and `khronos-egl`. Today no other mbv crate depends on them; the bump commit checks `cargo tree -i` for each one.

## Migration Plan

The prerequisite change merges first. This change then lands as one PR. Users of a release before it keep the Tauri GUI. The first release after it ships the GPUI binary under the same name, app id, and desktop entry, so a package upgrade swaps the front end with no user step. The packages drop WebKitGTK 4.1 and add the Vulkan loader.

Rollback is a revert of the merge commit. No configuration, state file, or Owner protocol changes, so the Tauri GUI from the previous release works against the same Owner and the same files.
