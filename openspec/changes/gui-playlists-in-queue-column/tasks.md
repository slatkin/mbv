# Tasks

Add no new tests in this change. Run every cargo command in the foreground and wait for it to finish; do not background it, sleep, or poll.

## 1. Core: toggle and close rules

- [ ] 1.1 In `src/app/gui_playlists.rs`, make `open_gui_playlists` set `panel_collapsed = false`, and make `toggle_gui_playlists` follow design D4: open and not collapsed closes; open and collapsed expands and keeps the view open with no reload; closed opens. Verify that `cargo check -p mbv` passes and that `cargo nextest run -p mbv gui_playlists` passes with no test changes.
- [ ] 1.2 In `src/app/gui_playlists/commands.rs`, make the `Action` and `ItemAction` arms call `close_gui_playlists()` after the shared action when the action is `MusicTreeAction::Enqueue` (design D5). Leave `src/app/dispatch/playlists.rs` unchanged. Verify that `cargo check -p mbv` passes and that `git diff --stat` shows no change under `src/app/dispatch/`.

## 2. GUI: the Queue column paints the Playlists view

- [ ] 2.1 In `crates/mbv-gui/src/window/components/queue_playback_panel.rs`, add the `now_playing_title: bool` prop with `#[props(default = true)]`. While an item plays, the art section always paints, and the title block paints only when the prop is true (design D2). Update the module doc. Verify that `cargo check -p mbv-gui` passes.
- [ ] 2.2 In `crates/mbv-gui/src/window/components/playlists_panel.rs`, remove the Wide arrangement, the `PanelDivider`, the `PanelWidthsStore` handles, the `arrangement`/`list_width`/`library_column` props, and `data-playlists-arrangement`. Keep the narrow column (hero above tree) with its classes and colours unchanged, and add no `data-pointer-zone` (design D3). Rewrite the module doc for the Queue column placement. Verify that `cargo check -p mbv-gui` passes and that `rg -n "GuiArrangement|PanelDivider|PanelWidthsStore" crates/mbv-gui/src/window/components/playlists_panel.rs` finds nothing.
- [ ] 2.3 In `crates/mbv-gui/src/window/components/panel.rs`, read `sections.playlists` and paint as in design D1: in Expanded and ExpandedWithoutQueue, `QueuePlaybackPanel { now_playing_title: false }` and `PlaylistsPanel {}` in the `min-h-0 flex-1` slot, with `QueueFooter` kept; in CompactStrip, `PlaylistsPanel {}` in a `min-h-0 flex-1` slot under `CompactStrip`; Rail unchanged. Update the module doc. Verify that `cargo check -p mbv-gui` passes.

## 3. GUI: App, top bar, and Mini

- [ ] 3.1 In `crates/mbv-gui/src/window/app.rs`, remove the `PlaylistsPanel` branch from the Library slot, set `panel_shows` to `has_library_list`, pass `header.current_tab` and `header.home_active` to `LibraryTopBar` with no Playlists override, and remove `painted_view` (design D6). Delete the test `mini_paints_the_playlists_screen_in_the_library_slot` and the `painted_view` import in `crates/mbv-gui/src/window/app/tests/header.rs`. Verify that `cargo check -p mbv-gui --all-targets` passes.
- [ ] 3.2 In `crates/mbv-gui/src/window/components/library_top_bar.rs`, stop sending `GuiPlaylistsCommand::Close` in `open_destination` (design D8), and add `#[props(default)] on_playlists_pressed: Option<Callback<()>>`. When the prop is set, the Playlists control calls it in place of sending `Toggle` (design D7). Update the doc comments that say a destination press closes the screen. Verify that `cargo check -p mbv-gui --all-targets` passes.
- [ ] 3.3 Create the child module `crates/mbv-gui/src/window/app/playlists.rs` with the Mini open-edge effect and the Playlists press handler from design D7, and wire both into `App` (pass the handler as `on_playlists_pressed`). Verify that `cargo check -p mbv-gui` passes and that `wc -l crates/mbv-gui/src/window/app.rs` reports at most 800 lines.

## 4. GUI: key routing

- [ ] 4.1 In `crates/mbv-gui/src/window/library/keys.rs`, change the Playlists gate in `route_library_key` so that it keeps only the rename field step and falls through to the Library steps when no rename draft holds the keyboard (design D9). Update the comment. Verify that `cargo check -p mbv-gui` passes and that `cargo nextest run -p mbv-gui keys` passes.

## 5. Docs

- [ ] 5.1 Update `docs/architecture/gui-client.md`: rewrite the Playlists screen paragraph for the Playlists view in the Queue column, and change "the Playlists screen" in the Ctrl+K routing step to "the Playlists view" (design D10). Verify that `rg -n "Playlists screen" docs/architecture/gui-client.md` finds nothing.
- [ ] 5.2 Update `CONTEXT.md`: add the *Playlists view* term with its *Avoid* line, remove Playlists from the Settings screen's section list, and change the Service mark entry to "Home shows a text title" (design D10). Verify that `rg -n "Playlists screen" CONTEXT.md` finds only the *Avoid* line.

## 6. Integration checks

- [ ] 6.1 Run `cargo fmt`, then `cargo clippy --workspace --all-targets -- -D warnings`, and verify that clippy reports no warnings.
- [ ] 6.2 Run `cargo nextest run -p mbv -p mbv-gui` and verify that every test passes.
- [ ] 6.3 Run `openspec validate gui-playlists-in-queue-column --strict` and `make check-code-file-lines`, and verify that both pass.

## Workflow follow-up

- Sync the delta specs into `openspec/specs/` and archive the change only after the user approves the PR for merge, as the last commit before the merge.
