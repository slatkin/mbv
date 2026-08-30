## 1. Make the top-level dispatch exhaustive

- [x] 1.1 **Superseded — no bug to characterize.** The audit premise (App-side
      episode mirror going stale) was already resolved by commit `0227d748`
      (migration task 5.3d.11 U2): the component solely owns `episode_selection`
      / `episode_filter` and `AudiobookshelfPodcastEpisodeIntent` resolves from
      the component. Regression coverage already exists
      (`shell_audiobookshelf_podcast_tests.rs:103,187`). No new test. Verify:
      confirm those two tests exist and pass; state that the Transition `Msg` is
      correctly inert.
- [x] 1.2 Restructure `Model::handle_terminal_message`
      (`src/app/shell_messages.rs:6`) to destructure `Msg::Shell(request)` and
      dispatch to an inner match over `ShellRequest` with **no wildcard arm**.
      Keep the existing `Msg::Playback` / `Msg::Service` arms as they are; the
      outer `Msg` match keeps its wildcard only if a genuinely unproduced
      variant remains, and the misleading comment at `:482` is corrected or
      deleted. Verify: `rtk cargo check -p mbv` fails, listing every unhandled
      `ShellRequest` variant. **Record that list in this file before fixing
      it** — it is the inventory the rest of the change works from.

      Inventory (exactly three variants surfaced by the exhaustive match; no
      others): `AudiobookshelfPodcastEpisodeTransition`, `DismissSettings`,
      `SelectionModalRefresh`. All three are documented no-op arms (see 1.3/1.4);
      none required a handler.
- [x] 1.3 Triage each variant the compiler named. For each: wire a handler, or
      add an explicit no-op arm with a comment naming the reason and the issue
      that owns it. No variant may be left matching a wildcard. Verify:
      `rtk cargo check -p mbv` is clean and every arm is either a handler or a
      commented no-op.
- [x] 1.4 **Reduced to a documented no-op arm** for
      `ShellRequest::AudiobookshelfPodcastEpisodeTransition` (emitted at
      `components/audiobookshelf_podcast.rs:217-249`, five sites). Reason:
      `AudiobookshelfPodcastComponent` owns `episode_selection` /
      `episode_filter` and mutates them locally in `handle_key` before emitting
      the `Msg`; the Intent resolves from the component, not App state (commit
      `0227d748`, migration task 5.3d.11 U2). The shell has no effect to run.
      Verify: `rtk cargo nextest run -p mbv` stays green.

## 2. Triage the remaining wildcard arms

- [x] 2.1 For each of the twelve other wildcard arms in the shell dispatch —
      `shell.rs:237`, `shell_browser.rs:100`, `shell_feeds_manage.rs:116,128`,
      `shell_root.rs:42`, `shell_overlays_menus.rs:257,274,473`,
      `shell_tv_workspace.rs:39,46`, `shell_playlists.rs:153`,
      `shell_home.rs:52` — record what enum it matches and whether it can hide
      an unhandled variant. Verify: a stated table, one row per arm.

      | # | file:line | enum matched | how called | verdict |
      |---|-----------|--------------|------------|---------|
      | 1 | `shell.rs:239` | `(KeyCode, Option<Command>, &RouterOutcome)` tuple | inside `update_double_tap_state`; not a request dispatch | PROVABLY DEAD (tuple over open sets; wildcard = "no double-tap timer to touch", cannot hide a `ShellRequest`) |
      | 2 | `shell_browser.rs:100` | `ShellRequest` | `handle_browser_request`; `shell_messages.rs` routes only the `request @ (BrowserActivate/Play/Enqueue/ToggleWatched/ContextMenu/Shuffle/Refresh/Rescan/Back/CycleLetterPill)` OR-group + `BrowserCursorIndex`; every one has an arm | PROVABLY DEAD |
      | 3 | `shell_feeds_manage.rs:117` | `KeyCode` (std enum) | `handle_feeds_manage_list_key`, crossterm compat bridge | PROVABLY DEAD (unbound key = no-op; not a request enum) |
      | 4 | `shell_feeds_manage.rs:130` | `KeyCode` (std enum) | `handle_feeds_manage_form_key`, crossterm compat bridge | PROVABLY DEAD (unbound key = no-op; not a request enum) |
      | 5 | `shell_root.rs:43` | `ComponentId` | `render_overlay_stack`, iterating mounted overlay ids | PROVABLY DEAD (non-overlay ids are not painted by the overlay stack; not a request enum) |
      | 6 | `shell_overlays_menus.rs:260` | `SelectionModalSource` (closed: Series/Album/Podcast/Book) | inside `handle_selection_modal_request` filter branch | PROVABLY DEAD (only Series/Podcast modals carry a filter; Album/Book have no filter-selection effect — every real case handled) |
      | 7 | `shell_overlays_menus.rs:281` | `ShellRequest` | `handle_selection_modal_request`; callers pass only `DismissSelectionModal` / `SelectionModalFilterSelected` / `SelectionModalRefresh` (`shell_overlays_modals.rs:163`) / `SelectionModalActivate` (`shell_messages.rs` OR-group); every one has an arm | PROVABLY DEAD |
      | 8 | `shell_overlays_menus.rs:481` | `ShellRequest` | `handle_library_routes_request`; `shell_messages.rs` routes only `request @ (LibraryRoutesEnter \| LibraryRoutesEsc)`; both have an arm | PROVABLY DEAD |
      | 9 | `shell_tv_workspace.rs:39` | `ShellRequest` | inner match, guarded by the outer arm to `TvMoveRows/TvMoveColumn/TvJumpCursor/TvActivate/TvBack/TvCycleLetterPill`; pure cursor moves need no App effect | PROVABLY DEAD (closed set from the outer guard) |
      | 10 | `shell_tv_workspace.rs:50` | `ShellRequest` | `handle_tv_request`; `shell_messages.rs` routes only the `request @ (TvMoveRows/TvMoveColumn/TvJumpCursor/TvActivate/TvEpisodeActivate/TvBack/TvCycleLetterPill/TvEpisodeMove/TvSeasonMove)` OR-group; every one has an arm | PROVABLY DEAD |
      | 11 | `shell_playlists.rs:156` | `ShellRequest` | `handle_playlists_request`; `shell_messages.rs` routes only the `request @ (PlaylistsBack/Open/Activate/Rename/Delete/Refresh \| DismissPlaylists)` OR-group; every one has an arm | PROVABLY DEAD |
      | 12 | `shell_home.rs:52` | `ShellRequest` | `handle_home_request`; `shell_messages.rs` routes only the `request @ (HomePlay/HomeEnqueue/HomeContextMenu/HomeDelete/HomeToggleWatched/HomeSectionSelected)` OR-group; every one has an arm | PROVABLY DEAD |

      No arm is CAN-HIDE-A-VARIANT. Every `ShellRequest` arm is a second
      dispatch site fed by an explicit OR-group in the now-exhaustive
      `shell_messages.rs`; the non-`ShellRequest` arms (`KeyCode`,
      `ComponentId`, tuple, `SelectionModalSource`) are closed sets whose real
      cases are all handled. Line numbers are as of the 2.2 edits.
- [x] 2.2 Make exhaustive every arm 2.1 found capable of hiding a variant;
      leave the rest with a comment stating the closed set they match and why
      the wildcard is unreachable. Do not restructure an arm that matches on a
      small closed enum where the wildcard is provably dead. Verify:
      `rtk cargo check -p mbv`; `rtk cargo clippy --workspace --all-targets`.

## 3. Record the rule

- [x] 3.1 Add the rule to the `interactive-component-framework` spec delta: a
      component's typed request has a shell handler or an explicit documented
      no-op, enforced by exhaustive matching rather than convention. Verify:
      the delta states it as a testable requirement.
- [x] 3.2 Consider whether `rules/interactive-component-boundary/` should carry
      an ast-grep rule for a wildcard arm over a request enum. Add it only if
      it can be expressed without false positives on legitimate closed-set
      matches; record the decision either way. Verify: `rtk ast-grep test`
      fixtures pass and `rtk ast-grep scan` is clean.

## 4. Close out

- [x] 4.1 Report the handler-gap inventory from 1.2 to #623 and #627, so the
      routing work is scoped against the real list rather than the audited
      sample. Verify: comment posted, variants classified as wired here vs.
      owned by #627.
- [x] 4.2 Verify the full gate: `rtk cargo check -p mbv`,
      `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
      --all-targets`, `rtk ast-grep scan`, `rtk cargo fmt`,
      `rtk make check-code-file-lines`.
