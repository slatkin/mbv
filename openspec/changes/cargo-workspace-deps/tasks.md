# Tasks

## 1. Baseline and deduplicate crate-local declarations

- [ ] 1.1 Record the pre-change feature union (`cargo metadata` feature set per affected dep and/or `Cargo.lock` copy) and verify it is stored alongside the change for later comparison.
- [ ] 1.2 Add workspace definitions for `tracing-subscriber` (`default-features = false`, no enabled features beyond basic `std`/`registry` per design decision 2), `tracing-log`, `rust_cast`, `mdns-sd`, `flume`, `ab_glyph`, `num-traits`, and switch `mbv-core`, `mbv-cast`, `mbv-images` (deps), `mbv-core` (dev-dep `rust_cast`), `mbv-cast` (dev-dep `flume`), `mbv-daemon`/`mbv-net`/`mbv-player` (dev-dep `tracing-subscriber`) to `<dep>.workspace = true` with per-member features where needed; verify `cargo check -p mbv-core,mbv-cast,mbv-images,mbv-daemon,mbv-net,mbv-player` passes.

## 2. Strip non-basic features from workspace definitions

- [ ] 2.1 Rewrite workspace definitions for `ureq`, `serde`, `uuid`, `tungstenite`, `nix`, `time`, `textwrap`, `ratatui-image`, `image` to `default-features = false` with no non-basic enabled features, and re-declare each removed feature on the member(s) that actually exercise it (call-site inspection per design decision 3); verify each touched member compiles via `cargo check -p <package>`.
- [ ] 2.2 Confirm no member manifest outside section 1 still declares a third-party version directly (only `version.workspace`-style or path deps plus per-member `features` remain) by re-running the issue's evidence greps and verifying zero hits; verify `cargo check --workspace --all-targets` passes.

## 3. Integration verification

- [ ] 3.1 Compare post-change feature union and `Cargo.lock` against the section 1.1 baseline and verify no version changed and no enabled feature was added or dropped (only moved from workspace definition to member use-sites).
- [ ] 3.2 Run the affected test suites (`cargo nextest run -p mbv-core,mbv-cast,mbv-images,mbv-daemon,mbv-net,mbv-player` at minimum, workspace-wide if cheap) and `cargo clippy --workspace --all-targets` and verify green; verify `mbv-desktop` manifests untouched.
