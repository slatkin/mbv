# Proposal

## Why

The GUI Client is a Tauri 2 program. Its window is a WebKitGTK webview that runs a Svelte front end. A measured idle window with playback used about 505 MB PSS across the GUI processes: 248 MB in `mbv-gui`, 222 MB in `WebKitWebProcess`, and 35 MB in the network process. A heaptrack run showed a peak heap of 59.5 MB, and about 8 MB of that is mbv's own Rust code. The webview is most of the cost.

The user rejected the GPUI plan (`replace-tauri-with-gpui`). This change tests the other path: Blitz, the HTML and CSS engine behind Dioxus Native. Blitz keeps the HTML and CSS model the GUI already uses, and it runs in the same process with no web engine. It does not run JavaScript.

Before the team commits to a full replacement, this change builds one panel. It measures memory, threads, and CPU against the WebKit GUI, and it lists the Blitz gaps the Queue panel hits. Its result is a go or no-go gate for a later full replacement change.

## What Changes

- A new binary `mbv-native` in a new workspace member `crates/mbv-native`. It is not a default member, as `mbv-gui` is not. It paints one panel: the Queue panel. The panel has the queue list, the scrubber, the playback artwork, and the playhead that advances between anchors.
- The binary attaches to the Owner as a Client, the same way the GUI Client does. It declares itself with `declare_gui_client()`, so it does not take part in Pin swap. It never runs playback.
- The binary reuses the GUI's core thread, attach campaign, and section ledger in process. Section batches move through an in-process channel. No JSON, serde wire, or IPC is in the path. The window still receives the section structs that `mbv-gui` builds.
- Artwork comes in process. The window requests an `mbv-art` URL. A custom Blitz net provider sends the request to the core, and the core answers with bytes from its image cache. No HTTP and no `data:` URL are used.
- The playhead advances in the window from the playhead anchor. The window repaints only when a painted value changes.
- **Gap-finding output.** The change keeps a list of Blitz gaps the Queue panel reaches. Each gap is backed by a source reference or an upstream issue. Gaps that need an upstream fix get a draft issue report. The user approves each draft before anyone posts it.
- **One change to `mbv-gui`:** a Cargo feature gate that makes Tauri optional, so the spike can use the GUI's library target without building WebKitGTK. Behavior of `mbv-gui` does not change. This is a proposal for the user to approve (see Open Questions).
- The GUI's Tauri window, its commands, and its front end do not change.

## Capabilities

### New Capabilities

- `native-client-window`: a native window Client of the Owner that paints the Queue panel from the core's sections in process. It holds no playback state and paints no panel other than the Queue panel. It serves artwork in process.

### Modified Capabilities

None. The `gui-client` requirements do not change in this change. The spike does not replace the GUI Client.

## Impact

- **Prerequisite:** `gui-change-driven-sections` is merged to `main`. Its Rust core lives on `feat/gui-change-driven-sections` (65 commits ahead of `main` on 2026-11-13). The spike cannot start from `main` until that branch merges.
- **Code:** a new crate `crates/mbv-native`. A Cargo feature gate in `crates/mbv-gui/Cargo.toml` and `crates/mbv-gui/src/main.rs` (Open Question 1 in `design.md`).
- **Dependencies:** `dioxus-native` 0.7.10 and a small set of exact-pinned direct dependencies. The versions and features are in `design.md`. The build also pulls `tokio` and the GPU stack through `dioxus-native`. `tokio` is an edge exception and is recorded in `design.md`.
- **Licenses:** the spike is MIT. Blitz pulls Stylo and other Servo crates under MPL-2.0. MPL requires source availability when the binary is distributed. The spike is not distributed.
- **Not in this change:** the full GUI replacement, an ADR, a `mbv-gui` behavior change, packaging, and the live measurement run. The measurement run needs the user's approval because it runs against the user's Owner.
