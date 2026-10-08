# Proposal

## Why

Dependency declarations that belong in `[workspace.dependencies]` are declared in member manifests instead, with verbatim copies across crates (`tracing-subscriber` in four manifests, `rust_cast` in two). The same workspace table also enables dependency features where `docs/standards/rules/M-CARGO-WORKSPACE.md` asks for `default-features = false` with features enabled per-member. This drifts from the project's own cargo-workspace standard and makes version/feature changes N-way edits (issue #886).

## What Changes

- Move crate-local third-party declarations into the workspace root `[workspace.dependencies]`:
  - `tracing-subscriber` (deduplicates the four-way copy in `mbv-core`, `mbv-daemon`, `mbv-player`, `mbv-net` dev-deps), `tracing-log`, `rust_cast`, `mdns-sd`, `flume` (dev-only), `ab_glyph`, `num-traits`.
  - Convert each member use-site to `<dep>.workspace = true`, preserving per-member `features` / `default-features` where the member actually needs them.
- Strip non-basic features from workspace definitions per the rule (workspace uses `default-features = false`, no enabled features except basic ones such as `std`); move each removed feature to the member(s) that actually require it. Dependencies in scope: `ureq`, `serde`, `uuid`, `tungstenite`, `nix`, `time`, `textwrap`, `ratatui-image`, `image` (full list in issue evidence).
- No dependency version upgrades, no feature additions/removals in the resolved build: the union of enabled features per dependency after the move must equal the union before it (verified via `cargo metadata` / lockfile comparison and full workspace check + tests).
- No runtime, API, or user-visible behavior change.

## Capabilities

### New Capabilities

None — this change introduces no system behavior.

### Modified Capabilities

None — no existing spec's requirements change. This is a pure build-config refactor to comply with `M-CARGO-WORKSPACE`; specs describe system behavior, so no delta spec applies (`skip_specs: true` in `.openspec.yaml`).

## Impact

- Affected files: workspace root `Cargo.toml` (`[workspace.dependencies]` table) and member manifests for `mbv-core`, `mbv-cast`, `mbv-images`, `mbv-daemon` (`[dev-dependencies]`), `mbv-net` (`[dev-dependencies]`), `mbv-player` (`[dev-dependencies]`), plus any member that currently enables a feature being moved off the workspace definition (e.g. consumers of `ureq`, `serde`, `uuid`, `tungstenite`, `nix`, `time`, `textwrap`, `ratatui-image`, `image`).
- Risk is low but mechanical: a missed per-member feature re-declaration breaks compilation of that crate; mitigated by per-crate `cargo check` and lockfile/feature-union comparison.
- No API, protocol, config-file, or UI changes. No new dependencies.
