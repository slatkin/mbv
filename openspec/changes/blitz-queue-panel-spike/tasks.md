# Tasks

Precondition: `feat/gui-change-driven-sections` is merged to `main`. Do not start before that.

## 1. Gate Tauri in mbv-gui

- [ ] 1.1 Confirm the GUI core is on `main`: `git merge-base --is-ancestor feat/gui-change-driven-sections main` exits 0, and `crates/mbv-gui/src/core_thread.rs` exists on `main`. Verify: the command exits 0.
- [ ] 1.2 Make Tauri optional in `crates/mbv-gui/Cargo.toml` (D5): `tauri` and `tauri-build` become optional behind a default feature `tauri`. Set `required-features = ["tauri"]` on the `mbv-gui` binary and on every test that uses Tauri. Gate the Tauri glue in `src/main.rs` under the same feature. Verify: `cargo tree -p mbv-gui --no-default-features -i tauri` reports no path, and `cargo check -p mbv-gui --no-default-features --lib` passes.
- [ ] 1.3 Confirm `mbv-gui` behavior is unchanged. Verify: `cargo nextest run -p mbv-gui` passes with the default features, and the same tests that ran before this change still run.

## 2. Create the mbv-native crate

- [ ] 2.1 Add `crates/mbv-native` (package and binary `mbv-native`) to the workspace `members`. Do not add it to `default-members`. Set `[lints] workspace = true`. Add the direct dependencies from D3 and D4 with the exact versions named there. Verify: `cargo check -p mbv-native` passes.
- [ ] 2.2 Add the `profiling` feature. It drops the `#[global_allocator]` (D16). Verify: `cargo check -p mbv-native --features profiling` passes.
- [ ] 2.3 Check the dependency graph for duplicates of Blitz or winit. Verify: `cargo tree -p mbv-native -d` shows no duplicate `blitz-*`, `winit`, or `dioxus-*` crate.
- [ ] 2.4 Add the tokio edge note (D13) and the MPL-2.0 note (design Risks) as comments in `crates/mbv-native/Cargo.toml`. Verify: `rg -n 'tokio|MPL' crates/mbv-native/Cargo.toml` shows both notes.

## 3. In-process artwork

- [ ] 3.1 Implement `MbvArtNetProvider` in `crates/mbv-native/src/art_net.rs` (D6). It implements `NetProvider<Resource>`. It takes `mbv-art` requests, sends `CoreInput::Art(ArtRequest { path, reply })` through the core's sender, and answers `handler.bytes(doc_id, Bytes, callback)` from the reply. A non-`mbv-art` URL logs and returns. Verify: `cargo check -p mbv-native` passes.
- [ ] 3.2 In the reply's `Found` arm, decode the source bytes, downscale to at most 512 px on the long edge, and encode PNG (D6). `Missing` answers with `QUEUE_CARD_PLACEHOLDER_BYTES`. Verify: `cargo check -p mbv-native` passes.
- [ ] 3.3 Test contract 1 (D17) in `src/art_net.rs`. Use a fake reply, a fake `NetHandler`, and a fake `NetCallback`. The test checks that `Found` reaches the handler with PNG bytes that decode, and that `Missing` reaches it with the placeholder. Verify: `cargo nextest run -p mbv-native` passes.

## 4. Custom launch and core wiring

- [ ] 4.1 Add `crates/mbv-native/src/launch.rs`. It copies the net-less branch of `launch_cfg_with_props` from `dioxus-native` 0.7.10, using `MbvArtNetProvider` as `DocumentConfig.net_provider` and as the root context (D6). It creates no tokio runtime. The window title is `mbv`, and the Wayland app id is `io.github.slatkin.mbv-native` (D14). Verify: `cargo check -p mbv-native` passes, and `rg -n 'tokio::runtime' crates/mbv-native/src` returns no match.
- [ ] 4.2 In `src/main.rs`, call `declare_gui_client()` first (D15). Start `mbv_gui::core_thread::spawn` with a sink that sends each batch into a `futures_channel::mpsc::unbounded` sender (D7). Keep the core's `Sender<CoreInput>`. On window close, send `CoreInput::Close` and join the core handle. Verify: `cargo check -p mbv-native` passes, and `rg -n 'declare_gui_client' crates/mbv-native/src` finds one call.

## 5. Queue panel

- [ ] 5.1 Add the Dioxus root in `crates/mbv-native/src/app.rs`. A `use_future` drains the batch receiver and writes each section into its signal (D7). Verify: `cargo check -p mbv-native` passes.
- [ ] 5.2 Build the queue list view from the `queue` section: viewport rows, the active marker, and the active row's played fraction (D9). Verify: `cargo check -p mbv-native` passes.
- [ ] 5.3 Build the scrubber from the `playback` and `playhead` sections: played fill, chapter ticks, intro and credits marks, elapsed and remaining clocks (D9). Verify: `cargo check -p mbv-native` passes.
- [ ] 5.4 Build the playback artwork from `GuiArtRef`. An `image` ref uses `art_url(card_path(key))`. A `loading` ref paints no image (D6). Verify: `cargo check -p mbv-native` passes.
- [ ] 5.5 Add the playhead tick (D8): a std thread sends a tick at 4 Hz while the anchor is active and not paused. The view writes a signal only when the quantized value changes. Test contract 2 (D17) with the quantizer function, not the timer. Verify: `cargo nextest run -p mbv-native` passes.
- [ ] 5.6 Add `crates/mbv-native/assets/queue.css` with plain CSS (D11). Load the vendored woff2 files with `@font-face` (D12). Copy the woff2 files for IBM Plex Sans and JetBrains Mono, Latin subset, weights 400, 500, and 600, from `@fontsource` 5.3.0 into `crates/mbv-native/assets/fonts/`. Verify: `fd -e woff2 . crates/mbv-native/assets/fonts` lists six files.

## 6. Records, checks, and upstream drafts

- [ ] 6.1 Write `openspec/changes/blitz-queue-panel-spike/measurements.md`. It holds the D16 protocol as commands only, a blank table for the GUI thread-count baseline, the go and no-go table with the bounds from design "Go / No-Go", and a blank results section. Do not run the protocol. Verify: the file names every command in D16.
- [ ] 6.2 Draft upstream reports in `openspec/changes/blitz-queue-panel-spike/upstream/` for each gap marked "confirmed in source" in the design (G5, G6, G8, G9). Each draft has a minimal HTML and CSS repro where one applies, and a line that records which existing issues and WPT tests were searched first. Do not post anything. Verify: four files exist in `upstream/`, and each names the searched issues and WPT tests.
- [ ] 6.3 Run `cargo fmt --all -- --check`, `cargo clippy -p mbv-native -p mbv-gui --all-targets -- -D warnings`, and `cargo nextest run -p mbv-native -p mbv-gui`. Fix each failure the change causes. Verify: all three commands pass.
- [ ] 6.4 Before the push, run `make check-code-file-lines`. Verify: the command reports no file over 800 lines.

## Workflow follow-up

- The live measurement run in `measurements.md` needs the user's approval. Do not run it as part of these tasks.
- Posting the upstream drafts needs the user's approval, one draft at a time.
