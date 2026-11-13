# Tasks

Each task names how to verify it. Verification uses commands, tests, or delivered artifacts. Design decisions are cited as `D<n>`.

## 1. Precondition and gate

- [x] 1.1 Confirm the GUI core APIs are on `main`: `crates/mbv-gui/src/core_thread.rs` defines `spawn`, `crates/mbv-gui/src/art.rs` defines `ArtRequest` and `ArtResponse`, and `src/owner_link.rs` defines `declare_gui_client`. Verify: `rg -n 'pub fn spawn|pub struct ArtRequest|pub enum ArtResponse' crates/mbv-gui/src` and `rg -n 'pub fn declare_gui_client' src/owner_link.rs` each find one match on `main`.
- [x] 1.2 Apply the gate to `crates/mbv-gui` (D18): make `tauri` optional behind a default feature `tauri`, add `[[bin]] mbv-gui` with `required-features = ["tauri"]`, and return early from `build.rs` unless `CARGO_FEATURE_TAURI` is set. Keep `tauri-build` as a build dependency. Verify: `cargo check -p mbv-gui --no-default-features --lib` passes, and `cargo tree -p mbv-gui --no-default-features -i tauri` prints nothing.
- [x] 1.3 Confirm `mbv-gui` behaves as before. Verify: `cargo nextest run -p mbv-gui` passes with the default features.

## 2. The `mbv-native` crate

- [x] 2.1 Add `crates/mbv-native` (package and binary `mbv-native`) to the workspace `members`, not to `default-members`. Set `[lints] workspace = true`. Declare the direct dependencies in D4 with their exact versions and features, and the dev dependency `blitz-dom` (D4). Verify: `cargo check -p mbv-native` passes with no warnings.
- [x] 2.2 Add the `profiling` feature, which drops `#[global_allocator]` (D16). Verify: `cargo check -p mbv-native --features profiling` passes.
- [x] 2.3 Check the lock. Verify: `git diff -- Cargo.lock` shows no removed repository entry other than the nine replaced entries, and the replaced entries are exactly `bytemuck` 1.25.0 to 1.25.2, `smallvec` 1.16.1 to 1.16.5, and the seven `futures-*` crates (`futures-channel`, `futures-core`, `futures-io`, `futures-macro`, `futures-sink`, `futures-task`, `futures-util`) from 0.3.32 to 0.3.34 (D4). Dependency-edge changes inside the other existing entries are allowed (D4). If any other entry changes version, stop and report it.
- [x] 2.4 Write the dependency notes in `crates/mbv-native/Cargo.toml`: the tokio edge (D13) and the MPL-2.0 licence note (Risks). Verify: `rg -n 'tokio|MPL' crates/mbv-native/Cargo.toml` shows both notes.
- [x] 2.5 Add `crates/mbv-native` to the tokio edge list in `AGENTS.md` (the list of crates where tokio is allowed). Verify: `rg -n 'mbv-native' AGENTS.md` finds the entry.

## 3. In-process artwork and fonts

- [x] 3.1 Implement `MbvNetProvider` in `crates/mbv-native/src/art_net.rs` (D6). It implements `blitz_traits::net::NetProvider`. An `mbv-art` request sends `CoreInput::Art(ArtRequest { path, reply })`. The reply answers `Found` with a PNG downscaled to a 512 px long edge, answers `Missing` with `QUEUE_CARD_PLACEHOLDER_BYTES`, and calls `handler.bytes(url, Bytes)` both times. Verify: `cargo check -p mbv-native` passes.
- [x] 3.2 Serve the vendored fonts from `mbv-asset://localhost/font/<file>` with `include_bytes!` (D12). Verify: `cargo check -p mbv-native` passes.
- [x] 3.3 Write the contract tests 1 to 3 of D17 in `src/art_net.rs`, headless: a `Found` answer yields a 512 px raster on the image node, a `Missing` answer yields the placeholder raster, and an inline `@font-face` source is requested through the provider. Verify: `cargo nextest run -p mbv-native` passes.

## 4. Launch and core wiring

- [x] 4.1 Add `crates/mbv-native/src/launch.rs`: a copy of the `launch_cfg_with_props` body from `dioxus-native` 0.8.0-alpha.3 (D6). It uses `MbvNetProvider`, `navigation_provider: None`, `font_ctx: None`, renderer options with `base_color`, `alpha_mode`, `features`, and `limits` set to `None`, and window attributes with title `mbv` and the Wayland app id from D14. It creates no tokio runtime. Verify: `cargo check -p mbv-native` passes, and `rg -n 'tokio::runtime' crates/mbv-native/src` finds nothing.
- [x] 4.2 In `src/main.rs`: install `MiMalloc` as the global allocator (behind `not(feature = "profiling")`), call `declare_gui_client()` first (D15), start `mbv_gui::core_thread::spawn` with a sink that calls `unbounded_send` on a `futures_channel` sender (D7), and pass the core's sender to `launch`. On window close, send `CoreInput::Close` and join the core handle. Verify: `cargo check -p mbv-native` passes, and `rg -n 'declare_gui_client' crates/mbv-native/src` finds one call.

## 5. The Queue panel

- [ ] 5.1 Add the Dioxus root in `crates/mbv-native/src/app.rs`. A `use_future` drains the batch receiver and writes each section into its signal (D7). Verify: `cargo check -p mbv-native` passes.
- [ ] 5.2 Build the queue list from the `queue` section: the viewport rows, the active marker, and the active row's played fraction (D9). Verify: `cargo check -p mbv-native` passes.
- [ ] 5.3 Build the scrubber from the `playback` and `playhead` sections: the played fill, the chapter ticks, the intro and credits marks, and the elapsed and remaining clocks (D9). Verify: `cargo check -p mbv-native` passes.
- [ ] 5.4 Paint the playback artwork (B5). A pure function `art_src(&GuiArtRef) -> Option<&str>` returns `src` for `Image` and `Placeholder`, and `None` for `Loading` and an absent reference. Test that `Loading` yields `None`. Verify: `cargo nextest run -p mbv-native` passes.
- [ ] 5.5 Add the playhead tick (D8): a std thread sends a tick every 250 ms while the anchor is active and not paused. The signal receives the unquantized predicted position, and the clock text changes only on a whole-second change. Test contract 5 of D17 on the pure function `playhead_write`. Verify: `cargo nextest run -p mbv-native` passes.
- [ ] 5.6 Add `crates/mbv-native/assets/queue.css` (D11), included with `include_str!` in one `style` element. Vendor the six WOFF2 files (D12) from `@fontsource` 5.3.0 into `crates/mbv-native/assets/fonts/`, and declare their `@font-face` rules in the same style element. Verify: `fd -e woff2 . crates/mbv-native/assets/fonts` lists six files, and `rg -n 'include_str' crates/mbv-native/src` finds the CSS.

## 6. First-build checks

- [ ] 6.1 Run the headless contract 4 of D17 (a value sent from another thread updates the DOM after one `poll`) against the real `app.rs`. Verify: `cargo nextest run -p mbv-native` passes.
- [ ] 6.2 Run the fixture window check, only after the user approves it (Open Question 3). The run uses no Owner, no Emby, and no network. A fixture thread sends one batch per second to the window with no input. Record in `measurements.md`: whether the Queue panel updates with no input (D7), whether the placeholder and a downscaled image show (D6), whether the scrubber fill moves (G-Playhead), whether the `@font-face` faces render (D12), and the state of G2, G10, G11, G12, G13, G14, and G15. For each, record `confirmed` or `absent` in the design's gap table. Verify: the design gap table has no `verify` left, and `measurements.md` records each result.

## 7. Records, checks, and drafts

- [ ] 7.1 Add the term **Blitz Client** to `CONTEXT.md` (D19). The entry says what the Blitz Client is, that it is not the GUI Client, that it presents only the Queue panel, and that a full replacement folds it into the GUI Client. Verify: `rg -n 'Blitz Client' CONTEXT.md` finds the entry.
- [ ] 7.2 Write `openspec/changes/blitz-queue-panel-spike/measurements.md`. It holds the D16 protocol as commands only, a blank table for the GUI thread-count baseline, the go and no-go table with the bounds in "Go / No-Go", and a blank results section. Do not run the protocol. Verify: the file names every command in D16.
- [ ] 7.3 Draft upstream reports for G1, G8, and G9 in `openspec/changes/blitz-queue-panel-spike/upstream/`. Each draft has a minimal HTML and CSS repro where one applies, and it names the existing issues and WPT tests searched. Post nothing. Verify: three files exist, and each names what was searched.
- [ ] 7.4 Run `cargo fmt --all -- --check`, `cargo clippy -p mbv-native -p mbv-gui --all-targets -- -D warnings`, and `cargo nextest run -p mbv-native -p mbv-gui`. Fix each failure this change causes. Verify: all three commands pass.
- [ ] 7.5 Before the push, run `make check-code-file-lines`. Verify: the command reports no file over 800 lines.

## Workflow follow-up

- The live measurement in `measurements.md` needs the user's approval for each run (Open Question 2). It is not part of these tasks.
- Each upstream draft is posted only after the user approves it (decision 6).
- The fixture window run in task 6.2 needs the user's approval (Open Question 3).
