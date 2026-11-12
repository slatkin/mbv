# Design

## Context

See `proposal.md` (Why) for the motivation. This design starts from the trimmed Rust half of `gui-change-driven-sections` (proposal, Prerequisite), on whose branch this change stacks: the core thread builds a `GuiSectionBatch` in `SectionLedger::collect`, dirty marks and the test audit guard the gated sections, `CoreInput::Sync` asks for a snapshot, the core sends a *Playhead anchor* only on a discontinuity, and the Library content rides `libraryList`, `libraryDetail`, and `rowMenu`. The window side of that change (section stores, the predicted playhead, the derived values, the art scheme) is not built. This change builds it in Rust.

On that base the Tauri window does not paint: the Svelte code still reads `mbv://frame`, the clock strings, and the `library` section. No working Tauri window exists on the branch, so every live check compares the GPUI window with the last released Tauri package (D10).

Today `crates/mbv-gui` is about 9,300 lines of Rust (core thread, attach campaign, bridge DTOs, key and pointer translation, frame builders) and about 21,000 lines of Svelte and TypeScript in `ui/`. The Svelte side holds two kinds of code. The views paint what the core projects. The Library panel state (`ui/src/lib/library/`, about 6,100 lines) and the Settings screen state (`ui/src/lib/settings/`, about 1,300 lines) own interaction state in the window (ADR 0036, ADR 0037). The Library panel's requirements live in the `gui-client` spec. The dragged panel widths live in WebKit's `localStorage` (`ui/src/lib/bridge/resize.ts`).

A probe at Zed revision `3c846c3d491095d75a76ca00a99fe348439736f6` built `gpui`, `gpui_platform` (Wayland only), `ui`, and `theme` with mbv's pinned toolchain (1.99.0). The probe binary links only `libc`, `libm`, `libgcc_s`, `libxkbcommon`, `libwayland-client`, `libvulkan`, and `libEGL`. `gpui_wgpu` asks wgpu for Vulkan and GL, so EGL is the GL fallback. Zed's workspace patches three crates (`async-task`, `calloop`, `khronos-egl`) to Zed forks, and GPUI does not build without them. The dependency graph has no `tokio`.

## Goals / Non-Goals

**Goals:**

- One process, one binary, no web engine. The GPUI main thread paints; the core thread stays the only owner of `App`.
- The current look and feel: the layout, the panel arrangement, the `mbv-theme` colours, the chrome fills, the canvas tokens, the two bundled fonts, and the Lucide icons.
- The same behaviour on every surface. The specs of `gui-client`, `gui-settings-screen`, and `client-interaction-core` stay true, except where this change's deltas say otherwise.
- Art crosses once, as encoded bytes, from the core's image cache into a GPU texture.
- No repaint while nothing painted changes, also during playback.

**Non-Goals:**

- No change to `ClientCore::tick`, the router, the hit map's semantics, or the section ledger's marks.
- No X11 backend, no macOS or Windows build.
- No change to the core's image cache. It still decodes every image for the TUI paths. A GUI-only cache mode is a separate change.
- No widget shape parity with the Svelte widgets (D7).
- No measurement spike before the port.

## Decisions

### D1. Dependencies come from one pinned Zed revision

`crates/mbv-gui/Cargo.toml` takes `gpui`, `gpui_platform`, `ui`, and `theme` as git dependencies at revision `3c846c3`. `gpui_platform` turns off its default features and turns on `wayland` only. The dev-dependencies add `gpui` with `test-support` for the `TestAppContext` tests. The root `Cargo.toml` copies Zed's three `[patch.crates-io]` entries at the revisions Zed's `Cargo.lock` names.

A Zed bump is one commit that changes the `rev`, the three patch revisions if Zed changed them, and the call sites that broke. It repeats the probe checks: the `x11` feature stays off, the graph has no `tokio`, and no other mbv crate depends on a patched crate. The patches apply to the whole workspace, so the bump commit runs the full workspace check.

`ui` and `theme` are GPL-3.0-or-later. `crates/mbv-gui/Cargo.toml` sets `license = "GPL-3.0-or-later"` in place of `license.workspace`, and the crate carries the GPL text. Every other crate stays MIT.

**Alternative rejected: crates.io `gpui` 0.2.2.** It is from October 2025, it has no `gpui_platform` split, and it has no wgpu renderer on Linux.

**Alternative rejected: `gpui-component`.** It is a second framework on top of GPUI with its own release cycle. Zed's `ui` is built and used at the same revision as `gpui`, so one pin covers both.

### D2. Two threads, two channels

GPUI owns the main thread: `main.rs` builds the GPUI `Application`, opens one window, and runs the event loop. The core thread starts before the window opens and keeps its current shape: synchronous, owner of `App`, waiting on the `std::sync::mpsc` channel of `CoreInput` with the `CLIENT_IDLE_CADENCE` timeout.

The core sends to the window through a `flume` channel of `WindowMsg`:

- `Batch(GuiSectionBatch)`: the sections that changed in one tick, as plain Rust values.
- `Art { art: GuiArtRef, bytes: Option<Vec<u8>> }`: the answer to an art request (D6).
- `Quit`: the run ended. Every end of the run sends it: the user's quit, the Owner's announced shutdown, and `CoreInput::Close`.

A GPUI foreground task awaits `recv_async` on that channel. `flume` needs no runtime, so no tokio enters `mbv-gui`. The task applies each message to the window state and calls `notify` on only the views that the message changed (D4). On `Quit` it calls `cx.quit()`.

The window sends to the core through the existing `Sender<CoreInput>`. The window's close request sends `CoreInput::Close` and does not close the window itself. The core runs the quit edge and sends `Quit`, and the window then quits the application. GPUI does not quit when its last window closes, so this path is the one that ends `Application::run`. `main.rs` joins the core thread after `run` returns, so the exit teardown completes before the process exits.

The serde derives, the Tauri commands, the `FrameSink` event emit, and the `base64` encode go away. The batch is moved, not serialized.

**Alternative rejected: run the core on GPUI's foreground executor.** The core blocks on its channel and runs synchronous drains. On the main thread it would stall painting. The current thread split is the one the TUI and the tick host already assume.

### D3. The window asks for a snapshot when it opens

When the root view is created, it sends `CoreInput::Sync`. The ledger marks every section "not held", and the next batch carries every present section. Before the window opens, the attach campaign answers `Sync` with `attach` and `theme`. A layout report applies the layout and sends nothing on its own.

There is no window-content reload in GPUI, so `Sync` is sent once per window. The requirement "The window opens with every section" owns this.

### D4. The window's module tree

`crates/mbv-gui/src/` gets two new trees.

- `window_state/`: GPUI-free Rust. The section stores (one value and its revision for each section), the playhead predictor, the derived values, the art cache, the panel widths, the Library panel state, and the Settings screen state. The Library and Settings state are ports of `ui/src/lib/library/` and `ui/src/lib/settings/`. This tree has unit tests and no GPUI dependency, so its tests run without a window.
- `window/`: GPUI views and elements. One entity for each area that changes on its own: the player panel, the queue, the Status bar, the overlay layer, the Library panel, and the Settings screen. A root view composes them and holds the layout breakpoint. Each area entity renders as a cached view, so a batch that changes only `status` repaints only the Status bar.

A section store skips a write when the incoming revision equals the held revision. The area entity of a section is notified only when the write happens.

The rule from ADR 0022 to ADR 0024 stays. The views paint what the core projects for the shared surfaces and own no cursor, selection, or viewport of those surfaces. The Library panel and the Settings screen own their state in `window_state/`, as ADR 0036 and ADR 0037 say.

**Alternative rejected: one view for the whole window.** Every batch would rebuild the whole element tree, which is the per-tick cost the section ledger removed on the core side.

### D5. Input goes to the same router

**Keys.** The window registers no GPUI keymap and no GPUI actions. A key goes to the first of these that takes it, and the taker stops propagation:

1. The focused text field (D7). It takes printable text, Backspace, Delete, the arrow keys, Home, End, and Ctrl+A, C, V, X, and gives its owner submit (Enter) and cancel (Escape).
2. The Library panel state or the Settings screen state in `window_state/`, for the keys that ADR 0036, ADR 0037, and the requirement "The window content takes the window keys" give them.
3. The Client core, as `CoreInput::Key`, through the root view's `on_key_down`.

`bridge/keys.rs` translates a GPUI `Keystroke` (key name and modifiers) into the router's key event, in place of the DOM `key` and `code`. `src/app/input/` stays the only keyboard routing site for the shared surfaces.

**Pointer.** `bridge/pointer.rs` translates GPUI mouse down, up, move, and scroll events into the same `PointerEvent` gestures, and the root view forwards them to the core. On the shared surfaces (the player panel, the queue, the row menu, the popovers, and the Status bar), no element takes `on_click`, `on_toggle`, or another handler that stops propagation. Every press on them reaches the root forward and resolves through the core's hit map, as the requirement "GUI pointer input resolves through semantic targets" says. The Library panel and the Settings screen take their own pointer events, as they do today. The wheel conversion keeps 16 px for one line.

**Layout reports.** Each area that the hit map needs records its painted bounds in prepaint. After a frame, the root view builds a `LayoutReport` and sends `CoreInput::Layout` only when it differs from the last one sent. A resize therefore sends one report, and a repaint with no geometry change sends none.

### D6. Art is requested in process and decoded in the window

`GuiArtRef` lives in `mbv-gui`: `Card(key)` for an image-cache entry, `Trickplay(TrickplaySheetKey)` for a sheet, `Placeholder`, and `Loading`. The Library art, `GuiCardArtwork` in `playback`, and the tile's sheet in `hover` carry it in place of their `data:` payloads. `artwork.rs`, the trickplay sheet latch, `data_url`, and `base64` go away.

When a view paints a `Card` or `Trickplay` reference that the window cache does not hold, the window sends `CoreInput::Art(GuiArtRef)` one time and paints the loading state. The tick host answers in `tick_input` with `WindowMsg::Art`, through two new read-only queries in `src/app/`: `App::gui_art_source(&str) -> Option<SourceImage>` for a cache key and `App::trickplay_sheet_bytes(&TrickplaySheetKey) -> Option<Arc<[u8]>>`. They never call the projections that start fetches. An art request sets no dirty mark and does not set `input_received`, so it does not reset the navigation-idle clock of the Hero neighbour fetch. `None` means that the cache no longer holds the image, and the window paints the loading state (spec scenario "A referenced image is no longer cached"). The attach campaign answers every art request with `None`.

The window decodes the bytes on GPUI's background executor into a `RenderImage` and keeps it in a bounded LRU in `window_state/`, keyed by `GuiArtRef`. The bound is the core's image cache capacity (`image_cache_size`), so the window holds no more images than the core can name. On eviction, the window calls `cx.drop_image`, so the texture leaves the GPU too. A second paint of the same reference uses the cached image and sends no request (spec scenario "The same image shows again"). The `Placeholder` image is decoded once at start from `QUEUE_CARD_PLACEHOLDER_BYTES` and never evicted.

**Alternative rejected: share the image cache across threads.** It needs a lock or an `Arc` around a cache that the core mutates on every drain. The copy of one encoded image per first paint is cheaper than a lock on every tick.

### D7. Zed's widgets by default, mbv's own where the user rejects them

The views use Zed's `ui` widgets where a widget has no interaction state of its own: `Button`, `IconButton`, `Label`, `Icon`, `Divider`, `ListItem`, `Checkbox`, `Switch`, and `Tooltip`. These take their shapes from Zed and their colours from the active `Theme` (D8). On the shared surfaces they are paint only, with no click handler (D5).

The views do not use Zed's stateful menus (`ContextMenu`, `PopoverMenu`, `DropdownMenu`, `RightClickMenu`) for the shared surfaces. Those menus own their selection and open state. For the row menu and the popovers that state lives in the Client core (requirement "The Client core holds the shared surfaces' interaction state"). mbv paints these menus from the core projection, with `ListItem` rows.

Where the user does not like a Zed widget, mbv replaces that widget with its own element in `window/widgets/`, built on GPUI `div()` and the same theme colours. A replacement is a local change: one widget module and its call sites. No widget is replaced in advance.

The text field is always mbv's own (`window/widgets/text_field.rs`). Zed's `ui_input` depends on Zed's `editor`, which is far too large for a single-line field. The field implements GPUI's `EntityInputHandler`, so typing, selection, the clipboard, and IME composition go through GPUI's Wayland text input. The Library filter field, Search, and the Settings fields use it.

Lists use GPUI's `uniform_list`. The Library rows have a fixed height, so `uniform_list` replaces `LibraryVirtualList` and its spacers. The list's `UniformListScrollHandle` is the live scroll position while the user scrolls. After each scroll event the view copies the offset into the Library panel state in `window_state/`, and that copy is the authority (ADR 0036). The view writes the stored offset back into the handle only on a discrete event: a destination switch, a collection change, or a reveal. A list update writes nothing into the handle, so it keeps the scroll.

### D8. The mbv palette becomes the active Zed theme

`window/theme.rs` builds a Zed `Theme` from the `theme` section. `ThemeColors` and `StatusColors` are struct literals with no rest pattern, and each field gets an mbv role, a chrome fill, or a canvas token. The compiler then fails the build when a Zed bump adds a field, so no field keeps a Zed colour by accident and no test is needed for it. `GlobalTheme::update_theme` installs the theme when the `theme` section arrives, and `cx.refresh_windows()` follows, because a cached view does not repaint for a global change. Before the first `theme` section, the window paints the attach screen with the same mapping from `mbv-theme`'s default palette, so no Zed colour shows at any time.

The canvas tokens move from `app.css` into a Rust table in `window/canvas.rs`, with the same names and values. Views that need a colour that has no `ThemeColors` field read the table directly.

mbv installs its own `ThemeSettingsProvider`. It gives IBM Plex Sans as the UI font, JetBrains Mono as the buffer font (Zed's widgets use it for monospace text), the font sizes of the current window, and `UiDensity::Default`. mbv does not use Zed's `settings` crate.

**Fonts.** The two families ship in the binary as TTF files from the upstream IBM Plex and JetBrains Mono releases, with their OFL licence texts, through `include_bytes!`, and load with `text_system().add_fonts` before the window opens. The `@fontsource` packages hold only WOFF and WOFF2 files, which the font loader does not read. GPUI's Linux text system also loads the system font database, and mbv cannot turn that off without a GPUI patch. That matches today's behaviour: the Svelte window loads only the Latin subsets, and CJK and Cyrillic titles already fall back to system fonts. The spec says that the bundled faces paint the text and that a system font paints only the glyphs they lack.

**Icons.** The Lucide icons that the front end uses ship as SVG files from the upstream Lucide release, with the ISC licence text, through mbv's own `AssetSource`. Zed widgets ask for an icon by `IconName`, which loads `icons/<stem>.svg` from the asset source. mbv's asset source answers each `IconName` that an mbv view or a used Zed widget (for example the check and dash marks of `Checkbox`) needs with the matching Lucide SVG. mbv's own views name Lucide icons through `Icon::from_path`. Zed's own icon files do not ship. A unit test asserts that every `IconName` that `window/` names resolves in the asset source.

### D9. The playhead and the derived values run in the window

`window_state/playhead.rs` keeps the last anchor and the `Instant` when the window received it. The anchor crosses in process, so the receive time is close to the send time. The predicted position is the anchor's position plus the elapsed time, clamped to `[0, runtime]`, with no advance while paused.

The window does not repaint on every display frame. While the anchor plays, the player panel computes the time of the next painted change and sets one foreground timer to it: the next second boundary for the elapsed and remaining clocks, and the next pixel step of the scrubber fill and the row progress (from the bar width and the runtime). When the timer fires, the panel notifies only the views whose painted value changed and sets the next timer. A new anchor, a pause, or a resize replaces the timer. A minute timer moves the finish clocks at each wall-clock minute, also while paused. Paused playback with no input sets no playhead timer and paints nothing.

The derived values (elapsed, remaining, the finish clocks with the `+1d` suffix, each row's "Ends at", the active row's progress, the current Audiobooks chapter, and the next-change time) are pure functions in `window_state/derived.rs`, with `fmt_duration_short` and `fmt_wall_clock` from `mbv-ui-model`.

### D10. The cutover happens inside the change

The GPUI window is built in the same branch, in the `mbv-gui` crate, beside the Tauri code. While the port runs, the crate has a second binary target, `mbv-gui-gpui`, that starts the GPUI window over the same core thread. The Tauri binary still builds, but its window does not paint on this branch (Context). Each live check therefore compares the GPUI window with the last released Tauri package. The comparison runs one after the other, not side by side, because the build-id handshake does not let the released GUI attach to the branch's Owner.

When every surface is ported, the last task group deletes Tauri, `ui/`, the Tauri config, the icon set, `build.rs`, and the pnpm and Node steps, and makes `mbv-gui` the GPUI binary. `mbv-gui-gpui` goes away in the same commit.

A Vitest test moves to Rust only when it owns a contract that no Rust test covers already. Tests of the Library and Settings state become unit tests in `window_state/`. Tests that only checked Svelte markup do not move. Until group 10 deletes `ui/`, `make ui-test` is not a gate on this branch.

### D11. The window frame and geometry

The window opens at 1280 × 800 with a minimum of 400 × 200 and the title "mbv", as `tauri.conf.json` sets today, with app id `io.github.slatkin.mbv-gui`. It asks the compositor for server-side decorations. When the compositor gives none, GPUI falls back to client-side decorations and draws no frame. In that case mbv draws a thin title strip that moves the window (`start_window_move`) and resize edges (`start_window_resize`), so the window can still be moved and resized. GTK drew this frame for the Tauri window.

### D12. The panel widths persist in a GUI window state file

The dragged widths (the Queue panel's column and the Wide list column) are window state in `window_state/widths.rs`. They persist in a GUI window state file in mbv's state directory, through `mbv_config::load_gui_window_state` and `save_gui_window_state`, beside the GUI launch-state file. The window reads the file once before it opens and writes it when a drag ends and when a double-click resets a divider. A missing or unreadable file gives the defaults. The window thread writes the file, because the widths are window state and the core does not hold them.

Reset UI State reaches the window through the `panel` section's UI-reset counter. When the counter changes, the window sets the default widths and saves them, which meets "clear the GUI's saved … widths".

The widths that the Tauri window kept in WebKit's `localStorage` do not carry over. Each user's widths reset once, at the upgrade.

## Risks / Trade-offs

- [A Zed bump breaks call sites] → One pin and one bump commit at a time, with the probe checks of D1. `mbv-gui` uses a small part of `ui`, and the replacement path of D7 removes a widget that keeps breaking.
- [GPUI on Wayland with the NVIDIA driver] → The user's machine is the test target. Live checks on the user's machine come before the Tauri deletion task, and only the user runs them. wgpu falls back to GL through EGL when Vulkan fails.
- [No working GUI to compare with on the branch] → The live checks compare with the last released package (D10). The two branches merge together, so `main` keeps a working Tauri GUI until then.
- [Wayland only] → A session without Wayland cannot open the window. The user's machines run Wayland.
- [A compositor with no server-side decorations] → mbv's title strip and resize edges (D11).
- [IME and text input in mbv's own field] → The field uses GPUI's `EntityInputHandler`, which is the path Zed's editor uses for the same Wayland text-input protocol.
- [The binary becomes GPL] → Accepted. Only `mbv-gui` changes licence, and the release notes say so.
- [The workspace patches affect every crate] → Today no other mbv crate depends on `async-task`, `calloop`, or `khronos-egl`. Each bump commit checks this again.

## Migration Plan

`gui-change-driven-sections` is trimmed first, as a commit on its own branch. This change's branch stacks on it, and the two merge together. Users of a release before the merge keep the Tauri GUI. The first release after it ships the GPUI binary under the same name, app id, and desktop entry, so a package upgrade swaps the front end with no user step. The packages drop WebKitGTK 4.1 and add the Vulkan loader and EGL. The dragged panel widths reset once (D12). The GUI launch location, the configuration, and the TUI state do not change.

Rollback is a revert of the merge. The Owner protocol and the configuration do not change, so the Tauri GUI from the previous release works against the same Owner and files. It ignores the new GUI window state file.
