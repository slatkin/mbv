## 1. Make the top-level dispatch exhaustive

- [ ] 1.1 Characterization test for the live bug before fixing it: in ABS
      podcast episode mode, `j`/`k` then Space acts on the episode the user
      selected, not the one App state still points at. Written against the
      component + shell seam, not a full app flow. Verify: the test is **red**
      pre-change, and that is stated.
- [ ] 1.2 Restructure `Model::handle_terminal_message`
      (`src/app/shell_messages.rs:6`) to destructure `Msg::Shell(request)` and
      dispatch to an inner match over `ShellRequest` with **no wildcard arm**.
      Keep the existing `Msg::Playback` / `Msg::Service` arms as they are; the
      outer `Msg` match keeps its wildcard only if a genuinely unproduced
      variant remains, and the misleading comment at `:482` is corrected or
      deleted. Verify: `rtk cargo check -p mbv` fails, listing every unhandled
      `ShellRequest` variant. **Record that list in this file before fixing
      it** — it is the inventory the rest of the change works from.
- [ ] 1.3 Triage each variant the compiler named. For each: wire a handler, or
      add an explicit no-op arm with a comment naming the reason and the issue
      that owns it. No variant may be left matching a wildcard. Verify:
      `rtk cargo check -p mbv` is clean and every arm is either a handler or a
      commented no-op.
- [ ] 1.4 Wire `ShellRequest::AudiobookshelfPodcastEpisodeTransition`
      (emitted at `components/audiobookshelf_podcast.rs:217-249`, five sites,
      matched only in tests). It must move the App-side episode selection so
      `AudiobookshelfPodcastEpisodeIntent`'s target resolution
      (`msg/shell.rs:254-257`) agrees with what the component shows. Cover the
      filter cycle and mode exit in the same arm. Verify: 1.1's test goes
      green; `rtk cargo nextest run -p mbv`.

## 2. Triage the remaining wildcard arms

- [ ] 2.1 For each of the twelve other wildcard arms in the shell dispatch —
      `shell.rs:237`, `shell_browser.rs:100`, `shell_feeds_manage.rs:116,128`,
      `shell_root.rs:42`, `shell_overlays_menus.rs:257,274,473`,
      `shell_tv_workspace.rs:39,46`, `shell_playlists.rs:153`,
      `shell_home.rs:52` — record what enum it matches and whether it can hide
      an unhandled variant. Verify: a stated table, one row per arm.
- [ ] 2.2 Make exhaustive every arm 2.1 found capable of hiding a variant;
      leave the rest with a comment stating the closed set they match and why
      the wildcard is unreachable. Do not restructure an arm that matches on a
      small closed enum where the wildcard is provably dead. Verify:
      `rtk cargo check -p mbv`; `rtk cargo clippy --workspace --all-targets`.

## 3. Record the rule

- [ ] 3.1 Add the rule to the `interactive-component-framework` spec delta: a
      component's typed request has a shell handler or an explicit documented
      no-op, enforced by exhaustive matching rather than convention. Verify:
      the delta states it as a testable requirement.
- [ ] 3.2 Consider whether `rules/interactive-component-boundary/` should carry
      an ast-grep rule for a wildcard arm over a request enum. Add it only if
      it can be expressed without false positives on legitimate closed-set
      matches; record the decision either way. Verify: `rtk ast-grep test`
      fixtures pass and `rtk ast-grep scan` is clean.

## 4. Close out

- [ ] 4.1 Report the handler-gap inventory from 1.2 to #623 and #627, so the
      routing work is scoped against the real list rather than the audited
      sample. Verify: comment posted, variants classified as wired here vs.
      owned by #627.
- [ ] 4.2 Verify the full gate: `rtk cargo check -p mbv`,
      `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
      --all-targets`, `rtk ast-grep scan`, `rtk cargo fmt`,
      `rtk make check-code-file-lines`.
