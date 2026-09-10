## 1. Classify and characterize

- [ ] 1.1 Classify every production call site: run `rg -n "SURFACE_FOCUSED|SURFACE_ACCENT_SOFT|SURFACE_BACKDROP|list_selected_row_bg|resolve_surface_focus" src/app --type rust` and assign each site (excluding `theme/`, the two re-export modules, and test modules) to one of the four nesting levels from design.md D1 — column/pane, panel, recess, dialog — or mark it `unchanged` with a reason. Verify: the list covers every site the command returns and the 11 direct `SURFACE_FOCUSED` names are all present (10 in the nine modal frames, 1 in `shell_playback.rs`).
- [ ] 1.2 Land characterization buffer tests for the appearance that must not change, before any role edit: the library column gutter, the library panel body, and the absence of a selected-row highlight when the library is unfocused, at wide `LibraryOnly` and wide `Both` plus the narrow library width. Verify: `cargo nextest run -p mbv` passes and the new tests pin the unfocused values.

## 2. Role taxonomy

- [ ] 2.1 Add `SURFACE_DIALOG` (`#3c4841`) to `theme/mod.rs` and move all 10 direct `SURFACE_FOCUSED` names inside the nine modal frames (`confirm_modal`, `library_routes`, `multiselect`, `feeds_manage` ×2, `selection_modal` ×2, `remote_reanchor`, `daemon_lost_modal`, `playlists`) to it. Verify: `cargo check -p mbv` passes and the existing modal buffer tests pass with no expected-value edit, proving no rendered change.
- [ ] 2.2 Add `SURFACE_COLUMN_FOCUSED` (`#3c4841`), `SURFACE_COLUMN_RESTING` (`#2d353b`) and `resolve_surface_column(focused)`, renaming today's `SURFACE_FOCUSED`/`SURFACE_RESTING` to the panel-role names (`SURFACE_PANEL_FOCUSED`/`SURFACE_PANEL_RESTING`) with `resolve_surface_focus` returning them. Verify: a theme unit test asserts each resolver returns its level's focused/resting pair, that the panel's two arms differ, that the column's two arms differ, and that `SURFACE_DIALOG` differs from `SURFACE_PANEL_FOCUSED`.
- [ ] 2.3 Retarget the panel role's focused arm to `#48584e` and delete `SURFACE_ACCENT_SOFT`, moving its three production sites to the panel role: the queue panel (`widgets.rs`), the TV episode box (`tv_wide.rs`), the Music track box (`wide_hero.rs`). Verify: `cargo check -p mbv` passes with `SURFACE_ACCENT_SOFT` gone, and the queue/episode/track buffer tests pass with their expected role updated in place.
- [ ] 2.4 Move the now-playing strip (`shell_playback.rs`) off the direct `SURFACE_FOCUSED` name onto the panel resolver. Verify: the queue-focused playback strip buffer assertion observes the panel-focused role, and no direct `SURFACE_FOCUSED` name remains outside `theme/`.
- [ ] 2.5 Point `LeftPaneFocus::Workspace(held)` at `resolve_surface_column(held)` instead of the panel lever, leaving `ReadOnly` on the panel resting role. Verify: `tests_wide_hero_pane_characterization` and the TV/ABS pane tests pass with **no** expected-value edit — the proof that the pane kept its values and did not join the panel role.

## 3. The column surface follows focus

- [ ] 3.1 Add `right_focused` to `FrameChromeGeometry`, derived in `arrangements/chrome.rs` from the `PanelFocus` it already receives, false when the right column is not visible. Verify: chrome-geometry unit tests cover `LibraryOnly` (focused), `Both` with library focus, `Both` with queue focus, `QueueOnly`, and the narrow mini-view widths.
- [ ] 3.2 Resolve the right-column backdrop fill (`render_legacy_backdrops` right arm) from `right_focused`. Verify: a new buffer test asserts the gutter around a focused library panel resolves to the column-focused role at wide `LibraryOnly` and wide `Both`, and to the column-resting role once the queue takes focus.
- [ ] 3.3 Move the two in-screen column-surface insets onto the column resolver: the narrow Home spacer (`home.rs`) and the wide Music browser panel (`music_wide.rs`). Verify: the Home narrow/wide spacer tests and the wide Music buffer test observe the column-focused role while focused and the column-resting role when unfocused.

## 4. The selected row follows its containing surface

- [ ] 4.1 Replace `list_selected_row_bg()` with a focus-resolved resolver, then delete `SelectedRowSurface`, the `selected_surface` field on both paint policies, and every call-site argument. Verify: `cargo check -p mbv` passes with `SelectedRowSurface` gone from the workspace.
- [ ] 4.2 Restate the four tests that pin a selected row against its panel (`tests_library_characterization`, `tests_home_characterization`, `tests_feeds`, `wide_tv_tests`) to assert the row matches the surface containing its panel **and** differs from the panel body. Verify: those tests pass and each contains both assertions.
- [ ] 4.3 Thread the focus bit into the legacy row builders (`build_list_row_spans`, `item_cell_spans` in `list_rows.rs` and their `plain_rows`/`list_letter_groups` call sites) so a selected cell derives from the containing surface. Verify: a buffer test shows the unfocused legacy library row byte-identical to today and the focused row matching the column-focused role.

## 5. Verify

- [ ] 5.1 Add the breakpoint coverage the spec's two new scenarios need: one assertion per breakpoint (wide `LibraryOnly`, wide `Both` with each column focused, narrow) that a focused panel's fill differs from the surface it sits on and that a focused list's selected row matches that surface. Verify: the new assertions pass and no test was deleted or weakened — list any expected value that changed and why.
- [ ] 5.2 Run the gates: `cargo nextest run -p mbv`, `cargo check -p mbv`, `cargo clippy --workspace --all-targets`, `cargo fmt --all`, `ast-grep scan`. Verify: all pass with no new warnings.
