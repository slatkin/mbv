## 1. Core change

- [x] 1.1 Change `POLL_INTERVAL_MS` from `100` to `16` in `crates/mbv-core/src/visualizer.rs`

## 2. Verification

- [x] 2.1 Build the project with `cargo build` and confirm no compilation errors
- [x] 2.2 Run existing tests with `cargo test` and confirm no regressions
- [ ] 2.3 Manually verify: start the daemon with spectrum streaming enabled, play audio, and confirm the visualization appears smooth and responsive (no visible stutter compared to before)
