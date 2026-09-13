## 1. Video cache configuration

- [x] 1.1 Add positive integer `video_cache_forward_mb` and `video_cache_back_mb` fields with 50/100 defaults to the core `Config`, parse each `[mpv]` value independently with invalid values falling back to its default, and persist both through the existing save path; extend the existing configuration parse/save tests with one compact valid-and-invalid table and verify `cargo nextest run -p mbv-core config` passes.
- [x] 1.2 Add the two optional keys and concise memory-versus-buffering guidance to `dist/config.toml` and `dist/mbvd.toml`, keeping packaged defaults unchanged; verify both examples parse through the existing configuration loader tests.

## 2. Player policy projection

- [x] 2.1 Carry the resolved video cache values from each binary's existing `Config` through `Player` and every `MpvRunConfig` construction without adding another configuration reader or global; update affected construction tests and verify `cargo check -p mbv-core -p mbv -p mbvd` passes.
- [x] 2.2 Update `init_mpv` so non-headless runs use the configured video cache values, headless runs remain fixed at 10M/10M, and cache-property failures are logged; extend the existing real-mpv headless/non-headless initialization tests to prove custom video values apply only to the non-headless arm, then verify `cargo nextest run -p mbv-core player_tests_submit` passes.
- [x] 2.3 Set `hwdec=auto-safe` only for non-headless runs with `use_mpv_config=false`, leaving headless runs and `use_mpv_config=true` untouched; extend the existing real-mpv initialization coverage to assert the resolved `hwdec` property for the isolated path and a temporary user `mpv.conf` override for the full-config path, then verify the targeted mbv-core tests pass without requiring GPU hardware.

## 3. Final verification

- [x] 3.1 Run `cargo fmt`, `cargo nextest run -p mbv-core`, and `cargo clippy --workspace --all-targets -- -D warnings`; confirm the generated private mpv configuration still protects mbv's IPC option and no protocol or Settings-sidebar surface changed.
