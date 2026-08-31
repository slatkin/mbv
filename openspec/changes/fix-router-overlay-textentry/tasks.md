# Tasks: Router Swallows Its Own Overlay Keys; Globals Fire While Typing

## 1. Router snapshot

- [x] 1.1 Add `text_entry_focused: bool` to `RouterSnapshot` in
  `src/app/key_policy.rs`. Update the shell's `router_outcome` in
  `src/app/shell.rs` to set the field by inspecting the focused leaf:
  - `Some(ComponentId::Overlay(OverlayId::Search))` → true (search sidebar)
  - `Some(ComponentId::InlineSearch(_))` → true (inline library search)
  - `Some(ComponentId::Overlay(OverlayId::Settings))` → true (settings
    sidebar; the sidebar's form fields own focus)
  - everything else → false
  Add a `Default`-deriving placeholder; existing tests that build
  `RouterSnapshot::default()` keep working.

- [x] 1.2 Update `resolve_router_outcome` in `src/app/router.rs` with
  two new rules:
  - **Never swallow the focused leaf's own typed request.** When the
    policy would return `Swallow` and the focused leaf is a blocking
    overlay (i.e. `snapshot.blocking_overlay_open` is true and the
    focused leaf id is one of the blocking-overlay `ComponentId`s),
    return `FallThrough` instead. The leaf's request stands.
  - **Global bindings require no text entry focused.** When the policy
    matches a global binding whose `KeyPolicyOwner::Sub` is
    `ComponentId::UiRoot` and `snapshot.text_entry_focused` is true,
    return `FallThrough` instead of `Command`. The leaf's character
    input stands.
  The `blocking_overlay_open` "catch-all" rules stay; they still
  discard the focused leaf's message when no overlay is mounted.
  The behavior is: "the focused leaf is the overlay, the policy
  declines or the overlay's own request is silent → the leaf's
  request stands."

- [x] 1.3 Update `KeyPolicyBinding::LibraryTabJump` in
  `src/app/key_policy.rs:97` to also require `chord.mods.is_empty()`.
  Move the `library_tab_jump` entry in `KEY_POLICY` so it is
  ordered **after** `alt_swallow` — `Alt+1` is swallowed, not
  jumped. Update the policy-name comment table.

## 2. Confirm-intent key translation

- [x] 2.1 Update `handle_confirm_intent` in
  `src/app/shell_modal_actions.rs:14-28` to re-encode `Accept` as
  `KeyEvent::new(KeyCode::Char('y'), NONE)`, matching the action
  handler's `Char('y')`-only arms. `Cancel` (`Esc`), `Save`
  (`Char('s')`), `Discard` (`Char('d')`), `Dismiss` (`Char('x')`)
  stay unchanged. The actions whose handler already accepts Enter
  (`RemoveEmby`, `ReplaceEmby`, `RemoveAudiobookshelf`,
  `ReplaceAudiobookshelf`) continue to accept Enter via the
  existing `confirm_key_dismisses` table; the `Char('y')`
  re-encoding covers the four actions that previously did not.

## 3. Double-tap timer

- [x] 3.1 Drop the `arm_first_press = focus() != Some(&ComponentId::UiRoot)`
  gate in `src/app/shell.rs:176` (or wherever the assignment lives
  after the snapshot population). The first eligible Space/Esc
  press arms the timer regardless of focus; the second press
  within 300 ms is still claimed by `command_for_policy` when
  `snapshot.space_double_tap` / `snapshot.esc_double_tap` is true.
  Update the comment to document the new rule.

## 4. Wide-music track hitmap + conformance-matrix

- [x] 4.1 Publish `wide_music_track_hitmap` from the Music workspace
  leaf's `view` (or from a `set_content`-time shell reader) so the
  shell can push it. This is not a return to a global hit map;
  the leaf owns the geometry and the shell reads a narrow
  projection through a known accessor. Place the publication
  where the leaf's own geometry is computed and the
  `wide_music_track_hitmap` is the same shape the shell currently
  reads. Verify `shell_music_workspace::tests::music_resize_push_uses_current_frame_geometry`
  passes after the publication lands.

  **Already satisfied (per user).** The Music-wide render component
  (`src/app/render/components/music_wide_tracks.rs:120`) already
  pushes `(row_rect, ti)` into `layout.wide_music_track_hitmap` from
  the leaf's own painted geometry, and `shell_music_workspace.rs:214`
  reads that projection at push time.
  `music_resize_push_uses_current_frame_geometry` passes at baseline
  (`rtk cargo nextest run -p mbv -E 'test(music_resize_push_uses_current_frame_geometry)'`
  → 1 passed). The `dce4389d` underpaint removal did not regress this
  path. No code change required.

- [x] 4.2 Update the three `tests_conformance_matrix` rows that report
  a blank buffer for the leaf the component owns to assert the
  post-fix behavior (the rendered buffer must include the leaf's
  painted surface, not a blank buffer for a leaf the component
  owns). The third row
  (`matrix_all_surfaces_paint_one_pill_bar_with_one_parent_spacer`)
  is a separate layout-conformance regression and is out of
  scope for this change.

  **No change required.** Scouted `src/app/render/tests_conformance_matrix.rs`:
  the Music/Home/Feeds legs already render through mounted components
  and assert non-blank buffers (converted in archived commits
  `971106ab`/`2320cc7e`/`c455e28c`). No row currently asserts a blank
  buffer for a component-owned leaf, and
  `matrix_all_surfaces_paint_one_pill_bar_with_one_parent_spacer`
  passes at baseline (not `#[ignore]`d). The precondition this task
  describes no longer exists in the tree.

## 5. Routing matrix

- [x] 5.1 Rewrite the three `blocking_overlay_swallows_*_chord` tests
  in `src/app/tests_routing_matrix.rs:96,112,131` to assert the
  post-fix behavior: the leaf's `ConfirmIntent::Accept` /
  `ConfirmIntent::Dismiss` stands instead of being discarded.
  The "blocking overlay `Swallow`s an unbound chord" invariant
  is moved to a new test that uses `fold_tick_with_outcome` to
  inject a `Swallow` outcome explicitly, preserving the
  `apply_router_outcome` fold semantics as the policy's
  "blocking overlay, no leaf request" path.

- [x] 5.2 Add new matrix rows pinning the text-entry rule:
  - `quit_global_does_not_fire_in_search_sidebar` — focused leaf is
    `Overlay(Search)`, key is `q`, snapshot has
    `text_entry_focused: true`, outcome is `FallThrough`.
  - `panel_mode_cycle_global_does_not_fire_in_search_sidebar` —
    same shape, key is `x`.
  - `library_tab_jump_does_not_fire_in_search_sidebar` — same
    shape, key is `Char('1')`.
  - `quit_global_does_not_fire_in_inline_search` — focused leaf is
    `InlineSearch(_)`, same shape.
  - `library_tab_jump_with_modifiers_is_swallowed` — focused leaf
    is `Browser(_)`, key is `Char('1')` with `ALT`, outcome is
    `Swallow` (because the policy's `alt_swallow` entry swallows
    it first).

- [x] 5.3 Add new matrix rows for the ConfirmIntent re-encoding:
  - `confirm_accept_re_encodes_to_y_chord` — the action handler's
    `apply_confirm_action(RemoveActiveQueueItem, Char('y'))` path
    is invoked. The test confirms the re-encoding from
    `ConfirmIntent::Accept` reaches the right action.

## 6. Spec delta

- [x] 6.1 Update the `interactive-component-framework` spec's
  "Input precedence preserved through focus and subscriptions"
  requirement with a new scenario: "A global binding does not
  fire when the focused leaf is a text-entry component." Add a
  clarifying sentence to the requirement body: "Global bindings
  (those owned by `ComponentId::UiRoot`) require that the focused
  leaf is not a text-entry component; otherwise the leaf's
  character input stands as a typed request."

  **Already authored & committed** in `de45cdb8` at
  `openspec/changes/fix-router-overlay-textentry/specs/interactive-component-framework/spec.md`:
  the MODIFIED requirement carries the clarifying sentence and the
  new scenario. `openspec validate fix-router-overlay-textentry
  --strict` → valid. Delta merges to the main spec at archive/sync.

## 7. Verification

  **Note (orchestrator):** tasks 1.1–1.3 and the conformance-matrix
  fixes landed upstream in `de45cdb8` before this change's HEAD, so
  the baseline is cleaner than the plan predicted. Observed values
  below supersede the plan's estimates.

- [x] 7.1 Run `rtk cargo fmt --all -- --check` — 0 diff. ✅
- [x] 7.2 Run `rtk cargo check -p mbv` — 0 errors (34 prod
  warnings, all pre-existing; plan estimated 25). ✅
- [x] 7.3 Run `rtk cargo nextest run -p mbv --no-fail-fast` —
  1164 passed, **1 failed**: `browser_local_navigation_mirrors_legacy_flat_movement`
  (pre-existing baseline, tracked as #632; the conformance-matrix +
  `music_resize` rows the plan expected to fix were already green
  at HEAD). No new failures. ✅
- [x] 7.4 Run `rtk cargo clippy --workspace --all-targets` — 0
  errors (111 warnings, all pre-existing; plan estimated 85). ✅
- [x] 7.5 Run `rtk make check-code-file-lines` — PASS. ✅
- [x] 7.6 Run `rtk ast-grep scan` — 66 diagnostics (pre-existing
  screen-boundary baseline, unchanged; plan estimated 69). ✅
- [x] 7.7 Run `rtk ast-grep test` — 7 passed, 0 failed. ✅
- [x] 7.8 Squash all fix commits into one, per user request.

  **Deferred to PR time (user decision, 2026-08-31).** The fix
  commits (`42860bbb` code, `f48b5e48` tests) are non-adjacent —
  the concurrent `sync-interactive-surface-docs` (#614) committed
  to `feat/migrate-tui-to-tuirealm` in the same session and its
  commits are interleaved. A rebase to squash would rewrite another
  agent's history on a shared long-lived branch. Since the branch
  is itself squash-merged at PR time, a per-commit squash inside it
  adds little; history collapses when the branch PR is cut.
- [x] 7.9 Update the `interactive-component-framework` spec
  (task 6.1) and re-run `openspec validate --specs` — passes
  (66 passed, 0 failed). ✅

## 8. Final

- [x] 8.1 Mark this change complete. The fixes are filed against
  `fix-router-overlay-textentry`; the just-archived
  `migrate-tui-to-tuirealm` and `remove-legacy-keyboard-endpoint`
  are not reopened.
