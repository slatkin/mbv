# Proposal

## Why

The GUI Client is a Tauri 2 program. Its window is a WebKitGTK webview that runs a Svelte front end. A profile of an idle window showed about 340 MB PSS across the GUI processes. About 140 MB of that is the `WebKitWebProcess`, and most of the rest is WebKit, JavaScriptCore, and GTK in `mbv-gui`. The Rust code that mbv owns keeps less than 20 MB live. Every image also exists in many copies: the core's image cache, a base64 string in the frame memo, the JSON event, the JS string, and WebKit's decoded bitmap. The webview is the cost. A memo or a smaller payload cannot remove it.

GPUI is the GPU UI framework of the Zed editor. It is Rust, it draws with wgpu on Vulkan (with a GL fallback through EGL), and on Linux it talks to Wayland directly. It has no web engine, no JavaScript, no GTK, and no second process. Zed ships from GPUI's `main` branch every week, so that branch is production code.

## What Changes

- **BREAKING (GUI front end):** the GUI Client's window is a GPUI window. The Svelte front end (`crates/mbv-gui/ui/`), Tauri, WebKitGTK, pnpm, Node, and the Vitest suite are removed in this change. `mbv-gui` stays one binary with the same name, the same Wayland app id (`io.github.slatkin.mbv-gui`), and the same desktop entry.
- **The core thread does not change shape.** It stays synchronous, it owns `App`, it runs `ClientCore::tick`, and it builds the section batches of `gui-change-driven-sections`. The batches go to the GPUI main thread through an in-process channel, not through an IPC event. Window input goes to the core channel as the same `CoreInput` values. The Tauri commands and the serde DTO wire go away.
- **Every surface is ported, and the behavior stays the same.** The player panel in its four states, the queue, the row menu, the popovers, the Status bar, the overlays, the attach screens, the GUI Library panel with its eight destinations and Search, and the Settings screen. The window still paints only what the Client core projects for the shared surfaces, forwards neutral key and pointer events, and reports its layout for the core's hit map (`client-interaction-core`). The GUI Library panel and the Settings screen still own their interaction state in the window (ADR 0036, ADR 0037). That state moves from TypeScript to Rust modules that do not depend on GPUI.
- **Artwork goes in process.** A section carries an art reference. The window asks the core for the bytes of a reference in process and keeps the decoded image in a bounded window cache. No base64, no `data:` URL, and no URI scheme.
- **The playhead advances in the window.** The window predicts the position between two playhead anchors from the anchor's `Instant`, and derives the elapsed, remaining, and finish times, the active row's progress, and the current chapter. It redraws only when a painted value changes.
- **The window keeps its look and feel.** The layout, the panel arrangement, the colours (the `mbv-theme` roles, the chrome fills, and the canvas tokens), the fonts, and the icons stay as they are now. Widget shapes (buttons, pills, menus, switches) can take Zed's form; they do not have to match the Svelte widgets.
- **Zed's `ui` and `theme` crates paint the common widgets.** mbv maps its palette roles and canvas tokens into a Zed `Theme`, so no widget shows Zed's colours. mbv adds its own single-line text field, because Zed's `ui_input` needs Zed's editor.
- **The GUI binary becomes GPL.** `crates/mbv-gui` is licensed `GPL-3.0-or-later`, because Zed's `ui` and `theme` crates are GPL. Every other crate stays MIT.
- **Wayland only.** GPUI is built with its Wayland backend and without X11.
- **Packaging.** The packages drop WebKitGTK 4.1 and declare the Vulkan loader, EGL, and fontconfig. The builds drop Node and pnpm.

## Prerequisite

This change builds on `gui-change-driven-sections`, trimmed to its Rust half: the section ledger, the batches, `sync`, the dirty marks and the audit, the playhead anchor and the time values that leave the sections (tasks 4.1 and 4.2), the Library list and detail split (tasks 6.1 to 6.3), and the projection fixes (tasks 7.1 and 7.2). Its front-end tasks (3, 4.3 to 4.5, 5.2, 5.4, 6.4 to 6.6, and 8) leave that change. Its art scheme (design D7, tasks 5.1 and 5.3) leaves it too: this change keeps the art reference and does the art path in process. Of its spec delta, that change keeps only the requirement "The window updates only the sections that changed". This change owns the requirements that depend on the window: the snapshot when the window opens, the playhead, the derived values, and the artwork. The trim is a commit on that change, and it also trims ADR 0038's art and mount decisions while the ADR is still proposed.

The trimmed Rust half breaks the Tauri window, because the Svelte code that reads the new section shapes is in the part that leaves. So the two changes do not merge one after the other. This change's branch stacks on the trimmed branch, and both merge together. `main` keeps a working Tauri GUI until then.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the program has no webview stack, and its packages declare the GPUI runtime dependencies. No requirement names a window-content reload any more. The fonts ship inside the program. The canvas tokens live in the GUI program, not in `app.css`. New requirements cover the snapshot when the window opens, the playhead that advances in the window, and the values the window derives. "Library art comes through the frame" becomes "Artwork comes from the core's image cache".
- `gui-settings-screen`: "The window content owns the screen's interaction state" loses its reload rule and its reload scenario.

## Impact

- `crates/mbv-gui/`: a new GPUI window module tree. The bridge DTOs lose serde. `bridge/keys.rs` and `bridge/pointer.rs` translate GPUI events. `main.rs` starts GPUI, not Tauri. `ui/`, `tauri.conf.json`, `capabilities/`, `build.rs`, and the Tauri icon set are deleted. Bundled fonts (IBM Plex Sans, JetBrains Mono) and Lucide SVG icons move into the crate.
- `src/app/`: two read-only queries for the art path, keyed by the image cache key and by `TrickplaySheetKey`.
- `mbv-config`: a GUI window state file for the dragged panel widths, beside the GUI launch-state file. The widths that the Tauri window kept in WebKit's `localStorage` reset once at the upgrade.
- Dependencies: `gpui`, `gpui_platform`, `ui`, and `theme` from one pinned Zed git revision. Three `[patch.crates-io]` entries copied from Zed's workspace (`async-task`, `calloop`, `khronos-egl`). `flume` (already in the workspace) for the batch channel. `tauri`, `tauri-build`, `base64`, and `serde` leave `mbv-gui`.
- Build and release: the `Makefile` `ui-*` targets and `make dev`, the CI jobs, `PKGBUILD`, `PKGBUILD-git`, and the `.deb` dependency list (WebKitGTK out; the Vulkan loader and EGL in; fontconfig stays).
- Docs: `docs/architecture/gui-client.md` (rewritten for the GPUI window), `AGENTS.md` (the `mbv-gui` repository map entry, and the tokio rule, which no longer names `mbv-gui`), `CONTEXT.md` (the *GUI Client* entry says GPUI), a new ADR 0039 that records GPUI and amends the webview rationale and the reload consequences of ADR 0036 and ADR 0037, and `docs/architecture/interactive-surface-ledger.md` (the GUI Library panel row).
- Open work that this change makes moot: PR 999 (`chore/add-ui-prettier`).
