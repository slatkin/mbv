# Tasks

## 1. Confirm the defect and legacy semantics

- [x] 1.1 In `~/Dev/mbv/src/app/input_lib_keys.rs`, record in `design.md` D3
      exactly what `Ctrl+P/A/W/S/R` and bare `r` did for a **music** library
      row and a **TV** library row (series vs episode), including whether
      `Ctrl+W` targeted the series or the highlighted episode by pane.
      Verify: the notes cite line numbers from the legacy file, not this
      proposal's summary.
- [x] 1.2 Confirm on this branch that `MusicWorkspaceComponent::handle_key`
      and `TvWorkspaceComponent::handle_key` drop these chords at `_ => None`
      (`music_workspace.rs:360`, `tv_workspace.rs:393`) and that no shell
      fallback consumes them. Verify:
      `rtk grep -rn "GlobalViewKey|handle_legacy_key" src/` returns 0.

## 2. Add the `Library*` request variants

- [x] 2.1 Add `LibraryPlay { item }`, `LibraryEnqueue { item }`,
      `LibraryToggleWatched { item }`, `LibraryShuffle { item }`,
      `LibraryRefresh`, `LibraryRescan` to `ShellRequest`
      (`src/app/components/msg/shell.rs`), each with a doc comment pointing
      at the `Browser*` equivalent it mirrors. Verify: `rtk cargo check -p mbv`.
- [x] 2.2 Check `CONTEXT.md` for a name collision with `Library*` terms; if
      one exists, rename to `EmbyLibrary*` before proceeding. Verify: the
      chosen prefix appears in no other `ShellRequest` variant.

## 3. Route the variants in the shell (D1)

- [x] 3.1 Extend `Model::handle_browser_request`
      (`src/app/shell_browser.rs:20`) — or extract a shared
      `handle_emby_library_request` — so the six new variants call
      `play_or_activate_lib_item` / `enqueue_lib_item` / `toggle_watched_item`
      / `shuffle_play_selected` / `refresh_lib` / `ask_confirm(RescanLibrary)`
      with `lib_idx` from `self.app.tab.emby_library_index()`. Verify:
      `rtk cargo check -p mbv`; if `shell_browser.rs` exceeds 800 lines,
      split per `rtk make check-code-file-lines`.
- [x] 3.2 Add the dispatch arms in `src/app/shell_messages.rs` so the new
      variants reach 3.1 (an unrouted variant is silent — see
      `make-shell-dispatch-exhaustive`). Verify: `rtk cargo check -p mbv`.

## 4. Claim the chords in the Music component (D3)

- [ ] 4.1 In `MusicWorkspaceComponent::handle_key`, add album-level arms for
      `Ctrl+P/A/W/S/R` and bare `r`, each guarded on `track_cursor.is_none()`,
      placed before the `[`/`]` group-pill arms, resolving the target from
      `self.selected_item()` and returning `Msg::Shell(ShellRequest::Library…)`.
      `Ctrl+R` before bare `r`. Verify: `rtk cargo check -p mbv`.
- [ ] 4.2 Confirm the existing track-focus `Ctrl+P` / `Ctrl+A` arms
      (`music_workspace.rs:236, 271`) still win when `track_cursor.is_some()`.
      Verify: test 5.2 below.

## 5. Claim the chords in the TV component (D3)

- [ ] 5.1 In `TvWorkspaceComponent::handle_key`, add `Ctrl+P/A/W/S/R` and
      bare `r` arms before the letter-pill arm, resolving from
      `self.selected_item()` with the pane-correct `EmbyItem` per 1.1.
      Verify: `rtk cargo check -p mbv`.

## 6. Tests (D5)

- [ ] 6.1 Component tests in
      `src/app/components/music_workspace_component_tests.rs`:
      `ctrl_s_on_album_emits_library_shuffle`,
      `ctrl_s_with_track_focus_does_not_shuffle`,
      `ctrl_p_empty_list_is_unclaimed`. Verify: `rtk cargo nextest run -p mbv`.
- [ ] 6.2 Component tests in
      `src/app/components/tv_workspace_component_tests.rs`:
      `ctrl_r_emits_library_rescan`,
      `ctrl_w_emits_library_toggle_watched`. Verify: `rtk cargo nextest run -p mbv`.
- [ ] 6.3 Model tests mirroring `shell_browser_tests.rs:81-211`: one per new
      variant, driving the key through the mounted music/TV component and
      asserting the `App` effect ran. Verify: `rtk cargo nextest run -p mbv`.

## 7. Close out

- [ ] 7.1 Update the Music and TV workspace rows in
      `docs/architecture/interactive-surface-ledger.md`.
- [ ] 7.2 Re-read `render/components/help.rs` `Ctrl+S`/`Ctrl+R` lines and the
      `["Shuffle", "Rescan", "Search library"]` "Emby-only" gate — confirm it
      still reads correctly now that Music/TV honor the chords. Adjust only
      if the gate text is now wrong.
- [ ] 7.3 Verify: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`
      (`no-raw-fallback-variants` green), `rtk cargo fmt`,
      `rtk make check-code-file-lines` all pass.
- [ ] 7.4 Sync the `service-browse-dispatch` delta into
      `openspec/specs/service-browse-dispatch/spec.md` and archive the change.
      Comment the outcome on #633 and close it.
