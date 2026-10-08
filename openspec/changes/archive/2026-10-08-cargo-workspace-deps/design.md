# Design

## Context

See `proposal.md` (Why) for motivation. Current state:

- Workspace root `Cargo.toml` `[workspace.dependencies]` pins most shared deps and most members already consume them via `<dep>.workspace = true`.
- Exceptions (issue #886 evidence): `tracing-subscriber` (identical `registry`+`std` declaration in `mbv-core` `[dependencies]`, `mbv-daemon`/`mbv-net`/`mbv-player` `[dev-dependencies]`), `rust_cast` (`mbv-cast` deps + `mbv-core` dev-deps), `tracing-log`, `mdns-sd`, `flume` (dev-only), `ab_glyph`, `num-traits` — all crate-local.
- Workspace definitions that enable non-basic features, contrary to `M-CARGO-WORKSPACE` (`default-features = false`, features per-member): `ureq` (`json`, `native-tls`), `serde` (`derive`), `uuid` (`v4`, `fast-rng`), `tungstenite` (`native-tls`), `nix` (5 features), `time` (4 features), `textwrap` (`unicode-width`), `ratatui-image` (`crossterm`), `image` (5 codecs).
- Constraint from `AGENTS.md`: `tokio` is edge-only (`mbv-desktop`, `zbus`). `mbv-desktop` also declares `zbus`/`tokio`/`ksni` crate-locally with features; those are out of scope here (see Non-Goals).

## Goals / Non-Goals

**Goals:**

- Every third-party dependency in scope is defined once in `[workspace.dependencies]` with `default-features = false` (or no default features to disable) and no non-basic enabled features; members opt into features per-use via `<dep>.workspace = true` plus `features = [...]` where needed.
- The resolved build is unchanged: same versions, same per-crate enabled-feature union (Cargo feature unification means moving a feature flag from workspace to the member(s) that need it must preserve the workspace-wide union).
- `cargo check` passes per affected crate and workspace-wide; existing test suites stay green.

**Non-Goals:**

- No version upgrades, no feature additions/removals, no new dependencies.
- `mbv-desktop` async-runtime deps (`tokio`, `zbus`, `ksni`) stay as-is: they are crate-local with good reason (tokio edge-only boundary) and are not part of issue #886's evidence. If a future change wants them pinned workspace-wide, that is a separate decision.
- No `[workspace.lints]` or other workspace-table changes.

## Decisions

1. **Workspace definition shape: version-only + `default-features = false`, features per-member.**
   Why: this is exactly what the rule asks for, and it makes each crate's real feature needs visible at the use-site instead of silently inherited. Alternative (keep features on the workspace definition and just dedupe the crate-local copies) was rejected: it would fix duplication but leave the second half of the issue (features enabled in workspace definitions) unaddressed.

2. **What counts as "basic" (stays on the workspace definition): `std` and equivalent no-op conveniences (e.g. `tracing-subscriber`'s `std`+`registry`).**
   Why: the rule explicitly names `["std"]` as the exception. Everything else (`json`, `native-tls`, `derive`, `v4`, codecs, etc.) moves to members. Alternative (treat widely-used features like `serde/derive` as basic) was rejected: `derive` is a proc-macro surface, not a basic capability, and the issue explicitly lists it.

3. **Feature placement by actual use, verified by feature-union comparison — not by blind move.**
   For each stripped feature, the member(s) that exercise it re-declare it (`dep = { workspace = true, features = [...] }`). Cargo unifies features workspace-wide, so the check is: `cargo metadata` / `Cargo.lock`-derived feature union before vs. after must match, and every previously-compiling crate must still compile with its own declared features. Where a feature's consumer is unclear (e.g. which crates need `ureq/json` vs. just `ureq`), implementation inspects call-sites (which APIs are used) rather than copying the full feature list everywhere — over-declaring at every member would re-hide real needs. Alternative (declare the full moved feature set at every consuming member) was rejected: it preserves the build but defeats the visibility goal.

4. **Dev-dependency copies (`tracing-subscriber` in daemon/net/player dev-deps, `flume` in cast dev-deps, `rust_cast` in core dev-deps) become workspace definitions too.**
   Why: the rule makes no normal/dev distinction, and dev-deps drift the same way. `flume` stays dev-only in scope (only `mbv-cast` tests use it) but its version is pinned once in the workspace. Alternative (leave dev-deps local because they don't affect the shipped build) was rejected: the four-way `tracing-subscriber` copy is mostly dev-deps copies, i.e. the core dedup win.

5. **One mechanical commit, no behavior split needed.**
   `Cargo.toml`-only change; rollback is revert. No migration plan beyond: land, run workspace check + affected test suites, confirm lockfile diff touches only manifest-source lines (no version changes).

## Risks / Trade-offs

- [Missed per-member feature re-declaration breaks that crate's build] → Mitigation: per-crate `cargo check -p <each touched member>` plus full `cargo check --workspace`; compiler errors name the missing feature-gated API directly.
- [Feature unification masks a missing declaration (crate compiles in workspace but not standalone)] → Mitigation: the per-crate check above is run per package; additionally compare the before/after feature union so nothing is silently dropped or added.
- [Scope creep into `mbv-desktop` async deps or version upgrades] → Mitigation: explicitly out of scope; any unrelated lockfile churn is reverted before landing.
- [Merge conflicts with in-flight changes touching `Cargo.toml`] → Mitigation: manifests are small, conflicts are textual and trivially re-resolvable; re-run checks after resolving.

## Open Questions

None. Feature-to-member mapping is resolved during implementation by call-site inspection (a mechanical question with a compiler-checked answer, not a design decision).
