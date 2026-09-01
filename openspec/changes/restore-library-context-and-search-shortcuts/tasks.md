# Tasks

## 1. Confirm the defect and legacy semantics

- [x] 1.1 In the pre-migration tree, record in `design.md` (append to
      Context) exactly what `.` and `/` did for a **music** library row and a
      **TV** library row: `input_lib_keys.rs:142` (`.` →
      `open_context_menu()`) and `:292` (`/`). Verify: notes cite the legacy
      line numbers, not this proposal.
- [x] 1.2 Confirm on this branch that `MusicWorkspaceComponent::handle_key`
      drops `.` at the album level and `/` entirely, and
      `TvWorkspaceComponent::handle_key` drops both, all at `_ => None`.
      Verify: `rtk grep -n "Char('.')|Char('/')" src/app/components/music_workspace.rs src/app/components/tv_workspace.rs`
      shows only the existing focused-track `.` arm.

## 2. Add the `EmbyLibraryContextMenu` request variant

- [x] 2.1 Add `EmbyLibraryContextMenu { item }` to `ShellRequest`
      (`src/app/components/msg/shell.rs`), with a doc comment pointing at
      `BrowserContextMenu` as the mirror. Verify: `rtk cargo check -p mbv`.
- [x] 2.2 Confirm no `CONTEXT.md` term collision on the variant name (the
      `EmbyLibrary*` prefix is already used by #633). Verify: prefix appears
      only on the #633 variants and this one.

## 3. Route the variant in the shell (D1)

- [x] 3.1 Route `EmbyLibraryContextMenu { item }` to
      `self.app.open_context_menu_for(item)` beside the `EmbyLibrary*` arms
      (`src/app/shell_library.rs` if #633 split it out, else
      `src/app/shell_browser.rs`), deriving nothing else — the item is
      component-resolved. Verify: `rtk cargo check -p mbv`; split if the file
      exceeds 800 lines.
- [x] 3.2 Add the dispatch arm in `src/app/shell_messages.rs` so the variant
      reaches 3.1. Verify: `rtk cargo check -p mbv`.
- [x] 3.3 If `make-shell-dispatch-exhaustive` has landed, add
      `EmbyLibraryContextMenu` to its exhaustive match. Verify:
      `rtk ast-grep scan` clean.

## 4. Claim `.` and `/` in the Music component (D2, D3)

- [ ] 4.1 In `MusicWorkspaceComponent::handle_key`, add album-level arms
      (guarded `track_cursor.is_none()`, placed next to the `EmbyLibrary*`
      `Ctrl+*` arms, before the `[`/`]` group-pill arms): `.` →
      `Msg::Shell(ShellRequest::EmbyLibraryContextMenu { item: self.selected_item()? })`;
      `/` → `Msg::Shell(ShellRequest::OpenInlineSearch)`. Verify:
      `rtk cargo check -p mbv`.
- [ ] 4.2 Confirm the existing focused-track `.` arm
      (`music_workspace.rs:278`, `MusicTrackContextMenu`) still wins when
      `track_cursor.is_some()`, and `/` is unclaimed while a track is
      focused. Verify: test 6.1.

## 5. Claim `.` and `/` in the TV component (D2)

- [ ] 5.1 In `TvWorkspaceComponent::handle_key`, add `.` and `/` arms before
      the letter-pill arm: `.` → `EmbyLibraryContextMenu { item }` from
      `self.selected_item()` (series-list selection authoritative even with
      the Episodes pane focused, per #633's `EmbyLibrary*` arms); `/` →
      `OpenInlineSearch`. Verify: `rtk cargo check -p mbv`.

## 6. Tests

- [ ] 6.1 `src/app/components/music_workspace_component_tests.rs`:
      `dot_on_album_emits_library_context_menu`,
      `dot_with_track_focus_emits_track_context_menu`,
      `slash_on_album_emits_open_inline_search`,
      `slash_with_track_focus_is_unclaimed`,
      `dot_empty_list_is_unclaimed`. Verify: `rtk cargo nextest run -p mbv`.
- [ ] 6.2 `src/app/components/tv_workspace_component_tests.rs`:
      `dot_emits_library_context_menu`,
      `slash_emits_open_inline_search`,
      `dot_with_episode_focus_targets_series`. Verify:
      `rtk cargo nextest run -p mbv`.
- [ ] 6.3 Model tests mirroring the #633 `shell_browser`/`shell_library`
      tests: driving `.` through the mounted Music and TV components opens a
      context menu for the resolved item; driving `/` mounts the inline
      search for the selected library. Verify: `rtk cargo nextest run -p mbv`.
- [ ] 6.4 Render test: with the inline search mounted over a Music library
      and over a TV library, the underlying workspace list does not
      underpaint (the `project_inline_search_active` / `inline_search_active`
      flag is honored on both surfaces — see design Risks). Add the one guard
      in the workspace `view` if the test shows bleed. Verify:
      `rtk cargo nextest run -p mbv`.

## 7. Close out

- [ ] 7.1 Update the Music and TV workspace rows in
      `docs/architecture/interactive-surface-ledger.md` (`.` and `/` now
      owned).
- [ ] 7.2 Re-read `render/components/help.rs` `.` "Context menu" (Global) and
      `/` "Search library" (Library) lines — confirm they now read correctly
      for Music/TV. Adjust only if wrong.
- [ ] 7.3 Verify: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`
      (`no-raw-fallback-variants`, `no-second-router-site` green),
      `rtk cargo fmt`, `rtk make check-code-file-lines` all pass.
- [ ] 7.4 Sync the `service-browse-dispatch` delta into
      `openspec/specs/service-browse-dispatch/spec.md` and archive the
      change. Comment the outcome on #636 and close it.
