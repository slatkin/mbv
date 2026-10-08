# Design

## Context

See `proposal.md` (Why) for motivation. Current state:

- All check-in gates live as steps in one job in `.github/workflows/build.yml` (Arch container, pinned image; toolchain from `rust-toolchain.toml`, currently stable `1.99.0` minimal + clippy/rustfmt): fmt check, clippy `--workspace --all-targets -D warnings`, `cargo audit`, release build, nextest. There is no PR workflow; gates run on main/tag pushes.
- Unsafe surface (~45 sites, 11 files, sampled): `libc` signal plumbing (`run_shutdown.rs` sigmask/sigwait, `signals.rs` `signal`/`poll`, `single_instance.rs` `kill`), `pin.rs` pty/fd hand-over (`openpty`, `dup2`, `ioctl(TIOCNOTTY)`), `std::env::set_var/remove_var` in test support, libmpv FFI in `mbv-player`. Nearly all of it is syscall/FFI-based, i.e. not miri-runnable.
- Feature surface: 12 crates declare a `test` feature; cross-enabling exists (`mbv-components/test → mbv-ui-msg/test + mbv-render/test`, `mbv-emby/test → mbv-net/test + mbv-config/test`, `mbv-player/test → mbv-remote-player/test`), and root dev-deps enable them. Nothing builds feature combinations today.

## Goals / Non-Goals

**Goals:**

- `cargo-hack` validates the feature matrix in CI; miri validates the unsafe code that can actually run under it in CI.
- Exclusions are explicit and documented (crate/site + reason), not silent.
- New gates live with the existing gates and cost proportionally (no hour-long job, no nightly-everything).

**Non-Goals:**

- No workspace-wide `cargo miri test` (impossible: threads/signals/FFI/extern-C everywhere).
- No `cargo-udeps` in this change (same rule sentence, but not issue #888's scope).
- No behavior, API, or dependency changes; findings get fixed only if in scope (see decision 4).

## Decisions

1. **Miri: separate CI job installing its own nightly toolchain; `rust-toolchain.toml` untouched.**
   Miri requires nightly, but the workspace pin (stable `1.99.0`) exists for build reproducibility and cache stability. The miri job installs `nightly` + `miri` component locally in that job (`rustup toolchain install nightly --component miri`) and runs with `cargo +nightly miri test`. Alternative (pin nightly workspace-wide) rejected: it would move every gate and every developer onto nightly and churn the cargo cache. Alternative (no CI job, docs-only "run miri locally") rejected: unenforced gates rot.

2. **Miri scope: audit-first, gate-what-runs, document the rest.**
   Implementation audits each unsafe site and sorts into (a) miri-runnable pure unsafe worth gating (e.g. raw-pointer/data-layout code, if any exists once syscall wrappers are excluded), (b) excluded with reason (signals/threads/FFI/pty/libmpv — cannot execute under miri by construction). The job runs `cargo +nightly miri test -p <crate...>` over set (a) only; set (b) is recorded as a comment block in the workflow (or a short doc if it outgrows comments) so a future reader can tell deliberate exclusion from oversight. Rationale: the issue explicitly demands honest scoping; a gate that fails on un-runnable code on day one is worse than no gate. Expected outcome is a small set (a) — possibly near-empty — and that is an acceptable, truthful result.

3. **cargo-hack: `--each-feature`, same job family as the other checks.**
   `cargo hack check --each-feature --workspace` (installed via `taiki-e/install-action@cargo-hack`, matching the existing install-action pattern for nextest/audit/deb). `--each-feature` is linear in feature count; `--feature-powerset` over 12+ cross-enabling features is combinatorial and unjustified for features that are additive test shims. `--no-dev-deps` handling follows what the first run shows: dev-deps enable `test` features workspace-wide today, so the matrix must cover the dev-deps configuration actually used, not an artificial `--no-dev-deps` slice. Alternative (powerset) rejected on cost; can be revisited if `--each-feature` ever misses a real interaction.

4. **Findings policy: fix in scope, else document-and-defer in the issue.**
   If miri flags real UB or hack finds a broken combo in covered code, the fix lands in this change (that is the point of the gate). If a finding concerns deliberately excluded territory (e.g. UB suspicion inside a pty path miri cannot execute), it is recorded on issue #888 with reasoning and the gate stays green — a red gate nobody can satisfy gets disabled.

5. **Placement: new jobs in `build.yml`, alongside the existing gates.**
   Keeps every gate in the one place the project already looks; no new workflow file, no trigger-matrix duplication. The miri job runs in parallel with the build job (separate `job`, not a step) so interpreted-execution slowness never sits on the release critical path. Alternative (separate `verify.yml`) rejected: no PR workflow exists to attach it to, and splitting gates across files invites them to drift apart.

## Risks / Trade-offs

- [Miri-testable set turns out empty] → Mitigation: still land the job (pinned to whatever pure-unsafe tests exist, even if one crate) plus the documented exclusion list; an empty-but-honest gate with the audit recorded beats re-litigating scope later.
- [Nightly miri flakes (miri tracks nightly internals)] → Mitigation: job allowed to be pinned to a known-good nightly date if it proves flaky; stability over freshness for a gate.
- [cargo-hack multiplies CI minutes] → Mitigation: `--each-feature` only, check (not test/clippy) profile; revisit if the multiplier hurts.
- [New gates break on unrelated future PRs (e.g. someone adds a feature that breaks a combo)] → Mitigation: that is the gate working as intended; failure output names the combo directly.

## Open Questions

None. The audit outcome (which crates land in set (a)) is implementation discovery with a compiler-checked answer, recorded in the workflow comments when done.
