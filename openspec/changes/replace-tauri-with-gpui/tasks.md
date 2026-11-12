# Tasks

Precondition: `gui-change-driven-sections`, trimmed to its Rust half (proposal, Prerequisite), is merged into `main`. Groups 2 and 3 use its `SectionLedger`, `GuiSectionBatch`, `CoreInput::Sync`, and *Playhead anchor*.

Each row names its test contract and its owning layer (`docs/invariants/14-test-ownership.md`). A row with no new claim adds no test. `mbv-gui` is outside `default-members`, so name it: `cargo nextest run -p mbv-gui`. Run `cargo fmt` after each Rust row. Use no real sleep in any test: inject the elapsed time through a `*_for_test` seam. Until group 10, the Tauri binary keeps building and working (design D10); `make ui-test` stays green until group 10 deletes it.

## 1. Dependencies, licence, and the decision record

- [ ] 1.1 Add `gpui`, `gpui_platform` (`default-features = false`, `features = ["wayland"]`), `ui`, and `theme` to `crates/mbv-gui/Cargo.toml` at Zed revision `3c846c3d491095d75a76ca00a99fe348439736f6` (or the newest `main` revision on the day, recorded in the comment), and `flume` from the workspace. Copy Zed's `[patch.crates-io]` entries for `async-task`, `calloop`, and `khronos-egl` into the root `Cargo.toml` at the revisions of Zed's `Cargo.lock`. Verify: `cargo check -p mbv-gui` and `cargo check --workspace`; `cargo tree -p mbv-gui -i x11rb` finds nothing. Test contract: none.
- [ ] 1.2 Set `license = "GPL-3.0-or-later"` in `crates/mbv-gui/Cargo.toml` and add `crates/mbv-gui/LICENSE-GPL-3.0`. Add a licence note to `README.md` (the GUI binary is GPL, every other crate is MIT). Verify: `cargo metadata --format-version 1 | jq -r '.packages[] | select(.name=="mbv-gui") | .license'` prints `GPL-3.0-or-later`.
- [ ] 1.3 Install the GPUI build dependencies in CI (`build.yml`): the Wayland, xkbcommon, and Vulkan development packages that the build needs. Verify: the CI `mbv-gui` job passes on the branch.
- [ ] 1.4 Write ADR 0039 (read `docs/adr/README.md` first): the GUI window is GPUI; it amends ADR 0038's "Artwork is a URL" and its mount-time `sync` point (design D3, D6). Add it to the ADR index. Update the *GUI Client* entry in `CONTEXT.md` to name GPUI. Verify: `rg -n 0039 docs/adr/README.md` finds the entry.

## 2. The core-to-window channel

- [ ] 2.1 Add `WindowMsg { Batch, Art, Quit }` (design D2) in `crates/mbv-gui/src/window_msg.rs`. Change the core thread's sink from the Tauri emit closure to a `flume::Sender<WindowMsg>` behind the existing sink seam. The Tauri `main.rs` keeps working: it drains the receiver on a thread and emits as before. Send `Quit` at the run's quit edge. Verify: `cargo nextest run -p mbv-gui`. Test contract, in `tests/core_thread.rs`: the quit edge sends `Quit` after the last batch (`TestCore` reads `WindowMsg`).
- [ ] 2.2 Add `App::gui_art_source(&GuiArtRef) -> Option<Vec<u8>>` and `App::trickplay_sheet_bytes(...)` in `src/app/gui_library/` (read-only; the TUI does not call them). Add `CoreInput::Art(GuiArtRef)`; the step answers it with `WindowMsg::Art` and marks no section. Verify: `cargo nextest run -p mbv -p mbv-gui`. Test contracts, in `tests/core_thread/sections.rs`: an art request for a cached image answers with its encoded bytes; for an image the cache no longer holds, it answers `None`; neither request sends a batch.
- [ ] 2.3 Remove the `serde` derives from the section DTOs once no Tauri path serializes them (keep them until 10.1 if the Tauri bridge still needs them). Verify: `cargo check -p mbv-gui`. Test contract: none.

## 3. The GPUI-free window state

- [ ] 3.1 Add `crates/mbv-gui/src/window_state/sections.rs`: one store for each section (value and revision) and `apply(batch) -> Areas`, which writes only the sections whose revision differs and returns the set of areas to notify (design D4). Verify: `cargo nextest run -p mbv-gui`. Test contract: a batch with a held revision writes nothing and returns no area; a batch with `status` returns only the Status bar area.
- [ ] 3.2 Add `window_state/playhead.rs` (design D9): the anchor, the receive `Instant`, and the predicted position (clamped, no advance while paused), with an injected `now`. Verify: `cargo nextest run -p mbv-gui`. Test contracts, one `#[case]` table: playing advances by the elapsed time; paused does not advance; the prediction clamps at the runtime.
- [ ] 3.3 Add `window_state/derived.rs` (design D9): elapsed, remaining, the transport and queue finish clocks with the `+1d` suffix, each row's "Ends at" from `endsAfterSecs`, the active row's progress, and the current chapter, calling `fmt_duration_short` and `fmt_wall_clock` from `mbv-ui-model`. Verify: `cargo nextest run -p mbv-gui`. Test contracts: the queue finish clock passes midnight with `+1d` (spec scenario "A finish clock passes midnight"); the chapter whose span holds the position is current only while the Hero's book is the playing book.
- [ ] 3.4 Add `window_state/art_cache.rs` (design D6): a bounded LRU of decoded images keyed by `GuiArtRef`, with a pending set so one reference is requested one time, and eviction that returns the evicted images for the caller to drop. Verify: `cargo nextest run -p mbv-gui`. Test contracts: a second lookup of a held reference requests nothing (spec scenario "The same image shows again"); a `None` answer clears the pending mark and leaves the loading state.
- [ ] 3.5 Add the window-state section of `docs/architecture/gui-client.md` (the stores, the area notify rule, the playhead predictor, the art cache). Verify: the section names `window_state/`.

## 4. The GPUI program shell, theme, fonts, and icons

- [ ] 4.1 Add the temporary binary target `mbv-gui-gpui` (`src/bin/mbv-gui-gpui.rs`, design D10): it starts the core thread, builds the GPUI `Application`, loads the two bundled fonts (`include_bytes!`, `add_fonts`), installs mbv's `AssetSource` and `ThemeSettingsProvider` (design D8), opens one window with app id `io.github.slatkin.mbv-gui`, runs the foreground task that receives `WindowMsg`, sends `CoreInput::Close` on window close, calls `cx.quit()` on `Quit`, and joins the core thread after `run` returns. Move the IBM Plex Sans and JetBrains Mono files and the Lucide SVGs that `ui/src` imports into `crates/mbv-gui/assets/`. Verify: `cargo build -p mbv-gui --bin mbv-gui-gpui`. Test contract: none (program glue; live check in 4.4).
- [ ] 4.2 Add `window/theme.rs` and `window/canvas.rs` (design D8): the exhaustive map from the `theme` section (roles and chrome fills) and the canvas tokens to a Zed `Theme`, installed through `GlobalTheme::update_theme`, and the canvas token table with the names and values of `app.css`. Verify: `cargo nextest run -p mbv-gui`. Test contract: the default `mbv-theme` palette maps every `ThemeColors` field to an mbv value (no field keeps Zed's default).
- [ ] 4.3 Add the root view, the breakpoint, and the attach screens (waiting, failed, and dead-end states from the `attach` section). The root view sends `CoreInput::Sync` when it is created (design D3). Verify: `cargo nextest run -p mbv-gui`. Test contract, with GPUI's `TestAppContext`: a new root view sends exactly one `Sync` (spec requirement "The window opens with every section").
- [ ] 4.4 Live check by the user (ask first): `mbv-gui-gpui` opens on Wayland, shows the attach screen in the mbv palette and the bundled fonts, and closes cleanly. Verify: the user confirms.
- [ ] 4.5 Update *Process shape*, *Theme mapping*, and *Text and icons* in `docs/architecture/gui-client.md` for the GPUI program. Verify: these sections name no Tauri or webview type.

## 5. Input, layout reports, and the text field

- [ ] 5.1 Rewrite `bridge/keys.rs` to translate a GPUI `Keystroke` into the router's key event, and add the root view's `on_key_down` that offers keys to the Library panel and Settings state first, then sends `CoreInput::Key` (design D5). Port the key-translation tests from DOM names to GPUI names. Verify: `cargo nextest run -p mbv-gui`. Test contract: the existing key-translation cases, with GPUI input.
- [ ] 5.2 Rewrite `bridge/pointer.rs` to translate GPUI mouse and scroll events into `PointerEvent` gestures, keeping 16 px for one wheel line. Verify: `cargo nextest run -p mbv-gui`. Test contract: the existing gesture cases, with GPUI input; a line-mode wheel step converts to 16 px.
- [ ] 5.3 Add the layout report from painted bounds (design D5): each hit-map area records its bounds in prepaint, and the root view sends `CoreInput::Layout` only when the report differs from the last sent. Verify: `cargo nextest run -p mbv-gui`. Test contract, with `TestAppContext`: a repaint with no geometry change sends no report; a resize sends one.
- [ ] 5.4 Add `window/widgets/text_field.rs` (design D7): a single-line field on `EntityInputHandler` with the cursor, selection, the clipboard, and IME composition. Verify: `cargo nextest run -p mbv-gui`. Test contract, with `TestAppContext`: typed text and a backspace edit the value; the field's submit and cancel keys reach its owner.
- [ ] 5.5 Update *Layout reports and the hit map* and the key routing text in `docs/architecture/gui-client.md`. Verify: the section names `on_key_down` and the prepaint bounds.

## 6. The shared surfaces

- [ ] 6.1 Paint the player panel in its four states, with the Hero art from the art cache, the scrubber with the trickplay preview, and the transport controls (Zed `Button` and `IconButton`, design D7). The panel requests animation frames only while the playhead is playing (design D9). Verify: `cargo nextest run -p mbv-gui`. Test contract, with `TestAppContext`: a paused anchor requests no animation frame (spec scenario "Paused playback draws nothing").
- [ ] 6.2 Paint the queue: the header with the finish clock, the projected viewport rows with the marks, the hover, the drag visuals, the active row's progress, and the folded queue. Verify: `cargo nextest run -p mbv-gui`. Test contract: none beyond 3.3 (the views paint the projection).
- [ ] 6.3 Paint the row menu and the popovers from the core projection with `ListItem` rows (design D7, no Zed stateful menu). Verify: `cargo nextest run -p mbv-gui`. Test contract, with `TestAppContext`: a click on a menu row sends the pointer gesture and changes no window state.
- [ ] 6.4 Paint the Status bar, the queue footer, the overlays (confirm, help, and the DaemonLost overlay), and the toasts. Verify: `cargo nextest run -p mbv-gui`. Test contract: none (projection paint).
- [ ] 6.5 Live check by the user (ask first): with `mbv-gui-gpui`, play, pause, seek, reorder, and the row menu behave as in the Tauri window. Verify: the user confirms.
- [ ] 6.6 Update *The panel's four states*, *The player panel and the queue*, and *The Status bar in the footer line* in `docs/architecture/gui-client.md`. Verify: these sections name the GPUI views, not Svelte components.

## 7. The Library panel

- [ ] 7.1 Port the Library panel state from `ui/src/lib/library/` into `window_state/library/` (one module per destination where the TypeScript has one), keeping ADR 0036's ownership. Move each Vitest test that owns a contract into Rust unit tests; drop markup-only tests. Verify: `cargo nextest run -p mbv-gui`. Test contracts: the moved tests, each with its source test named in a comment.
- [ ] 7.2 Paint the Library top bar, tabs, pills, filter field (5.4), the letter rail, and the list with `uniform_list` for each of the eight destinations, plus Search. Verify: `cargo nextest run -p mbv-gui`. Test contract, with `TestAppContext`: a list update keeps the scroll offset and the selection (spec scenario "A list update keeps the window's state").
- [ ] 7.3 Paint the Heroes, the Workspace table, the tracklist with the playing mark, the Audiobooks chapters with the current chapter (3.3), and the item menu. Verify: `cargo nextest run -p mbv-gui`. Test contract: none beyond 3.3 and 7.1.
- [ ] 7.4 Live check by the user (ask first): every destination, Search, and art loading behave as in the Tauri window. Verify: the user confirms.
- [ ] 7.5 Update *The Library panel* in `docs/architecture/gui-client.md` and the GUI Library panel row in `docs/architecture/interactive-surface-ledger.md`. Verify: neither names a Svelte file.

## 8. The Settings screen

- [ ] 8.1 Port the Settings state from `ui/src/lib/settings/` into `window_state/settings/`, keeping ADR 0037's ownership, with the same test rule as 7.1. Verify: `cargo nextest run -p mbv-gui`. Test contracts: the moved tests, each with its source test named in a comment.
- [ ] 8.2 Paint every Settings section with the text field (5.4), `Checkbox`, `Switch`, and mbv's own controls where the user rejects a Zed widget (design D7). Verify: `cargo nextest run -p mbv-gui`. Test contract: none beyond 8.1.
- [ ] 8.3 Live check by the user (ask first): each Settings section edits and saves as in the Tauri window. Verify: the user confirms.
- [ ] 8.4 Update *The Settings screen* in `docs/architecture/gui-client.md`. Verify: it names no Svelte file.

## 9. Widget replacements

- [ ] 9.1 Review the GPUI window with the user (ask first) and list each Zed widget the user rejects. For each one, add mbv's own element in `window/widgets/` and change its call sites (design D7). Verify: the user confirms the list is empty or done; `cargo nextest run -p mbv-gui`. Test contract: none (appearance).

## 10. Delete Tauri

- [ ] 10.1 Make `mbv-gui`'s `main.rs` the GPUI program from `mbv-gui-gpui` and delete the temporary target. Delete `ui/`, `tauri.conf.json`, `capabilities/`, `build.rs`, the Tauri icon set, the Tauri glue in `main.rs`, and the `tauri`, `tauri-build`, `base64`, and `serde` dependencies. Remove the `serde` derives that 2.3 kept. Verify: `cargo nextest run -p mbv-gui`; `cargo tree -p mbv-gui -i webkit2gtk` finds nothing.
- [ ] 10.2 Remove the `ui-*` targets and the Node and pnpm steps from `make dev`, the `Makefile`, and CI. Close PR 999 with a note that this change made it moot (ask the user first). Verify: `rg -n 'pnpm|ui-test|ui-check' Makefile .github` finds nothing.
- [ ] 10.3 Update `PKGBUILD`, `PKGBUILD-git`, and the `.deb` dependency list: drop WebKitGTK 4.1, add the Vulkan loader, and keep the desktop entry and app id. Verify: `rg -n -i webkit PKGBUILD PKGBUILD-git` finds nothing; `makepkg --printsrcinfo` runs.
- [ ] 10.4 Finish `docs/architecture/gui-client.md`: delete *WebKitGTK renderer note* and every remaining Tauri or Svelte reference, and update the `mbv-gui` entry in `AGENTS.md`. Verify: `rg -n -i 'tauri|svelte|webview|webkit' docs/architecture/gui-client.md AGENTS.md` finds nothing.

## 11. Integration checks

- [ ] 11.1 Run `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo nextest run --workspace`, `cargo nextest run -p mbv-gui`, and `openspec validate replace-tauri-with-gpui --strict`. Verify: all pass.
- [ ] 11.2 Live check by the user (ask first): the packaged `mbv-gui` from 10.3 installs, starts from the desktop entry, attaches to the Owner, plays, and quits with the exit teardown. Verify: the user confirms.
- [ ] 11.3 Run `make check-code-file-lines` before the push. Verify: it passes.

## Workflow follow-up

- Sync the delta specs into `openspec/specs/` and archive this change only when the user approves the PR for merge, as the last commit before the merge.
