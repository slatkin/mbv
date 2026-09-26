# Tasks

## 1. Lint table

- [ ] 1.1 Remove `missing_errors_doc` and `too_long_first_doc_paragraph` from `[workspace.lints.clippy]` in `Cargo.toml`; keep `missing_panics_doc` (already covered by `pedantic`) and `doc_markdown` (already covered by `pedantic`) enabled. Verify: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -c "missing_errors_doc\|too_long_first_doc_paragraph"` returns 0.

## 2. Mechanical doc_markdown fixes

- [ ] 2.1 Run `cargo clippy --workspace --all-targets --fix --allow-dirty -- -W clippy::doc_markdown` and review the diff is backtick-only (no prose rewrites). Verify: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -c doc_markdown` returns 0, and `git diff --stat` shows only doc-comment lines changed.

## 3. Hand-written `# Panics` sections

- [ ] 3.1 Add `# Panics` sections in `crates/mbv-core/src/ctrl.rs:274` and `crates/mbv-core/src/player/submit.rs:71`, naming the actual panic source (lock poisoning, unwrap, index, etc.) at each site. Verify: `cargo clippy -p mbv-core --all-targets -- -D clippy::missing_panics_doc 2>&1 | grep -c "ctrl.rs\|submit.rs"` returns 0.
- [ ] 3.2 Add `# Panics` sections for the sites in `crates/mbv-core/src/player/controller.rs` (lines 66, 72, 200, 229, 240, 247, 256, 261, 270, 274, 283, 302, 331, 341, 368, 379). Verify: `cargo clippy -p mbv-core --all-targets -- -D clippy::missing_panics_doc 2>&1 | grep -c controller.rs` returns 0.
- [ ] 3.3 Add `# Panics` sections for the 3 sites in `crates/mbv-core/src/player/proxy.rs` (lines 62, 398, 489). Verify: `cargo clippy -p mbv-core --all-targets -- -D clippy::missing_panics_doc 2>&1 | grep -c proxy.rs` returns 0.
- [ ] 3.4 Add `# Panics` sections for the 8 sites in `crates/mbv-core/src/remote_player.rs` (lines 107, 156, 211, 240, 264, 307, 373) and `crates/mbv-core/src/remote_player/connect.rs:612`. Verify: `cargo clippy -p mbv-core --all-targets -- -D clippy::missing_panics_doc 2>&1 | grep -c remote_player` returns 0.
- [ ] 3.5 Add `# Panics` sections for the 4 sites across `crates/mbv-keybinds/src/chord.rs:84`, `crates/mbv-keybinds/src/config.rs:63,232`, `crates/mbv-keybinds/src/registry.rs:132`. Verify: `cargo clippy -p mbv-keybinds --all-targets -- -D clippy::missing_panics_doc` exits 0.
- [ ] 3.6 Add `# Panics` sections for the 2 sites in `crates/mbv-net/src/mock_http.rs` (lines 93, 99). Verify: `cargo clippy -p mbv-net --all-targets -- -D clippy::missing_panics_doc` exits 0.

## 4. Full verification

- [ ] 4.1 Run `cargo clippy --workspace --all-targets -- -D warnings` and confirm it exits 0. Run `cargo fmt --all -- --check` and confirm no diff. Run `cargo nextest run --workspace` and confirm all tests pass.
