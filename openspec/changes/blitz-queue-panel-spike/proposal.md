# Proposal

## Why

The GUI Client is a Tauri 2 program. Its window is a WebKitGTK webview that runs a Svelte front end. A measured idle window with playback used about 505 MB PSS across the GUI processes: 248 MB in `mbv-gui`, 222 MB in `WebKitWebProcess`, and 35 MB in the network process. A heaptrack run showed a peak heap of 59.5 MB, and about 8 MB of that is mbv's own Rust code. The webview is most of the cost.

The user rejected the GPUI plan (`replace-tauri-with-gpui`). This change tests the other path: Blitz, the HTML and CSS engine behind Dioxus Native. Blitz keeps the HTML and CSS model the GUI already uses, and it runs in the same process with no web engine. It does not run JavaScript.

Before the team commits to a full replacement, this change builds one panel. It measures memory, threads, and CPU against the WebKit GUI, and it lists the Blitz gaps the Queue panel hits. Its result is a go or no-go gate for a later full replacement change.

## What Changes

- A new binary `mbv-native` in a new workspace member `crates/mbv-native`, built on `dioxus-native` 0.8.0-alpha.3 with Blitz 0.3.0-beta.4. It is not a default member, as `mbv-gui` is not. It paints one panel, the Queue panel: the queue list, the scrubber, the playback artwork, and the playhead that advances between anchors.
- The binary attaches to the Owner as a Client, the same way the GUI Client does. It declares itself with `declare_gui_client()`, so it does not take part in Pin swap. It never runs playback. Its window kind is the **Blitz Client** (a new term in `CONTEXT.md`), not a second GUI Client.
- The binary reuses the GUI core in process: the core thread, the attach campaign, and the section ledger. Section batches move through an in-process channel. No JSON, serde wire, or IPC is in the path.
- Artwork comes in process. The window asks for an `mbv-art` URL, and a custom Blitz net provider sends the request to the core. The core answers with bytes from its image cache, which the provider scales down before Blitz decodes them. Fonts and the Queue styles come from the same provider. No HTTP and no `data:` URL are used.
- The playhead advances in the window from the playhead anchor. The window writes its position at 4 Hz while playing.
- The change keeps a Blitz gap list for the Queue panel. Each gap cites a pinned source or an upstream issue. Each gap that needs an upstream fix gets a draft report. The user approves each draft before anyone posts it.
- A Cargo feature gate on `mbv-gui`, which makes Tauri optional. The user approved it. The gate changes `mbv-gui`'s manifest and its `build.rs` only. Behavior does not change.
- The GUI's Tauri window, its commands, and its front end do not change.

## Capabilities

### New Capabilities

- `blitz-client-window`: the Blitz Client, a window Client of the Owner that paints the Queue panel from the core's sections in process. It holds no playback state, paints no panel other than the Queue panel, and serves artwork in process.

### Modified Capabilities

None. The `gui-client` requirements do not change. The spike does not replace the GUI Client, and its requirements stay in their own capability.

## Impact

- **Prerequisite:** the GUI core APIs this change uses are on `main`. PR #1001 (`72e525307`) put them there. `feat/gui-change-driven-sections` holds 66 later commits that `main` lacks (performance and docs). The spike does not need those commits. Task 1.1 checks the APIs.
- **Code:** a new crate `crates/mbv-native`. A Cargo feature gate in `crates/mbv-gui/Cargo.toml` and `crates/mbv-gui/build.rs`.
- **Dependencies:** `dioxus-native` 0.8.0-alpha.3 (Blitz 0.3.0-beta.4) and the exact-pinned direct dependencies in `design.md` (D4). The lock changes as recorded in D4: `futures-*` moves from 0.3.32 to 0.3.34, and two patch releases move. No repository entry downgrades.
- **Tokio:** `dioxus-native` brings tokio through a non-optional dependency. The repository already locks tokio 1.54.1. The edge is recorded in `design.md` (D13) and in `AGENTS.md` (task 2.5).
- **Licenses:** the spike is MIT. Blitz pulls Stylo and other Servo crates under MPL-2.0, which requires source availability when the binary is distributed. The spike is not distributed.
- **Not in this change:** the full GUI replacement, an ADR, packaging, the live measurement run, and posting upstream reports. Each of those needs the user's approval.
