# Design

## Context

Issue #956 lands the Tier-2 tripwires: the ast-grep rules that fire today and need a triage
pass. The Tier-1 landing (a63c77723) established the two-tree layout — `rules/ast-grep-rules/`
for the repo-wide rules pi-lens dispatches live, `rules/ast-grep-rules-ci/` for the
crate- and path-scoped tripwires stock ast-grep runs — and left the CI tree unwired. The
dry-run census for the three Tier-2 rules lived outside the repo.

## Decisions

### 1. The panic rule excludes the invariant-arm idiom structurally

Of the 35 census hits, 29 are the deliberate `unreachable!()`-in-a-match-arm form the watch
item from #950 described. Excluding that form in the rule (one structural clause) removes the
batch-ignore question entirely; the 6 remaining sites were real and are fixed in code:

- `crates/mbv-render/src/components/help.rs`: `playback_label` returns `String` and falls back
  to `humanize_action_id`.
- `crates/mbv-theme/src/palette_json.rs`: `panic!` became the invariant-arm `unreachable!`
  (the file is a `#[cfg(test)]` module the path globs cannot see, so the fix uses the
  sanctioned form rather than a suppression).
- `src/app/shell/chrome_panels.rs`: a mount failure logs `chrome_panel.mount_failed` and the
  shell continues.
- `crates/mbv-daemon/src/control.rs`: a non-queue projection event logs and replies
  `QueueOpOutcome::Rejected`.
- `crates/mbv-daemon/src/ws.rs`: a non-Play event logs and returns.
- `crates/mbv-ui-model/src/settings.rs`: `label_for` falls back to `"Unknown setting"`.

No per-instance ignore comments exist for this rule. (User decision 1.)

### 2. The wildcard rule gets explicit arms, with per-instance ignores for open key spaces

The four `Command` sub-dispatchers in `src/app/dispatch/action.rs` take typed intent enums, so
each of their matches is exhaustive. The nested shape of that enum comes from the
`nest-command-by-domain` change (`openspec/changes/nest-command-by-domain/`): it renames the
four sub-enums to `*Intent` (`VisualIntent`, `PlaybackIntent`, `NavigationIntent`,
`PanelIntent`), makes `Command` the `Shell(ShellCommand)` / `App(AppCommand)` wrapper, and
gives `App::dispatch` one arm per domain. That change resolves the `App::dispatch` complexity
this tripwire gates on, because it removes the flat-variant translation the rule would have had
to police. This change follows it instead of re-deriving the typed sub-enums, and the wildcard
rule then guards the translation-free shape. Seven `Option`/enum matches name their variants.
The three `match key` sites over the 40-variant `SettingKey` enum in
`src/app/dispatch/settings.rs` classify each row once through a `SettingRowKind` function
beside the dispatch sites (`Destination`, `OwnActivation`, `ConfigToggle`, `Passive`) and
route on the kind, so a new row states its behavior once instead of once per dispatch site
(review fix of the original exhaustive variant lists, which repeated the classification in
three hand-written lists).
The remaining seven sites match an open key space or an index mapping where
a wildcard is the correct arm; each carries a per-instance directive with its reason, and
the directive must be the comment line immediately above the arm. (User decision 2.)

### 3. One plain scan, with the path scoping inside the rule files

The planned wiring was two invocations — the CI tree with four test-path `--globs`, the live
tree without them — because the sleep rule needs test files visible while the panic and
wildcard rules must not fire on test paths. Building it exposed two facts:

- ast-grep 0.44.1 honors per-rule `files:` globs but ignores negated entries inside that list;
  the per-rule exclusion mechanism is the separate `ignores:` field, which the scanner checks
  before `files`.
- With the trees in separate configs, each scan printed `unused-suppression` hints for the
  other tree's directives (the CI scan cannot see the sleep rule's `bounded.rs` directive; the
  live scan cannot see the seven wildcard directives), so every build would log eight hint
  blocks that look like defects.

So the landed wiring is one invocation: the root `sgconfig.yml` merges both rule trees and
both fixture directories, and the four CI-tree rules carry the same five `ignores:` globs
(`**/*_tests/**`, `**/tests/**`, `**/tests.rs`, `**/*_tests.rs`, `**/test_*.rs`) — the same
patterns
`.pi-lens.json` lists for the LSP, plus `**/*_tests/**` for the repo's `*_tests/` module
directories, whose files the structural test-item exclusion cannot see through their
`#[cfg(test)]` parent (a review correction confirmed on `src/app/dispatch/actions/route_tests/`).
The repo-wide rules stay unscoped, which is what keeps the
sleep rule's teeth on test files. `ast-grep scan .` over the tree is clean, and a probe run
confirmed the teeth in both directions (a production `panic!` and a fresh dispatch `_` arm
fire; a shell `_` arm and all test paths stay quiet).

### 4. The build step runs the fixtures and the scan

The scan step sits after Lint in the `build` job: `ast-grep test --skip-snapshot-tests` then
`ast-grep scan .`. All rules ship as `warning`, so findings print and the step stays green; a
rule promoted to `error` fails the step (verified exit-code semantics: a warning-only scan
exits 0, a scan with an error-severity finding exits 1). `ast-grep` comes from the Arch
`extra` repository (0.44.1-1, the census version) through the existing package list.
`ast-grep test` rides in the same step because a rule whose fixtures stop passing fails there
even when its scan is quiet — a scan alone cannot tell a broken rule from a quiet tree.
(User decision 3.)

### 5. Sleep residuals are fixed with production seams, not timing

- The remote-player seed read takes its deadline as a parameter
  (`read_playback_output_seed(reader, timeout)`, `wait_for_seed_line(reader, timeout)`,
  `run_bounded_handshake(..., seed_read_timeout)`, with `connect_stream_with_seed_timeout(...)`
  beside `connect_stream`); production passes `PLAYBACK_OUTPUT_SEED_READ_TIMEOUT`. The late
  partial-seed test signals its peer thread over a channel instead of sleeping, and runs in
  ~0.2 s instead of ~1.5 s.
- `MockHttp::wait_for_requests(count, timeout)` waits on a `Condvar` notified from
  `transmit_output`; the Audiobookshelf playback test waits for the outcome instead of polling
  (0.024 s).
- `crates/mbv-net/src/bounded.rs` keeps the hang-simulator variant's sleep under a per-instance
  directive with its reason. (User decision 5.)

### 6. No deltas on existing capabilities

The triaged sites conform to requirements their capabilities already state: the mount-failure
event follows the dot-notation naming requirement in `application-logging`, and the queue-op
rejection follows the surfaced-rejection requirements in `unified-playback-queue`. The help
and settings label fallbacks have no capability that discusses those surfaces today; adding
one-line requirements there would scatter defensive-hardening statements across capabilities
that do not own them. The graceful-degradation requirement lives in the new capability
instead: a tripwire finding is fixed with a documented fallback, not suppressed.

## Risks

- The ast-grep LSP reload behavior for the new root `sgconfig.yml` is unverified in this
  session: a warm server started before the file existed reports `unused-suppression` hints
  for directives whose rules it has not loaded. The directives are correct — the stock scan
  honors every one of them. A fresh session loads the merged config; if it still mislabels a
  directive, that is a pi-lens behavior question, not a rule defect.
- The wildcard rule matches the `_` pattern only; a named catch-all binding (`other => ...`)
  is the same catch-all and is not matched. The rule's blind-spot comment documents this.
- The panic rule's path globs cannot see a test-only module file outside a test path; the
  palette generator is the known such file and uses the sanctioned invariant-arm form.
- Arch bumps its ast-grep package with the rolling release; a rule relying on 0.44.1
  behavior (per-rule `files`/`ignores`) would surface as a CI scan finding or a fixture
  failure, not as silent breakage.
