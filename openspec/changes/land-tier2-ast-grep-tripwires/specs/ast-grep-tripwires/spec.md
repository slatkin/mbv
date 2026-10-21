# Spec Delta

## Purpose

The repo-specific ast-grep tripwires are static checks that hold the prose rules AGENTS.md and
the testing standard state but no compiler lint enforces: no panics in production code, no
wildcard arms in dispatch matches, no real sleeps in tests, no error-helper crates. This
capability owns the rule set, its fixtures, its suppression policy, and the build step that
runs them.

## ADDED Requirements

### Requirement: The rule set is fixture-covered

Every rule under `rules/` SHALL carry two-sided fixtures in its tree's rule-test directory:
cases where the rule fires and cases where an exclusion or suppression keeps it quiet. The
fixture check (`ast-grep test --skip-snapshot-tests` at the repo root) SHALL pass.

#### Scenario: Fixture run passes

- **WHEN** the fixture check runs at the repo root
- **THEN** every rule's fixtures pass, for both trees (`rules/ast-grep-rules-ci/` and
  `rules/ast-grep-rules/`)

#### Scenario: A rule drifts from its fixtures

- **WHEN** a rule edit changes what the rule matches without a matching fixture change
- **THEN** the fixture run fails

### Requirement: Panic-family macros stay out of production code

The `mbv-no-panic-macro-production` rule SHALL flag `panic!`, `unreachable!`, `todo!` and
`unimplemented!` outside test items in the `src/` and `crates/` trees. The one sanctioned form
SHALL be `unreachable!()` as the body of a match arm: it asserts a variant that cannot reach
the match, and the compiler checks the surrounding match. A `panic!` in the same arm SHALL
still be flagged.

#### Scenario: Production panic is flagged

- **WHEN** a production function body calls `panic!`
- **THEN** the scan reports `mbv-no-panic-macro-production` at that site

#### Scenario: Invariant arm is not flagged

- **WHEN** a match arm's body is `unreachable!()` over an exhaustive enum
- **THEN** the scan reports nothing at that arm

### Requirement: Dispatch matches carry no wildcard arms

The `mbv-dispatch-no-wildcard-arm` rule SHALL flag a `_` wildcard arm in a match under
`src/app/dispatch/`. A wildcard arm is correct only where the matched key space is open (a
config key, a server-side item type, a notification action, a key chord, a pill index) or the
arm is an index mapping; every such site SHALL carry a per-instance
`// ast-grep-ignore:` comment naming the rule with its reason on the comment line directly
above the directive.

#### Scenario: New wildcard arm is flagged

- **WHEN** a match under `src/app/dispatch/` grows a `_` arm without a directive
- **THEN** the scan reports `mbv-dispatch-no-wildcard-arm` at that arm

#### Scenario: Documented open key space is not flagged

- **WHEN** a wildcard arm carries its per-instance directive with a reason
- **THEN** the scan reports nothing at that arm

### Requirement: Tests contain no real sleeps

The `mbv-no-sleep-in-tests` rule SHALL flag `thread::sleep`, `std::thread::sleep`,
`tokio::time::sleep` and `tokio::time::sleep_until` inside test items. The sanctioned form
SHALL be the hang-simulator idiom: a spawned or abandoned closure whose whole body is one
sleep, kept structurally excluded. Any other sleeping test SHALL carry a per-instance
directive with its reason.

#### Scenario: Sleep in a test is flagged

- **WHEN** a `#[test]` function sleeps on the wall clock
- **THEN** the scan reports `mbv-no-sleep-in-tests` at that call

#### Scenario: Hang-simulator idiom is not flagged

- **WHEN** a closure whose sole statement is a sleep is passed to a bounded runner
- **THEN** the structural exclusion keeps the site quiet

### Requirement: Error-helper crates stay banned

The `mbv-no-error-helper-crates` rule SHALL flag `anyhow`, `thiserror` and `eyre` use across
the repo; custom domain error types remain the only sanctioned errors.

#### Scenario: Error-helper import is flagged

- **WHEN** a source file uses one of the banned error-helper crates
- **THEN** the scan reports `mbv-no-error-helper-crates` at that use

### Requirement: Suppressions are per-instance with reasons

Every suppression in the tree SHALL be a single-site `// ast-grep-ignore: <rule-id>` comment
with its reason on the comment line directly above the directive line. Batch ignores, bare
`// ast-grep-ignore:` comments without a rule id, and suppression attributes other than the
comment form SHALL NOT exist.

#### Scenario: Suppression audit

- **WHEN** the scan runs over the tree
- **THEN** every directive is consumed by the rule it names, and the scan reports no
  unused-suppression finding

### Requirement: The build runs the tripwires

The build workflow SHALL install ast-grep and run the fixture check and one plain
`ast-grep scan .` over the repo. Findings at warning severity SHALL print and leave the step
green; a finding at error severity SHALL fail the step. The invocation SHALL pass no path
arguments and no `--globs`: every CI-tree rule scopes its own paths through its `files` and
`ignores` globs, and the repo-wide rules must see test files.

#### Scenario: Warning finding keeps the step green

- **WHEN** the scan reports a warning-severity finding
- **THEN** the finding is visible in the build log and the step passes

#### Scenario: Error finding fails the step

- **WHEN** a rule's severity is `error` and the scan reports a finding for it
- **THEN** the scan exits non-zero and the step fails

#### Scenario: A test sleep reaches the scan

- **WHEN** a test file under a `tests/` path gains a wall-clock sleep
- **THEN** the same plain scan reports it, because no invocation-level glob hides test paths
