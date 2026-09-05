# Fix: Router Swallows Its Own Overlay Keys; Globals Fire While Typing

## Why

The just-archived `migrate-tui-to-tuirealm` and `remove-legacy-keyboard-endpoint`
campaigns landed `UiRoot` as the central Keyboard Router (ADR 0023) and moved
all surface interpretation into typed component requests. A post-archive
spot-check found six live regressions in the router's policy and snapshot:

- **CRITICAL — every blocking overlay is keyboard-dead.** `resolve_router_outcome`
  returns `Swallow` for any chord when `snapshot.blocking_overlay_open` is true,
  and `apply_router_outcome` discards the focused leaf's message on `Swallow`.
  The focused leaf **is** the blocking overlay (`Confirm`, `DaemonLost`,
  `RemoteReanchor`, `SavePlaylist`, `ContextMenu`, `SelectionModal`,
  `Multiselect`, `LibraryRoutes`, `FeedManage`). The leaf's own typed request
  (`ConfirmIntent::Accept`, `SelectionModalActivate`, `MultiselectCommit`, …)
  is silently discarded. Press `c` to open Clear-Queue confirm → press `y` to
  accept → nothing happens. Mouse is accepted-broken (D16), so these are
  unescapable. The routing matrix at
  `tests_routing_matrix.rs::blocking_overlay_swallows_*_chord` (96/112/131)
  asserts the bug.
- **CRITICAL — global chords fire while typing.** `Quit` (`q`), `PanelModeCycle`
  (`x`), `Visualizer` (`v`), `LibraryTabJump` (`1`–`9`), `NextLibraryTab`
  (`Tab`), `PreviousLibraryTab` (`BackTab`) all gate only on
  `!blocking_overlay_open`. The search sidebar, inline library search, and
  settings setup form are focused leaves, not blocking overlays. Type
  `queen` in search → `q` resolves to `Command::Quit` and the app exits. Same
  for `x`, `v`, digits, Tab. Worst in the Emby setup form while typing a URL
  or password. `RouterSnapshot` carries no `text_entry_focused` fact.
- **HIGH — `y` on four confirms is a no-op.** `shell_modal_actions.rs:15`
  re-encodes `ConfirmIntent::Accept` as `KeyCode::Enter`, but the action
  handler matches only `Char('y')` for `RemoveActiveQueueItem`,
  `RemoveFeedSubscription`, `SaveOverwritePlaylist`, `DeletePlaylist`.
  `confirm_key_dismisses` returns `true` for these actions on any key, so the
  modal closes and the action does not run.
- **HIGH — Space/Esc dead under UiRoot focus.** `shell.rs:176` sets
  `arm_first_press = focus() != Some(&ComponentId::UiRoot)`. Several layouts
  fall back to UiRoot focus (narrow TV, narrow/non-grouped Music, Emby
  podcast libs, feed home-video groups). The first Space/Esc never arms the
  double-tap timer; `command_for_policy` returns `None` without it; the
  playback leaf never receives its second-press claim.
- **MEDIUM — four red tests attributed to a pre-existing baseline are
  actually broken by this branch.** Three
  `tests_conformance_matrix` rows report "Podcasts" with a blank buffer;
  `shell_music_workspace::music_resize_push_uses_current_frame_geometry` fails
  because `dce4389d` removed the legacy underpaint that published
  `wide_music_track_hitmap` for the shell to push. The commit messages that
  introduced these failures were landed in the just-archived campaign.
- **LOW — `LibraryTabJump` ignores modifiers.** `key_policy.rs:97` matches any
  `Char('1'..='9')` regardless of modifiers, and is ordered before `alt_swallow`,
  so `Ctrl+1` / `Alt+1` jump tabs. `Quit`, `Visualizer`, and `PanelModeCycle`
  all check `mods.is_empty()`; this one was missed.

The two CRITICALs are the same root cause: the router's `blocking_overlay_open`
fact is the only "don't reach the global layer" condition. It is over-applied
(it swallows the overlay's own keys) and under-applied (text fields aren't an
overlay). One new snapshot field, `text_entry_focused`, plus a single rule
("do not swallow the focused leaf's own typed request") closes both.

## What Changes

- **`RouterSnapshot` gains `text_entry_focused: bool`** (set true for the
  search sidebar, inline library search, and the settings setup form's
  text-input fields) and the router's swallow rule changes: a chord that
  matches an active-leaf request stands even when the policy declined to bind
  it; a global binding (Quit/PanelModeCycle/Visualizer/LibraryTabJump/…)
  resolves to `Command` only when the focused leaf is not a text-entry
  component. The blocking overlay's own keys are no longer silenced: when the
  focused leaf is the overlay itself, the policy's `Swallow` is replaced by
  `FallThrough` so the leaf's `ConfirmIntent::Accept` etc. stand.

- **`Accept` / `Cancel` / `Save` / `Discard` / `Dismiss` re-encoded as
  `Char('y')` / `Char('n')` / `Char('s')` / `Char('d')` / `Char('x')` in
  `handle_confirm_intent`** (matching the action handler's existing
  `Char('y')`-only arms and the `confirm_key_dismisses` table) and Enter
  continues to dispatch Accept for the actions whose handler already accepts
  it (`RemoveEmby` / `ReplaceEmby` / `RemoveAudiobookshelf` /
  `ReplaceAudiobookshelf`).

- **First-press timer arms unconditionally**, regardless of focus. UiRoot focus
  is a component-boundary fact that the router does not need to know about
  for the double-tap timer — the timer arms on any first eligible Space/Esc
  press and the second press within 300 ms claims playback.

- **Restores the wide-music track hitmap underpaint** that `dce4389d` removed,
  scoped to the leaf's own painted geometry so the test passes again without
  re-introducing a global hit map. Resolves the `shell_music_workspace::
  music_resize_push_uses_current_frame_geometry` failure and one of the three
  conformance-matrix failures.

- **Updates the three conformance-matrix rows** to assert the post-fix
  behavior (the rendered buffer must include the leaf's painted surface, not
  a blank buffer for a leaf that the component owns).

- **`LibraryTabJump` binding matches `mods.is_empty()` and is reordered after
  `alt_swallow`** so `Ctrl+1` / `Alt+1` do not jump tabs.

- **No new delta spec.** The behavior the router asserts already lives in
  `interactive-component-framework` (the spec the campaign just merged). This
  change updates that spec to refine the text-entry rule; no new capability is
  introduced.

## Capabilities

The change updates the existing `interactive-component-framework` capability
to refine the "supported mouse paths" and "input precedence preserved" rules
with the text-entry-facts. No new capability, no new main spec.

## Out of Scope

- Mouse interaction for the deferred surfaces (Music workspace, blocking
  modals, playback prompts) — D16 acceptance, unchanged.
- Per-key user-configurable bindings — ADR 0002 deferred, unchanged.
- The four "pre-existing baseline" red tests other than the three
  conformance-matrix rows and the one music_resize row. The other test
  (the third conformance-matrix `matrix_all_surfaces_paint_one_pill_bar_with_one_parent_spacer`)
  is a separate layout-conformance regression that needs its own analysis
  before a fix is proposed; the user is not in a position to ship a guess.
