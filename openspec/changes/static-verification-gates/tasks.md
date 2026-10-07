# Tasks

## 1. cargo-hack feature-matrix gate

- [ ] 1.1 Install cargo-hack locally and run `cargo hack check --each-feature --workspace` (plus the dev-deps configuration the repo actually builds) and verify the command completes; record every failing feature combination.
- [ ] 1.2 Fix each failing combination from 1.1 that is in covered code (or document-and-defer on issue #888 with reasoning if it is deliberately excluded territory) and verify `cargo hack check --each-feature --workspace` passes clean.
- [ ] 1.3 Add the cargo-hack job to `.github/workflows/build.yml` (install via `taiki-e/install-action@cargo-hack` matching the existing pattern, parallel job so it never sits on the release critical path) and verify the workflow YAML parses and the job's commands pass locally as written.

## 2. Miri scoped gate

- [ ] 2.1 Audit all ~45 `unsafe` sites (11 files: `run_shutdown.rs`, `signals.rs`, `single_instance.rs`, `pin.rs`, `test_support.rs`, `mbv-player` runtime/controller/run-loop, `local_daemon.rs`, `main.rs`, `mbvd`) into miri-runnable set (a) vs. excluded-with-reason set (b) (signals/threads/FFI/pty/libmpv cannot execute under miri) and verify the classification is recorded as a comment block in the workflow (or short doc if it outgrows comments).
- [ ] 2.2 For each set-(a) site lacking executable coverage, add the minimal miri-runnable test that exercises it (no coverage for coverage's sake; excluded code gets no tests) and verify `cargo +nightly miri test -p <crate...>` passes over set (a).
- [ ] 2.3 Fix any real UB miri flags in set (a) (or document-and-defer on issue #888 per the findings policy if it falls in excluded territory) and verify the miri run is green.
- [ ] 2.4 Add the miri job to `.github/workflows/build.yml` (own nightly toolchain install in-job, `rust-toolchain.toml` untouched, parallel job off the release critical path, exclusion list recorded per 2.1) and verify the workflow YAML parses and the job's commands pass locally as written.

## 3. Integration verification

- [ ] 3.1 Run the full local gate suite (`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo audit`, the 1.2 hack command, the 2.3 miri command) and verify all green together with no `rust-toolchain.toml` or lockfile churn beyond what the change intends.
