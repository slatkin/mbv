# Design: Router Swallows Its Own Overlay Keys; Globals Fire While Typing

## Context

The `migrate-tui-to-tuirealm` and `remove-legacy-keyboard-endpoint` campaigns
landed `UiRoot` as the central Keyboard Router (ADR 0023) and moved every
surface interpretation into typed component requests. A post-archive
spot-check found six live regressions in the router's policy and snapshot.

The two CRITICALs share a single root cause. The router models
`blocking_overlay_open` as the only "don't reach the global layer" fact. It
is over-applied (it swallows the overlay's own keys) and under-applied
(text fields aren't an overlay). One new snapshot field plus a single rule
closes both. The other four regressions are smaller and independent.

This design lands all six fixes as one commit (per user request) under the
new `fix-router-overlay-textentry` change. The Just-archived
`migrate-tui-to-tuirealm` and `remove-legacy-keyboard-endpoint` are not
reopened; the fixes are filed against a fresh change with the appropriate
scope.

## Root Cause (one fact, two symptoms)

`RouterSnapshot` carries one "is the leaf not a normal text/list surface"
fact: `blocking_overlay_open`. The router's `resolve_router_outcome` returns
`Swallow` whenever (a) the policy matched an entry whose `blocking: true`,
(b) the policy matched a non-blocking entry but `blocking_overlay_open` is
true and `command_for_policy` returned `None`, or (c) the policy declined
to match and `blocking_overlay_open` is true. `apply_router_outcome` then
discards the focused leaf's message on `Swallow`.

The focused leaf is the blocking overlay when an overlay is mounted (TuiRealm
moves focus onto the overlay on `active`). So the policy is silencing the
overlay's own typed requests. That is the first CRITICAL.

The same fact does not capture "the focused leaf is a text-entry field".
The search sidebar (`OverlayId::Search`), inline library search
(`InlineSearch(BrowserKey)`), and the settings setup form
(`OverlayId::Settings` when its text inputs own focus) accept `q`/`x`/`v`/
digits/Tab as character input. The router's `Quit`/`PanelModeCycle`/
`Visualizer`/`LibraryTabJump`/`NextLibraryTab` policies all gate only on
`!blocking_overlay_open`, so the gate is true and the chord resolves to
`Command` and the app quits (or panel-modes, or visualizes, or jumps tabs).
That is the second CRITICAL.

One fact, `text_entry_focused`, plus the rule "do not swallow the focused
leaf's own typed request, and do not let a global binding reach `Command`
when the focused leaf is a text-entry component" closes both.

## Decisions

### 1. `RouterSnapshot` gains `text_entry_focused: bool`

The shell sets it true when the focused leaf is one of:

- `ComponentId::Overlay(OverlayId::Search)` — search sidebar
- `ComponentId::InlineSearch(_)`
- `ComponentId::Overlay(OverlayId::Settings)` when its text-input fields
  own focus (the settings form is a sidebar; the form fields get focus
  through the form-component's own focus model)

The snapshot is a plain-data projection of focus; the shell resolves
"is the focused leaf a text-entry component" by inspecting the focused
component id. This mirrors `is_blocking_overlay_open` (a plain-data
projection of which components are mounted). No new attribute is
introduced on any component; the snapshot is the only authority the
policy reads.

The shell's `router_outcome` (in `shell.rs`) computes the new field
alongside the existing snapshot fields, at the same site that builds
`RouterSnapshot` for the policy.

### 2. Router rule changes

`resolve_router_outcome` (`src/app/router.rs`) gains two rules:

1. **Never swallow the focused leaf's own typed request.** When the policy
   would return `Swallow` and the focused leaf is a blocking overlay
   (the leaf is the overlay), return `FallThrough` instead. The leaf's
   request stands. This is the fix for the first CRITICAL.

2. **Global bindings require no text entry focused.** When the policy
   matches a global binding (Quit / PanelModeCycle / Visualizer /
   LibraryTabJump / NextLibraryTab / PreviousLibraryTab / CtrlL / F5 /
   SearchOpen / SettingsOpen / SessionsOpen / PlaylistsOpen) and the
   focused leaf is a text-entry component, return `FallThrough` instead
   of `Command`. The leaf's request stands. This is the fix for the
   second CRITICAL.

The "global binding" set is enumerated as a `KeyPolicyOwner` sub-list:
`Sub(ComponentId::UiRoot)`. The shell-level bindings (queue column width,
clear-queue prompt, panel-mode cycle, visualizer, playback) are NOT in
this set — they are owned by their own component and are the natural
target of the focused leaf's own interpretation. The shell's `Quit`/
`PanelModeCycle`/etc. are the ones that fire while typing.

### 3. ConfirmIntent key translation matches the action handler

`shell_modal_actions.rs:14-28` currently re-encodes:

- `Accept` → `KeyEvent::new(KeyCode::Enter, NONE)`
- `Cancel` → `Esc`
- `Save` → `Char('s')`
- `Discard` → `Char('d')`
- `Dismiss` → `Char('x')`

The action handler `confirm_key_dismisses` and `apply_confirm_action` only
accept `Char('y')` for `RemoveActiveQueueItem`, `RemoveFeedSubscription`,
`SaveOverwritePlaylist`, `DeletePlaylist`. Pressing `y` on a confirm
modal is the documented behavior; Enter is not. Re-encode `Accept` as
`Char('y')` to match. For actions whose handler already accepts Enter
(`RemoveEmby`, `ReplaceEmby`, `RemoveAudiobookshelf`,
`ReplaceAudiobookshelf`), Enter remains the accepted key.

### 4. Double-tap timer arms on the first eligible press unconditionally

`shell.rs:176` currently sets
`arm_first_press = focus() != Some(&ComponentId::UiRoot)`. The arm
condition is wrong: the timer's purpose is to remember "the user just
pressed Space" regardless of focus, and the second-press claim is
gated by `command_for_policy` returning `Some(TogglePlayPause)` /
`Some(Stop)` only when `space_double_tap` / `esc_double_tap` is true.
Drop the UiRoot-focus gate; arm on any first eligible press. The
router's playback gate is unchanged.

### 5. Wide-music track hitmap is published by the leaf

`dce4389d` (archived) removed the legacy underpaint that published
`wide_music_track_hitmap`. The leaf's own `view` does not republish it
under the new component-owned-geometry model. Two changes:

- The Music workspace component's `view` (or a small
  `set_content`/state-write seam the shell reads at push time) writes
  the same hitmap field the shell reads. This is not a return to a
  global hit map; the leaf owns the geometry and the shell reads a
  narrow projection through a known accessor.
- The three conformance-matrix rows that report a blank buffer for the
  Music-workspace case are updated to assert the post-fix behavior
  (the rendered buffer must include the leaf's painted surface, not
  a blank buffer for a leaf that the component owns).

The fourth conformance-matrix row (`matrix_all_surfaces_paint_one_pill_bar_with_one_parent_spacer`)
is a separate layout-conformance regression that needs its own
analysis; this change does not fix it.

### 6. `LibraryTabJump` matches `mods.is_empty()` and is reordered

`key_policy.rs:97` currently matches `Char('1'..='9')` with no modifier
check. Add `chord.mods.is_empty()` to the binding predicate, matching
`Quit`/`Visualizer`/`PanelModeCycle`. Reorder the policy entry so
`library_tab_jump` is after `alt_swallow` (so `Alt+1` is swallowed, not
jumped). The reorder is a documentation change, not a behavior change
beyond the modifier check.

## Migration Plan

Single commit, per user request. The commit:

1. Updates `RouterSnapshot` with `text_entry_focused: bool`.
2. Updates the shell's `router_outcome` to set the new field.
3. Updates `resolve_router_outcome` with the two rules above.
4. Updates `KeyPolicyBinding::LibraryTabJump` to check `mods.is_empty()`.
5. Reorders the `KEY_POLICY` list so `library_tab_jump` is after `alt_swallow`.
6. Updates `shell_modal_actions.rs` to re-encode `Accept` as `Char('y')`.
7. Removes the UiRoot-focus gate from `arm_first_press` in
   `shell.rs::update_double_tap_state`.
8. Restores the wide-music track hitmap publication through the leaf
   (or shell) seam; updates the three conformance-matrix rows to
   assert the post-fix behavior.
9. Updates `tests_routing_matrix.rs`: the three `blocking_overlay_swallows_*_chord`
   tests are rewritten to assert the post-fix behavior (the leaf's
   `ConfirmIntent::Accept` stands instead of being discarded).
10. Adds new matrix rows: text-entry focus swallows `Quit`/
    `PanelModeCycle`/`Visualizer`/`LibraryTabJump`/`NextLibraryTab`;
    the leaf's own `Char('q')` / `Char('x')` etc. stand as `Option<Msg>`.
11. Updates the `interactive-component-framework` spec's
    "Input precedence preserved through focus and subscriptions"
    requirement to state the text-entry rule.

Touched files (per file line-count cap; no file will exceed 800 lines):

- `src/app/router.rs` — `resolve_router_outcome` body change (~20 lines).
- `src/app/key_policy.rs` — `LibraryTabJump` predicate, one
  `RouterSnapshot` field, one reordered entry. Stays well under cap.
- `src/app/shell.rs` — `RouterSnapshot` field population;
  `arm_first_press` simplification.
- `src/app/shell_modal_actions.rs` — `Accept` re-encoding (~2 lines).
- `src/app/components/music_workspace.rs` and/or
  `src/app/shell_music_workspace.rs` — hitmap publication.
- `src/app/render/tests_conformance_matrix.rs` — three rows updated.
- `src/app/tests_routing_matrix.rs` — three tests rewritten, ~6 new
  rows added.
- `openspec/specs/interactive-component-framework/spec.md` — one
  scenario added to the text-entry rule.

## Risks / Trade-offs

- **A focused overlay that wants to suppress its own keys** would now have
  its requests run (the policy no longer silences them). This is the
  intended behavior — blocking overlays interpret every key they care
  about. The existing `apply_router_outcome` still drops the leaf's
  message on `Swallow`; the policy just no longer returns `Swallow`
  for the overlay's own keys.
- **`text_entry_focused` is a plain-data fact the shell computes from
  focus.** If a new text-entry surface is added, the shell must add it
  to the projection. The architectural rule stays "snapshot is the only
  authority the policy reads" — no component attribute is mirrored.
- **LibraryTabJump reorder** is a documentation change, but it also
  affects which entry's name appears in the policy listing. The
  policy-name comment table is updated.

## Verification

- `cargo fmt --all -- --check` — 0 diff
- `cargo check -p mbv` — 0 errors
- `cargo nextest run -p mbv` — exactly 4 failures (the pre-existing
  baseline minus the three conformance-matrix rows and the
  `music_resize` row that this change fixes)
- `cargo clippy --workspace --all-targets` — 0 errors
- `make check-code-file-lines` — PASS
- `rtk ast-grep scan` — 69 diagnostics (pre-existing screen-boundary
  baseline, unchanged)
- `rtk ast-grep test` — 7 passed, 0 failed

The campaign's archived final-gate statement ("exactly 4 baseline
failures") is restored as the post-fix expected result. The two
`Ok` rows that the user reported as now-passing (router first-space
+ esc-second-press) remain green.

## Out of Scope

- The third conformance-matrix row (`matrix_all_surfaces_paint_one_pill_bar_with_one_parent_spacer`).
  This is a separate layout-conformance regression that needs its own
  analysis and is not in scope for this fix.
- Mouse interaction for the deferred surfaces (D16 acceptance, unchanged).
- Per-key user-configurable bindings (ADR 0002 deferred, unchanged).
