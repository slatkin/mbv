# Tasks

Design decisions are cited as `D<n>`. Every port task follows the port rules of D15 (markup, classes, state, logic, commands, scroll, focus, text fields) and the test rules of D16: port a test only when it owns a contract that still holds. Every task that adds or changes markup runs `make gui-css` and commits `crates/mbv-gui/assets/app.generated.css` with the markup (D11). "Port" source paths are under `crates/mbv-gui/ui/src/`; target paths are under `crates/mbv-gui/src/window/`. Each task ends with `cargo fmt`, `cargo clippy -p mbv-gui --all-targets -- -D warnings`, and `cargo nextest run -p mbv-gui` passing.

## 1. Gate, dependencies, and the window process

- [ ] 1.1 Apply the D3 layout to `crates/mbv-gui/Cargo.toml`: features `default = ["tauri", "blitz"]`, `tauri = ["dep:tauri"]`, `blitz`; `tauri = { version = "=2.12.5", optional = true }`; `[[bin]] mbv-gui` with `required-features = ["tauri"]`; `[[bin]] mbv-gui-blitz` at `src/bin/mbv_gui_blitz.rs` with `required-features = ["blitz"]`. `build.rs` returns early unless `CARGO_FEATURE_TAURI` is set. Verify: `cargo check -p mbv-gui`, `cargo check -p mbv-gui --no-default-features --features blitz`, and `cargo tree -p mbv-gui --no-default-features --features blitz -i tauri` prints nothing.
- [ ] 1.2 Add the D2 dependencies behind the `blitz` feature with their exact versions: `dioxus-native` `=0.8.0-alpha.3` (features `prelude`, `svg`, `system-fonts`, `woff`, `vello-hybrid`, no defaults), `blitz-shell` `=0.3.0-beta.4`, `blitz-traits` `=0.3.0-beta.4`, `blitz-dom` `=0.3.0-beta.4` (feature `woff`), `parley` `=0.12.0` (feature `std`), `futures-channel` `=0.3.34`, `futures-util` `=0.3.34` (feature `std`), and `image` (workspace) with `png`, `jpeg`, `webp`. Verify: `git diff -- Cargo.lock` changes no existing entry's version other than the nine in D2; stop and report any other change.
- [ ] 1.3 In `AGENTS.md`, change the `crates/mbv-gui` tokio edge reason to `dioxus-native`'s asset resolver (D2). Add the tokio and MPL-2.0 notes to `crates/mbv-gui/Cargo.toml`. Verify: `rg -n 'dioxus' AGENTS.md crates/mbv-gui/Cargo.toml` shows both.
- [ ] 1.4 Add `src/window.rs` (`#[cfg(feature = "blitz")]`), `window/launch.rs` (the D5 launch copy, from `launch_cfg_with_props` of `dioxus-native` 0.8.0-alpha.3), `window/channel.rs` (`WindowEvent`, the window channel, `CoreLink` with the six methods of D4), and `src/bin/mbv_gui_blitz.rs` (D4: log init, `core_thread::spawn` with the channel sink and the quit sink, launch, close, join). The root renders an empty element and drains the channel. Verify: `cargo check -p mbv-gui --bin mbv-gui-blitz`; `rg -n 'tokio::runtime' crates/mbv-gui/src/window` finds nothing.
- [ ] 1.5 Add `window/net.rs` (`MbvNetProvider`, D7) with the `mbv-art` and `mbv-asset` routes and `ArtEvent`. Headless tests (D16): a `Found` answer gives the `img` node a raster and sends `ArtEvent::Loaded` with the source size; a `Missing` answer leaves the node unloaded and sends `ArtEvent::Failed`; an inline `@font-face` source is requested through the provider. Verify: the three tests pass.
- [ ] 1.6 Add `window/clock.rs` (D10): the timer thread, `after`, `cancel`, `want_frames`, the frame hook in `launch.rs`, and a recording clock for tests. Test: a cancelled timer sends nothing (recording clock). Verify: the test passes.

## 2. Look: stylesheet, fonts, icons, ellipsis

- [ ] 2.1 Copy the six Latin WOFF2 files (weights 400, 500, 600) of `@fontsource/ibm-plex-sans` 5.3.0 and `@fontsource/jetbrains-mono` 5.3.0 from `crates/mbv-gui/ui/node_modules` into `crates/mbv-gui/assets/fonts/`, with the OFL 1.1 text. Serve them from `net.rs` (D7). Verify: `fd -e woff2 . crates/mbv-gui/assets/fonts` lists six files.
- [ ] 2.2 Copy `tw-animate-css` 1.4.0 and `shadcn-svelte/tailwind.css` 1.8.0 into `crates/mbv-gui/assets/vendor/` with their MIT notices. Write `crates/mbv-gui/assets/app.css` as the port of `app.css` (D11): relative vendor imports, `@source "../src/window";`, and the six `@font-face` rules. Add the Makefile target `gui-css` that runs the Tailwind v4.3.5 standalone CLI (D11). Run it, commit `app.generated.css`, and include it in the root with `include_str!`. Verify: `make gui-css` exits 0 and `rg -n 'mbv-asset://localhost/font' crates/mbv-gui/assets/app.generated.css` finds six rules.
- [ ] 2.3 Add `window/icons.rs` (D13): the 36 Lucide icons that D13 lists, each with the child elements from `@lucide/svelte` 1.70.0 and the `class` and `size` props, and the ISC notice in `assets/vendor/`. Verify: `cargo check -p mbv-gui`, and every icon name in the `@lucide/svelte` imports of `ui/src` (multi-line blocks included) has a function in `icons.rs`.
- [ ] 2.4 Add `window/truncate.rs` (D12): the process font context over the bundled faces, `TextStyle`, the pure cut function `fit_text(text, style, width) -> String`, and the `Truncate` component. Tests: a text that fits is unchanged; a text that does not fit ends in `…` and its measured width is at most the box width. Verify: the tests pass.

## 3. Bridge layer ports

- [ ] 3.1 Port `lib/bridge/sections.ts` to `window/sections.rs` and `lib/bridge/theme.ts` to `window/theme.rs` (D6). Port the contracts of `sections.test.ts`: a section at or below its held revision is ignored; a batch writes all its sections in one pass; a section the batch does not name stays. Verify: the tests pass.
- [ ] 3.2 Port `lib/bridge/time.ts` to `window/time.rs` and `lib/bridge/playhead.ts` to `window/playhead.rs` over `Clock` (D10). Port the contracts of `time.test.ts` and `playhead.test.ts`. Verify: the tests pass.
- [ ] 3.3 Port `lib/bridge/input.ts` to `window/input.rs` (D8): `DomKey` and `DomPointer` from the Dioxus event data, the key router with the `Core`, `Handled`, and `Text` outcomes, and the root handlers. Port the contracts of `input.test.ts`. Verify: the tests pass.
- [ ] 3.4 Add `GuiPanelWidths`, `load_gui_panel_widths`, and `save_gui_panel_widths` to `mbv-config` (D14). Test in `mbv-config`'s test binary: an unreadable file gives the defaults. Verify: `cargo nextest run -p mbv-config` passes.
- [ ] 3.5 Port `lib/bridge/layout.ts`, `resize.ts`, `panel.ts`, `arrangement.ts`, and `natural-size.ts` to `window/` (D9, D14), with the `LayoutEpoch` signal and the container-size effect. Port the contracts of `resize.test.ts`, `panel.test.ts`, and `natural-size.test.ts`. Verify: the tests pass.
- [ ] 3.6 Port `ArtImage.svelte` to `window/components/art_image.rs` (D7): the fill, the natural size from `ArtEvent::Loaded`, and the retry on `ArtEvent::Failed` with the 1 s to 30 s backoff over `Clock`. Headless test: a failed URL paints the fill and, after the recording clock fires, recreates the `img`. Verify: the test passes.

## 4. Root, attach, and overlays

- [ ] 4.1 Port `App.svelte` and `main.ts` to `window/app.rs` (D6): the stylesheet, the theme style, the `sync` on mount, and the composition of the attach screen, the overlays, the Settings screen, and the player and Library trees, with each not-yet-ported child as an empty component in its target module. Verify: `cargo check -p mbv-gui`.
- [ ] 4.2 Port `AttachScreen.svelte`, `DaemonLostOverlay.svelte`, and `ConfirmOverlay.svelte`. Port the contracts of their `.test.ts` files. Verify: the tests pass.

## 5. Player panel and Queue panel

- [ ] 5.1 Port `Panel.svelte` and `PanelDivider.svelte` (the divider's drag, double-click reset, and width save through D14). Port the contracts of `Panel.test.ts`. Verify: the tests pass.
- [ ] 5.2 Port the artwork, title, and transport parts of `QueuePlaybackPanel.svelte` to `window/components/queue_playback_panel.rs` (split at 800 lines along the scrubber seam). Verify: `cargo check -p mbv-gui`.
- [ ] 5.3 Port the scrubber of `QueuePlaybackPanel.svelte`: the fill, the chapter ticks, the intro and credits marks, the clocks, the seek, and the hover preview with the trickplay tile. Port the contracts of `QueuePlaybackPanel.test.ts`. Verify: the tests pass.
- [ ] 5.4 Port `TrackChips.svelte` and its popovers. Port the contracts of `TrackChips.test.ts`. Verify: the tests pass.
- [ ] 5.5 Port `QueuePanel.svelte` (rows, the active row's progress, the finish times, the drag reorder visuals) and `QueueRowMenu.svelte`. Port the contracts of `QueuePanel.test.ts`. Verify: the tests pass.
- [ ] 5.6 Port `QueueHeader.svelte`, `QueueFooter.svelte`, and `TypedText.svelte`. Verify: `cargo check -p mbv-gui`.
- [ ] 5.7 Port `FoldedQueue.svelte`, `CompactStrip.svelte`, and `CollapsedRail.svelte`, and wire the four panel states in `app.rs` (D9). Verify: `cargo check -p mbv-gui`.

## 6. Library logic

- [ ] 6.1 Port `lib/library/state.ts`, `reset.ts`, `row-memo.ts`, `scroll.ts`, and `parts.ts`. Port the contracts of `scroll.test.ts` and `reveal.test.ts`. Verify: the tests pass.
- [ ] 6.2 Port `lib/library/keys-shared.ts` and `keys.ts`, and install the Library steps in the key router (D8). Port the contracts of `keys.test.ts`. Verify: the tests pass.
- [ ] 6.3 Port `lib/library/list.ts`, `pill-list.ts`, `hero.ts`, and `status.ts`. Port the contracts of `list.test.ts`, `hero.test.ts`, and `status.test.ts`. Verify: the tests pass.
- [ ] 6.4 Port `lib/library/home.ts`, `home-list.ts`, `emby.ts`, `emby-list.ts`, and `emby-keys.ts`. Port the contracts of `home.test.ts`, `home-list.test.ts`, `emby-list.test.ts`, and `emby-keys.test.ts`. Verify: the tests pass.
- [ ] 6.5 Port `lib/library/shows.ts`, `shows-list.ts`, `shows-keys.ts`, and `shows-workspace.ts`. Port the contracts of their `.test.ts` files. Verify: the tests pass.
- [ ] 6.6 Port `lib/library/audiobooks.ts`, `audiobooks-list.ts`, `audiobooks-keys.ts`, and `audiobooks-workspace.ts`. Port the contracts of their `.test.ts` files. Verify: the tests pass.
- [ ] 6.7 Port `lib/library/podcasts.ts`, `podcasts-list.ts`, `podcasts-keys.ts`, `feeds.ts`, `feeds-list.ts`, and `feeds-keys.ts`. Port the contracts of their `.test.ts` files. Verify: the tests pass.
- [ ] 6.8 Port `lib/library/search.ts` with the 200 ms debounce over `Clock` (D10). Port the contracts of `search.test.ts` and `search-keys.test.ts`. Verify: the tests pass.

## 7. Library components

- [ ] 7.1 Port `LibraryPanel.svelte`, `LibraryTopBar.svelte`, `LibraryStatusBar.svelte`, and `LibraryFilterField.svelte`. Port the contracts of `LibraryPanel.test.ts`, `LibraryTopBar.test.ts`, and `LibraryStatusBar.test.ts`. Verify: the tests pass.
- [ ] 7.2 Port `LibraryVirtualList.svelte` (scroll offset and row window through D15 scroll and D9 sizes), `LibraryList.svelte`, `LibraryItemRow.svelte`, `LibraryListHeader.svelte`, `LibraryListNotice.svelte`, `LibraryLetterRail.svelte`, and `LibraryLetterHeading.svelte`. Port the contracts of `LibraryList.test.ts` and `LibraryLetterRail.test.ts`. Verify: the tests pass.
- [ ] 7.3 Port `LibraryWorkspaceTable.svelte` and `LibraryTracklist.svelte`. Verify: `cargo check -p mbv-gui`.
- [ ] 7.4 Port `LibraryPillButton.svelte`, `LibraryPillRow.svelte`, `LibraryPills.svelte`, and the four destination pill components. Port the contracts of their `.test.ts` files. Verify: the tests pass.
- [ ] 7.5 Port `LibraryHero.svelte`, `LibraryHeroFrame.svelte`, `LibraryHeroArt.svelte`, `LibraryHeroSummary.svelte`, `LibraryHeroActions.svelte`, and `LibraryHomeHero.svelte`. Port the contracts of `LibraryHero.test.ts`, `LibraryHeroArt.test.ts`, and `LibraryHomeHero.test.ts`. Verify: the tests pass.
- [ ] 7.6 Port `LibraryEmbyHero.svelte` (with the `openUrl` command), `LibraryShowsHero.svelte`, `LibraryAudiobooksHero.svelte`, `LibraryPodcastsHero.svelte`, and `LibraryFeedsHero.svelte`. Port the contracts of their `.test.ts` files. Verify: the tests pass.
- [ ] 7.7 Port `LibraryHomeList.svelte`, `LibraryEmbyList.svelte`, `LibraryShowsList.svelte`, `LibraryAudiobooksList.svelte`, `LibraryPodcastsList.svelte`, and `LibraryFeedsList.svelte`. Port the contracts of their `.test.ts` files. Verify: the tests pass.

## 8. Settings screen

- [ ] 8.1 Port `lib/settings/state.ts`, `entries.ts`, `focus.ts`, `keys.ts`, and `pick-menu.ts`, and install the Settings steps ahead of the Library steps in the key router (D8). Port the contracts of `rows.test.ts`, `keys.test.ts`, `feeds.test.ts`, and `playlists.test.ts` under `lib/settings/`. Verify: the tests pass.
- [ ] 8.2 Port the eight components in `lib/components/settings/`. Verify: `cargo check -p mbv-gui`.
- [ ] 8.3 Port `SettingsScreen.svelte`, `SettingsSidebar.svelte`, `SettingsStatusBar.svelte`, `SettingsEntryRow.svelte`, and `SettingsRows.svelte`. Port the contracts of `SettingsRows.test.ts` and the screen's other `.test.ts` files. Verify: the tests pass.
- [ ] 8.4 Port `SettingsServices.svelte`, `SettingsFeeds.svelte`, `SettingsKeys.svelte`, `SettingsLibrary.svelte`, `SettingsPlaylists.svelte`, and `SettingsSessions.svelte`. Port the contracts of their `.test.ts` files. Verify: the tests pass.

## 9. The switch

- [ ] 9.1 Delete the Tauri path (D18): `crates/mbv-gui/ui/`, `tauri.conf.json`, `capabilities/`, `permissions/`, `gen/`, `build.rs`, the Tauri `src/main.rs`, `tests/capabilities.rs`, the features, and the Tauri dependencies. Move `src/bin/mbv_gui_blitz.rs` to `src/main.rs` as binary `mbv-gui`, and drop the `cfg` on `window`. Verify: `cargo build -p mbv-gui` produces `target/debug/mbv-gui`; `cargo tree -p mbv-gui -i tauri` and `-i webkit2gtk` print nothing.
- [ ] 9.2 Drop the serde derives and the wire tests in `bridge/` and `tests/bridge/` that nothing reads after 9.1, and rewrite the `tests/bridge/keys.rs` and `pointer.rs` translation tests to build `DomKey` and `DomPointer` directly (D16). Verify: `cargo nextest run -p mbv-gui` passes and `cargo clippy -p mbv-gui --all-targets -- -D warnings` reports no dead code.
- [ ] 9.3 Confirm no handler uses an event kind that panics (D8). Verify: `rg -n 'on(resize|visible|drag\w*|drop|transition\w*|animation\w*|copy|cut|paste|composition\w*|select|toggle)\b' crates/mbv-gui/src/window` finds nothing.
- [ ] 9.4 Packaging and CI (D17): `PKGBUILD`, `PKGBUILD-git`, the `.deb` `Depends` field and its check in `build.yml`, the CI container package lists, the removed `ui-*` steps, the Makefile targets and `UI_DIR`, and the `LICENSES/` notices in the tarball, `.deb`, and Arch packages. Verify: `rg -n 'webkit|pnpm|nodejs|UI_DIR' Makefile PKGBUILD PKGBUILD-git .github/workflows` finds nothing.

## 10. Records and gates

- [ ] 10.1 Write `docs/adr/0039-gui-client-renders-with-dioxus-native-on-blitz.md` (D19), following `docs/adr/README.md`. Verify: the file exists with status `proposed`.
- [ ] 10.2 Update `CONTEXT.md` (GUI Client), `AGENTS.md` (the `crates/mbv-gui/` map entry and the ADR 0036 note), and `docs/architecture/gui-client.md` (D19). Verify: `rg -n -i 'tauri|webview|webkit|svelte' CONTEXT.md AGENTS.md docs/architecture/gui-client.md` finds only historical mentions that name the replaced stack.
- [ ] 10.3 Update `docs/recipes.md`, `docs/architecture/performance.md`, `docs/architecture/interactive-surface-ledger.md`, the invariants, and the code comments in `src/`, `crates/`, and `docs/` that name the webview, Tauri commands, `app.css` paths, or Svelte files (D19). Verify: `rg -n -i 'tauri|webview|webkit|\.svelte|ui/src' src crates docs --glob '!docs/adr/**'` finds only historical mentions.
- [ ] 10.4 Run the gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, and `make check-code-file-lines`. Fix each failure. Verify: all four pass.
