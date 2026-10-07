# Proposal

## Why

CI gates formatting, clippy, `cargo audit`, and nextest, but two tools from the project's own `M-STATIC-VERIFICATION` rule are absent: miri (the rule's named validator for unsafe code) and cargo-hack (feature-combination validation). Both gaps have real surface here — ~45 `unsafe` sites across 11 files, and 12 crates declaring a `test` feature with cross-enabling nothing currently builds as a matrix (issue #888).

## What Changes

- Add a miri job to the check-in gates scoped honestly: per-crate `cargo miri test` over the crates whose unsafe code is genuinely pure and miri-runnable. The pty/GTK/libmpv/signal-handler paths cannot run under miri, so a workspace-wide `cargo miri test` is explicitly not the goal; excluded crates/sites are documented with reasons.
- Add a cargo-hack job validating the feature matrix (at minimum `--each-feature` over the workspace, including the 12 `test`-feature crates and their cross-enabling such as `mbv-emby/test → mbv-net/test`).
- Fix whatever the two new gates surface that is in scope (real UB findings, feature-combo build failures), or document and defer with justification if a finding is out of scope.
- No runtime, API, or user-visible behavior change.

## Capabilities

### New Capabilities

None — this change introduces no system behavior.

### Modified Capabilities

None — no existing spec's requirements change. This is pure CI/tooling work to comply with `M-STATIC-VERIFICATION`; specs describe system behavior, so no delta spec applies (`skip_specs: true` in `.openspec.yaml`).

## Impact

- Affected files: `.github/workflows/build.yml` (new jobs/steps), `rust-toolchain.toml` (miri needs a nightly toolchain — see design), possibly new per-crate miri test targets, and whatever source fixes the new gates surface.
- CI cost: miri runs are slow (interpreted execution) and need nightly; the job must be scoped tightly or it becomes the long pole. cargo-hack multiplies check jobs by the feature matrix; `--each-feature` keeps it linear.
- No API, protocol, config-file, or UI changes.
