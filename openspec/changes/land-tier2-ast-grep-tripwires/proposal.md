# Proposal

## Why

The Tier-1 ast-grep tripwires landed in a63c77723 are zero-hit rules. Issue #956 tracks the
Tier-2 rules that fire today — panic macros in production code, wildcard arms in dispatch
matches, and the residual test sleeps — and they cannot land until every site is triaged and
the CI tree is wired into the build.

## What Changes

- Two new CI-tree rules with two-sided fixtures: `mbv-no-panic-macro-production` (panic-family
  macros outside test items, with the match-arm `unreachable!()` invariant form excluded
  structurally) and `mbv-dispatch-no-wildcard-arm` (`_` wildcard arms in `src/app/dispatch/`
  matches, test items excluded).
- The panic census triaged: the 35 deliberate invariant arms are covered by the structural
  exclusion, and the 6 real production sites degrade gracefully instead of crashing (help
  label fallback, theme invariant arm, logged panel-mount failure, rejected queue op, logged
  no-op, neutral settings label).
- The wildcard census triaged: the four `Command` sub-dispatchers in `src/app/dispatch/action.rs`
  take typed sub-enums so their matches are exhaustive, seven `Option`/enum matches name their
  variants, and the ten open-key-space or index sites carry a per-instance
  `// ast-grep-ignore:` comment with its reason.
- The three sleep residuals triaged: the remote-player seed read takes its deadline as a
  parameter and the test orders its peer thread on a channel; the Audiobookshelf playback test
  waits for the outcome on a condvar instead of polling; the bounded hang-simulator variant
  keeps its sleep under a per-instance ignore with its reason.
- The two pre-existing crate-scoped rules gain their `ignores` test-path globs, so every
  CI-tree rule scopes its own paths and one plain `ast-grep scan .` covers the tree.
- The root `sgconfig.yml` merges both rule trees and their fixtures.
- The build workflow runs the fixture check and the scan: warning findings print and pass,
  error findings fail the step.

## Capabilities

### New Capabilities

- `ast-grep-tripwires`: the repo-specific ast-grep rule set, its fixtures, its suppression
  policy, and the build step that runs it.

### Modified Capabilities

None. The triaged sites conform to requirements their capabilities already state (named log
events, surfaced rejections); no existing requirement text becomes wrong.

## Impact

- `rules/ast-grep-rules-ci/`: two new rules, two new fixture files, `ignores` globs on the two
  pre-existing crate-scoped rules.
- `sgconfig.yml` (new, repo root): merges both rule trees and both fixture directories.
- `.github/workflows/build.yml`: `ast-grep` in the build job's package list, one tripwire step
  after Lint.
- Panic fixes: `crates/mbv-render/src/components/help.rs`,
  `crates/mbv-theme/src/palette_json.rs` (test-only module), `src/app/shell/chrome_panels.rs`,
  `crates/mbv-daemon/src/control.rs`, `crates/mbv-daemon/src/ws.rs`,
  `crates/mbv-ui-model/src/settings.rs`.
- Wildcard fixes: `src/app/dispatch/action.rs` and six further dispatch files.
- Sleep fixes: `crates/mbv-remote-player/src/connect.rs` + `connect/handshake.rs` +
  `connect/tests.rs`, `crates/mbv-net/src/mock_http.rs`,
  `crates/mbv-audiobookshelf/tests/audiobookshelf/playback.rs`, and the per-instance ignore in
  `crates/mbv-net/src/bounded.rs`.
