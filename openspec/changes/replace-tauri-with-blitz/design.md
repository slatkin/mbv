# Design

## Context

The GUI Client is the `mbv-gui` crate. Its Rust half is the core thread (`core_thread`, `attach`, `sections`, `art`, `leaves`, `painted_choices`) and the bridge DTOs (`bridge/`). Its window half is `crates/mbv-gui/ui/`: 73 Svelte components and 55 TypeScript modules, about 22,100 lines without tests, built by Vite with Tailwind v4 and loaded into a Tauri 2.12.5 WebKitGTK webview. `main.rs` registers six Tauri commands (`key`, `pointer`, `report_layout`, `library`, `settings`, `sync`), the `mbv://sections` event, and the `mbv-art` URI scheme.

The front end owns real state, not only paint. ADR 0036 puts the Library panel's cursor, hover, scroll, open nodes, focus, filter text, Hero overlay, and item menu in the window content, and the window content takes the Library panel's keys. ADR 0037 does the same for the Settings screen. ADR 0038 makes the window predict the playhead and derive every clock. All three decisions stay. This change moves the code that holds them from TypeScript to Rust.

The Blitz spike (`blitz-queue-panel-spike`, branch `feat/blitz-queue-panel-spike`, not merged) proved the stack: `dioxus-native` 0.8.0-alpha.3 on Blitz 0.3.0-beta.4 with the `vello-hybrid` renderer, a copied launch body with a custom net provider, `mbv-art` and font loads in process, and a section batch from another thread that repaints after one poll. The user ran the fixture window and accepted the result. The spike's code is a source for this change. Its commits are not merged.

Facts checked on 2026-11-13 in the pinned crate sources:

- `dioxus-native-dom` 0.8.0-alpha.3 converts pointer, mouse, click, context-menu, double-click, wheel, scroll, focus, blur, key, and input events. It panics (`unimplemented!`) on resize, visible, drag, transition, animation, clipboard, composition, selection, and toggle data. Event handlers honour `prevent_default` and `stop_propagation`.
- `MountedData` supports `get_client_rect`, `get_scroll_offset`, `get_scroll_size`, `scroll_to` (scroll into view), `scroll`, and `set_focus`.
- `blitz-dom` 0.3.0-beta.4 has text input, focus, IME, and scrolling. It has no `text-overflow` painting (gap G1, issue #888). Its `NetHandler` has no error callback (G8). Its font context is `pub(crate)`. It exports `decode_font_bytes`, which decodes WOFF2.
- `blitz-dom` uses `parley` 0.12 (locked 0.12.0) and `wuff` 0.2.
- The GUI front end uses no `bits-ui`, no drag-and-drop events, no `IntersectionObserver`, and no clipboard. It uses `ResizeObserver` in 10 Svelte files, `requestAnimationFrame` in the playhead, `setTimeout` for the search debounce, the art retry, and the minute clock, and `localStorage` for the dragged panel widths. It imports 36 Lucide icons (multi-line `import { … } from '@lucide/svelte'` blocks included).
- The latest Tailwind CSS release is v4.3.5. The front end locks 4.3.4.

## Goals / Non-Goals

**Goals:**

- One GUI Client program, `mbv-gui`, on Dioxus Native and Blitz, with no webview and no Node build.
- The same look: same markup structure, same Tailwind classes, same fonts, same icons, same theme roles.
- The same behaviour: every `gui-client` and `gui-settings-screen` requirement still holds, except the deltas in this change.
- No wire between the window and the core: Rust values cross an in-process channel.

**Non-Goals:**

- Memory, thread, or CPU measurements.
- Changes to the core thread's behaviour, the sections, the marks, or the TUI Client.
- A CPU renderer, or a Wayland-only or X11-only build.
- Patching Blitz, Dioxus, or Stylo, or posting upstream reports.

## Decisions

### D1. Stack and versions

`dioxus-native` 0.8.0-alpha.3 with Blitz 0.3.0-beta.4. These are the newest releases. The newest stable releases (`dioxus-native` 0.7.10, `blitz-dom` 0.2.4) are older and are not used, for two reasons: the user chose the 0.8 line as an early adopter, and 0.7.10 returns `NotSupported` from `get_client_rect`, which the layout reports need (D9).

Renderer: `vello-hybrid` (GPU through wgpu, Vulkan or GL), as the spike ran. winit 0.31.0-beta.3 keeps its default Wayland and X11 backends.

### D2. Dependencies (exact)

In `crates/mbv-gui/Cargo.toml`:

- `dioxus-native = { version = "=0.8.0-alpha.3", default-features = false, features = ["prelude", "svg", "system-fonts", "woff", "vello-hybrid"] }`.
- `blitz-shell = { version = "=0.3.0-beta.4", default-features = false }`.
- `blitz-traits = { version = "=0.3.0-beta.4", default-features = false }`.
- `blitz-dom = { version = "=0.3.0-beta.4", default-features = false, features = ["woff"] }`: `decode_font_bytes` for the ellipsis fonts (D12), and the `Document` trait in the headless tests.
- `parley = { version = "=0.12.0", default-features = false, features = ["std"] }`: the ellipsis measure (D12). The same version as Blitz, so Cargo unifies one copy.
- `futures-channel = "=0.3.34"` and `futures-util = { version = "=0.3.34", default-features = false, features = ["std"] }`: the window channel (D4).
- `image` (workspace, 0.25.10) with `png`, `jpeg`, and `webp`: art headers for the natural size (D7).
- `mimalloc` (workspace): unchanged.

Removed at the switch (D18): `tauri` 2.12.5, `tauri-build`, `serde_json` if nothing else reads it, and the Node packages of `ui/package.json`.

Peer ranges: `dioxus-native` requires `blitz-dom`, `blitz-shell`, and `blitz-traits` at `=0.3.0-beta.4`. `blitz-shell` requires `winit` `=0.31.0-beta.3`. `blitz-dom` requires `parley` `^0.12` and `image` `^0.25.6`. The spike measured the lock effect against `main`: `futures-*` 0.3.32 to 0.3.34 (seven crates), `bytemuck` 1.25.0 to 1.25.2, and `smallvec` 1.16.1 to 1.16.5. No entry downgrades. If the lock moves any other existing entry's version, the implementing agent stops and reports it.

Tokio: `dioxus-native` enables `tokio` 1.54.1 (`rt`) through `dioxus-asset-resolver`'s `native` feature, with no switch. `mbv-gui` is already in the tokio edge list (Tauri's runtime). The entry in `AGENTS.md` changes its reason from Tauri's runtime to `dioxus-native`'s asset resolver. No mbv code uses a tokio API.

### D3. Crate layout during the port

The window code lives in `crates/mbv-gui/src/window/`, one Rust module for each Svelte component or TypeScript module (D15 gives the map). The crate keeps one library and has two binaries while the port runs:

- `[features] default = ["tauri", "blitz"]`, `tauri = ["dep:tauri"]`, `blitz = [<the D2 window dependencies>]`.
- `[[bin]] name = "mbv-gui"`, `path = "src/main.rs"`, `required-features = ["tauri"]`: the Tauri program, unchanged.
- `[[bin]] name = "mbv-gui-blitz"`, `path = "src/bin/mbv_gui_blitz.rs"`, `required-features = ["blitz"]`: the new program.
- `build.rs` returns early unless `CARGO_FEATURE_TAURI` is set.
- `src/window.rs` is `#[cfg(feature = "blitz")]`.

Both features are default, so `cargo clippy --workspace --all-targets` and `cargo nextest run -p mbv-gui` compile and test both halves with no extra flags. At the switch (D18), the features, the Tauri binary, and `build.rs` go, `src/bin/mbv_gui_blitz.rs` becomes `src/main.rs`, and the binary is `mbv-gui`.

### D4. Process shape

`main` installs the app log as today (`mbv-gui.log`), then calls `core_thread::spawn(sink, quit)` as the Tauri `setup` does.

- **Sink:** `Box<dyn Fn(GuiSectionBatch) + Send>` that calls `unbounded_send(WindowEvent::Batch(batch))` on the window channel (`futures_channel::mpsc::UnboundedSender<WindowEvent>`). The batch is the Rust value. No serde runs.
- **Quit:** sends `WindowEvent::Quit`. The root component answers it by closing the window through the Dioxus window handle. This replaces `window.close()`.
- **Inputs:** the window holds the core's `Sender<CoreInput>` in a context value (`CoreLink`). Every former Tauri command is a method on it: `key(KeyEvent)`, `pointer(PointerEvent)`, `layout(LayoutReport)`, `library(LibraryCommand)`, `settings(SettingsCommand)`, `sync()`. A failed send logs and drops, as `push` does today.
- **Close:** when the event loop ends, `main` sends `CoreInput::Close` and joins the core thread, as today.

`WindowEvent` is one enum for everything that reaches the window from outside the Dioxus tree: `Batch`, `Quit`, `Art(ArtEvent)` (D7), `Timer(TimerId)` (D10), `Frame` (D10), and `Viewport` (D9). One `use_future` in the root drains the receiver and dispatches.

### D5. The launch copy

`window/launch.rs` copies the body of `launch_cfg_with_props` from `dioxus-native` 0.8.0-alpha.3 (about 100 lines; the spike's `launch.rs` is the source). It changes four things:

1. It installs `MbvNetProvider` (D7) as the document's net provider, and `navigation_provider: None`.
2. It wraps the application's window event handler: on `Resized` and `ScaleFactorChanged` it sends `WindowEvent::Viewport` after the shell handles the event (D9), and on `RedrawRequested` it sends `WindowEvent::Frame` while a frame is wanted (D10).
3. Window attributes: title `mbv`, the Wayland app id `io.github.slatkin.mbv-gui` (the GUI's current id, so the desktop entry still matches), set through `WindowAttributesWayland::with_name`.
4. It creates no tokio runtime.

Renderer options: `base_color`, `alpha_mode`, `features`, and `limits` are `None`, as in the spike. `font_ctx` is `None`.

### D6. Root, sections, and theme

`window/app.rs` is the port of `App.svelte` and `main.ts`. It mounts the stylesheet (D11), the theme style (below), and the attach, overlay, Library, Settings, and player trees exactly as `App.svelte` composes them. On first mount it calls `CoreLink::sync()`, as `App.svelte` calls `sync`.

`window/sections.rs` is the port of `lib/bridge/sections.ts`: one `Signal` for each section, a revision check that ignores a section at or below the held revision, and one write pass for a batch. All writes of one batch happen in one task poll, so Dioxus renders the batch once and no frame shows half a batch (ADR 0038).

`window/theme.rs` is the port of `lib/bridge/theme.ts`: it renders a `style` element whose `:root` rule sets each `--mbv-<name>` custom property from the theme section. `app.css`'s fallbacks stay, so the chrome paints before the first theme.

The TypeScript type mirrors (`lib/bridge/types.ts`, `library-types*.ts`, `settings-types.ts`, `theme-types.ts`, `art.ts`, `cells.ts`, `status.ts`) are not ported. The window reads the Rust types in `bridge/` directly.

### D7. Art, fonts, and the net provider

`window/net.rs` holds `MbvNetProvider` (`blitz_traits::net::NetProvider`). It holds the core's `Sender<CoreInput>` and the window channel's sender.

- `mbv-art://localhost/<path>`: sends `CoreInput::Art(ArtRequest { path, reply })`. On `Found { bytes, .. }` it reads the image header (`image::ImageReader::with_guessed_format().into_dimensions()`), sends `WindowEvent::Art(ArtEvent::Loaded { url, width, height })`, and calls `handler.bytes(url, bytes)` with the source bytes unchanged. On `Missing` it drops the handler, so the node stays unloaded, and sends `WindowEvent::Art(ArtEvent::Failed { url })`.
- `mbv-asset://localhost/font/<file>`: answers the six bundled WOFF2 files (D11) from `include_bytes!`.
- Any other URL: dropped with a debug log.

`window/art_image.rs` is the port of `ArtImage.svelte`. The webview's `onerror` becomes `ArtEvent::Failed` for the painted URL, and the image's natural size (`capToNaturalSize`, `lib/bridge/natural-size.ts`) comes from `ArtEvent::Loaded`. The retry keeps its rules: the fill paints, the wait starts at 1 s and doubles to a 30 s cap (D10's timer), and when it ends the `img` element is recreated with the same `src`, so Blitz requests it again. A new URL drops the failure and its timer. Invariant 27 stays true: a missing key paints the loading fill and asks the core again later.

The `ArtEvent` route is the only way the window learns of a failure, because `NetHandler` has no `error` (G8).

### D8. Keys and pointer

`window/input.rs` is the port of `lib/bridge/input.ts`.

- **Keys:** the root element has `tabindex="0"`, takes focus on mount, and has one `onkeydown` handler. The handler builds `DomKey` (the existing struct in `bridge/keys.rs`) from the Dioxus `KeyboardData`: `key` is the DOM key string (`evt.key().to_string()`), `code` is `evt.code().to_string()`, and the modifier flags come from `evt.modifiers()`. It then runs the installed key router (the Settings screen's steps, then the Library panel's steps, in the order of `input.ts`). The outcome is one of three, as in `input.ts`: `Core` calls `prevent_default` and sends `neutral_key(dom_key)`; `Handled` calls `prevent_default` and sends nothing; `Text` lets the focused `input` take the key and sends nothing.
- **Pointer:** the root has `onpointerdown`, `onpointerup`, `onpointermove`, `onwheel`, and `oncontextmenu` handlers. Each builds `DomPointer` (the existing struct in `bridge/pointer.rs`) from the Dioxus event's client coordinates, buttons, and modifiers and sends `neutral_pointer(dom_pointer)` when it is `Some`. `oncontextmenu` calls `prevent_default`. Front-end-owned surfaces (Library, Settings) take their pointer events in their own handlers and call `stop_propagation`, as the Svelte handlers do.
- The serde derives on `DomKey` and `DomPointer` go at the switch (D18). The structs stay as the translation's input.

No handler uses an event kind that `dioxus-native-dom` does not convert (resize, visible, drag, transition, animation, clipboard, composition, selection, toggle). `rg` for those handler names in `src/window` is part of the switch task's verification, because a use panics at run time.

### D9. Layout reports and container sizes

`ResizeObserver` has no equivalent. `window/layout.rs` and `window/resize.rs` are the ports of `lib/bridge/layout.ts` and `lib/bridge/resize.ts`, with the measurement replaced:

- A `LayoutEpoch` signal (a counter) increments on `WindowEvent::Viewport`, on every divider drag step and divider reset, on every panel state change (collapse, queue fold, Settings open or close), and on every section batch that changes a list's row count.
- Each container that `ResizeObserver` watched today (`App`, the four panel states, `QueuePanel`, `QueuePlaybackPanel`, `QueueFooter`, `TrackChips`, `LibraryVirtualList`, `AttachScreen`) keeps its `MountedData` from `onmounted`. A `use_effect` that reads `LayoutEpoch` calls `get_client_rect` on it and writes the size into the container's size signal. The effect runs after the render, so the rect is the laid-out one.
- The layout report keeps the rules of `layout.ts`: it is built from the same measured rects and sent through `CoreLink::layout`. A report that equals the last one sends nothing.

The panel state choice (`lib/bridge/panel.ts`, `arrangement.ts`) reads the container size signal, as it reads the observed size today.

### D10. Clocks and frames

`window/clock.rs` replaces `setTimeout` and `requestAnimationFrame`.

- **Timers:** one std thread holds a deadline queue. `Clock::after(duration) -> TimerId` and `Clock::cancel(TimerId)` add and drop deadlines. A passed deadline sends `WindowEvent::Timer(id)`. The search debounce (200 ms), the art retry (D7), and the minute clock of the playhead use it. Tests inject a recording clock, so no test sleeps (as the TypeScript seams do today).
- **Frames:** `Clock::want_frames(bool)`. While frames are wanted, the launch copy's `RedrawRequested` hook (D5) sends `WindowEvent::Frame` and requests the next redraw. This is `requestAnimationFrame`: it follows the presented frames, which `vello-hybrid` paces at vsync. The playhead (`window/playhead.rs`, the port of `lib/bridge/playhead.ts`) wants frames only while its anchor is active and not paused, as `pump` does today, and writes its `now` signal on each frame.

### D11. The stylesheet

The class strings in `rsx!` are the Svelte class strings, character for character. Conditional classes keep their conditions.

- `crates/mbv-gui/assets/app.css` is the port of `ui/src/app.css`. Its three imports change: `@import "tailwindcss"` stays; `tw-animate-css` 1.4.0 and `shadcn-svelte/tailwind.css` 1.8.0 are copied from the installed packages into `assets/vendor/` (MIT) and imported by relative path. It adds `@source "../src/window";`, so Tailwind scans the Rust sources, and the six `@font-face` rules that the `@fontsource` CSS gave, with `src: url('mbv-asset://localhost/font/<file>') format('woff2')`.
- `crates/mbv-gui/assets/fonts/` holds the six Latin WOFF2 files of `@fontsource/ibm-plex-sans` and `@fontsource/jetbrains-mono` 5.3.0 (weights 400, 500, 600; SIL OFL 1.1), copied from the installed packages.
- The Tailwind v4.3.5 standalone CLI builds `assets/app.generated.css` from `assets/app.css`: `tailwindcss -i crates/mbv-gui/assets/app.css -o crates/mbv-gui/assets/app.generated.css --minify`, run as `make gui-css`. The output is committed. `window/app.rs` includes it with `include_str!` in one `style` element. A normal build runs no CSS tool.
- A class added to the Rust sources needs `make gui-css` before its commit. The tasks that add markup run it.

### D12. Ellipsis in mbv

`window/truncate.rs` provides `Truncate { text, class }`, used wherever the Svelte markup uses `truncate` on a text box.

- It renders the box with the Svelte classes and its current display text.
- It keeps the box's `MountedData`. A `use_effect` that reads the `text` prop and `LayoutEpoch` reads the box's content width with `get_client_rect`, and, when the full text does not fit, writes a cut text that ends in `…` (U+2026) and fits.
- The measure uses one process-wide `parley::FontContext` with the six bundled faces, registered once through `blitz_dom::decode_font_bytes`, and a `parley::LayoutContext`. The font family, size, weight, and letter spacing come from a `TextStyle` value that each call site passes. It names the same Tailwind values as the class string, for example `TextStyle::sans(13.0, 500)`. A binary search over grapheme boundaries finds the longest prefix whose width plus the ellipsis width fits.
- The `truncate` class stays in the class string, so the box still clips (`overflow: hidden`, `white-space: nowrap`), and a measure that is one frame late shows a clipped text, not an overflow.

### D13. Icons

`window/icons.rs` holds the 36 Lucide icons the front end imports (`ArrowLeft`, `AudioLines`, `Captions`, `Cast`, `Check`, `ChevronDown`, `ChevronRight`, `ChevronsRight`, `Clock`, `Ellipsis`, `GripVertical`, `Keyboard`, `Library`, `ListEnd`, `ListOrdered`, `ListPlus`, `LoaderCircle`, `Maximize2`, `Monitor`, `MonitorPlay`, `Pause`, `Play`, `Plus`, `RotateCcw`, `Rss`, `SaveCheck`, `SaveOff`, `Search`, `Server`, `Settings`, `Shuffle`, `SkipBack`, `SkipForward`, `SquarePlay`, `X`, `Zap`). Before the port, the implementing agent re-reads every `@lucide/svelte` import in `ui/src` (multi-line blocks included) and adds any icon this list misses. Each is a function that renders an inline `svg` with Lucide's default attributes (24 × 24 view box, `stroke="currentColor"`, `stroke-width="2"`, round caps and joins, no fill) and the child elements copied from `@lucide/svelte` 1.70.0. Each takes the `class` and `size` props that the Svelte call sites pass. Lucide is ISC-licensed; the notice goes into `assets/vendor/`.

### D14. Panel widths

`mbv-config` gains `load_gui_panel_widths() -> GuiPanelWidths` and `save_gui_panel_widths(&GuiPanelWidths)`, a TOML file `gui-panel-widths.toml` in the state directory, beside the GUI launch state. `GuiPanelWidths` holds the queue column width and the Wide list column width as `Option<u32>` CSS pixels. A missing or unreadable file gives `None` for both, and the defaults apply. The window reads it once at start. A drag end and a double-click reset write it, and Reset UI State (`lib/library/reset.ts`) writes the defaults, as `resetPanelWidths` does today. The webview's saved widths are not migrated.

### D15. The port map and its rules

Each TypeScript or Svelte file has one Rust module under `src/window/`, with the same name in snake case and the same directory: `lib/library/keys.ts` is `window/library/keys.rs`, `lib/components/QueuePanel.svelte` is `window/components/queue_panel.rs`, `lib/components/settings/SettingsSwitch.svelte` is `window/components/settings/settings_switch.rs`. A Rust module over 800 lines splits along a responsibility seam (`foo.rs` plus `foo/`). The type-mirror files listed in D6 have no module.

Rules for an accurate port:

- **Markup:** the element tree, attributes, ARIA roles, and class strings match the Svelte markup. `{#if}`, `{#each}`, and snippets become `if`, `for`, and child components.
- **State:** a Svelte `$state` is a `Signal`; a `$derived` is a `Memo` or a plain function of signals; a `$effect` is a `use_effect`; a store module (`writable`) is a context-provided struct of signals. Module-level state in TypeScript (for example `lib/library/state.ts`, `lib/settings/state.ts`) is one struct provided as context at the root.
- **Logic:** pure TypeScript functions become pure Rust functions with the same inputs and outputs. Their branches and constants keep the TypeScript values.
- **Commands:** each `sendLibraryCommand` or `sendSettingsCommand` call builds the neutral Rust command that `bridge/library.rs` or `bridge/settings.rs` makes from the wire DTO today, and sends it through `CoreLink`.
- **Scroll:** `scrollIntoView` is `MountedData::scroll_to` with the same block and inline positions. `scrollTop` writes are `MountedData::scroll`. `onscroll` handlers read `get_scroll_offset`.
- **Focus:** `.focus()` is `MountedData::set_focus(true)`.
- **Text fields:** an `<input bind:value>` is an `input` with a `value` attribute from a signal and an `oninput` that writes it.

### D16. Tests

A ported test owns a contract that still holds; tests of Svelte, Vite, Tauri, or webview mechanics are not ported (`docs/invariants/14-test-ownership.md`).

- Pure logic (the `lib/library/*`, `lib/settings/*`, and `lib/bridge/{playhead,time,panel,resize,natural-size,input}` modules) gets unit tests in `src/window/**/tests.rs`, one per contract the matching `.test.ts` owns. Named `#[case]` tables replace `it.each` tables where the outcomes differ.
- A component contract (for example "the playing row carries the progress fill", "a pill press sends the select command") gets a headless test: `DioxusDocument::initial_build` and `Document::poll` over a fixture section batch, as the spike's harness did, with a recording `CoreLink`. One test per contract that the component's `.test.ts` owns.
- No test opens a window, sleeps, or reaches an Owner or a network. Clocks are injected (D10).
- The Rust core-thread tests (`crates/mbv-gui/tests/core_thread*`) do not change. `tests/capabilities.rs` (the Tauri capability check) and the serde wire tests of `tests/bridge/` go at the switch. The translation tests of `tests/bridge/keys.rs` and `tests/bridge/pointer.rs` stay: they own the chord and gesture mapping, which the Dioxus path reuses (D8).

### D17. Licences and packaging

- The packages ship `LICENSES/` notices for the MPL-2.0 crates (Stylo and the Servo crates), the fonts (OFL 1.1), Lucide (ISC), and the vendored CSS (MIT). The MPL notice names the crate source on crates.io as the source offer for the unmodified MPL files.
- `PKGBUILD` and `PKGBUILD-git`: `depends` drops `webkit2gtk-4.1` and adds `vulkan-icd-loader` (`fontconfig` is already there). `PKGBUILD-git`'s `makedepends` drops `webkit2gtk-4.1`, `nodejs`, and `pnpm`.
- The `.deb` (`build.yml`): `Depends` drops `libwebkit2gtk-4.1-0` and adds `libvulkan1` and `libfontconfig1`. The CI check on the field changes to match.
- CI containers drop `webkit2gtk-4.1`, `nodejs`, and `pnpm`, and drop the `ui-*` steps.
- The Makefile drops the `ui-install`, `ui-build`, `ui-check`, `ui-lint`, and `ui-test` targets and the `UI_DIR` variable, and adds `gui-css` (D11).

### D18. The switch

The last phase deletes the Tauri path in one change set: `crates/mbv-gui/ui/`, `tauri.conf.json`, `capabilities/`, `permissions/`, `gen/`, `build.rs`, `src/main.rs` (Tauri), the `tauri` and `blitz` features, `tests/capabilities.rs`, the serde derives and wire tests that nothing else reads, and the Tauri and Node dependencies. `src/bin/mbv_gui_blitz.rs` moves to `src/main.rs`, and `window` loses its `cfg`. Then the docs change (D19).

### D19. Records

- A new ADR, `docs/adr/0039-gui-client-renders-with-dioxus-native-on-blitz.md`, status `proposed`: the decision (Dioxus Native on Blitz, in process, no webview), the considered options (keep Tauri, GPUI as rejected in `replace-tauri-with-gpui`, Blitz), and the consequences (no reload, the art failure route, the ellipsis in mbv, the alpha pin). It keeps ADRs 0036, 0037, and 0038 and replaces only their transport: "window content" is the Dioxus tree, the `mbv://sections` event is the window channel, and the CSP goes.
- `CONTEXT.md`: the **GUI Client** entry names Dioxus Native on Blitz in place of the webview.
- `docs/architecture/gui-client.md`: the process shape, the bridge contract (now the window channel and `CoreLink`), the layout reports (D9), the clocks (D10), the stylesheet (D11), the ellipsis (D12), and the Blitz gap list replace the Tauri, webview, and WebKitGTK sections.
- `AGENTS.md`: the `crates/mbv-gui/` map entry, the ADR 0036 note in the input map entry, and the tokio edge reason.
- `docs/recipes.md`, `docs/architecture/performance.md`, `docs/architecture/interactive-surface-ledger.md`, and the invariants that name the webview, `app.css` paths, or Svelte files (27 included) are updated to name the Rust modules.
- The `blitz-queue-panel-spike` change directory is deleted from `openspec/changes/` with this proposal.

### D20. Blitz gaps that the port meets

| # | Gap | Where | Handling |
| --- | --- | --- | --- |
| G1 | No `text-overflow: ellipsis` | every `truncate` | D12 |
| G3 | An animation repaints the whole window (#1497) | `animate-spin` on the attach screen, the switch and list `transition` classes | Accepted. They run only while shown. |
| G8 | No `NetHandler::error` | art misses | D7 |
| G9 | No net provider injection in the launch | art and fonts | D5 |
| G10 | Transform painting (#663) | the scrubber thumb's `translate` | Accepted as the spike showed it. |
| G13 | Wayland resize slowness and surface timeout (#293) | every resize | Accepted. |
| G14 | No GPU: panic at surface creation (#1505) | hosts with no Vulkan or GL | Accepted. The packages depend on the Vulkan loader. |
| G-Events | Resize, visible, drag, transition, and animation events panic | none | D8: no handler uses them. D9 replaces `ResizeObserver`. |
| G-Reload | No window-content reload | dev reload | The spec deltas drop the reload scenarios. |

## Risks / Trade-offs

- [Prerelease stack] → Every crate is pinned exactly, so an upstream release cannot move the build. The launch copy (D5) is the main upgrade cost, about 100 lines.
- [A Tailwind class the CLI did not see] → The class has no rule, so its look is missing. D11 has each markup task run `make gui-css`. The committed output shows the change in review.
- [Stylo and Tailwind v4 output] → Tailwind v4 emits `@layer`, `@property`, `oklch`, `color-mix`, and nesting-free output. Stylo is Firefox's style engine and parses these. A rule Blitz does not paint is a G-row, handled as the table above handles it.
- [The ellipsis measure differs from the painted text] → The measure uses the same faces and sizes as the class string. A one-frame lag shows clipping, not overflow. A `TextStyle` that does not match its class string shows as an early or late cut, which is a markup bug in that call site.
- [Full-window repaint at frame rate while playing] → D10 wants frames only while the anchor plays, as `requestAnimationFrame` did in the webview.
- [Saved panel widths reset once] → D14. The user accepted local breakage.
- [A missed `ResizeObserver` site] → D9 lists each container. A missed one shows as a stale layout after a resize, and the layout report tests cover the report's rules.

## Migration Plan

1. Delete the spike's change directory (with this proposal's commit).
2. Phase 1: the feature gate, the dependencies, the launch copy, the net provider, the clock, the stylesheet pipeline, the fonts, the icons, the ellipsis, the sections, the theme, and the input and layout layers.
3. Phase 2: the attach screen, the overlays, the player panel, the Queue panel, the Status bar, and the four panel states.
4. Phase 3: the Library panel and its destinations.
5. Phase 4: the Settings screen.
6. Phase 5: the switch (D18), packaging (D17), and records (D19).

The Tauri binary builds and runs through phases 1 to 4. Rollback before the switch is the removal of `src/window/`, the `blitz` binary, and the feature. After the switch, rollback is a revert of the switch commits.

## Open Questions

None. The user decided: the spike is accepted and not merged; Tauri, WebKitGTK, Svelte, and Node are deleted at the end and the Blitz program becomes `mbv-gui`; the CSS comes from the Tailwind standalone CLI with the GUI's class strings; and mbv draws the ellipsis itself.
