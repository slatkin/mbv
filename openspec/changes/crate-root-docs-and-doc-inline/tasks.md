## 1. Protocol and data crates

Each task: read the crate's `src/lib.rs` `mod` list and `Cargo.toml`, add the `//!` block at the top of `lib.rs` (above any `mod`/`use`), then add `#[doc(inline)]` above every workspace-item `pub use` in that crate (root and nested files). Skip `std` and third-party paths.

- [x] 1.1 `mbv-ids`, `mbv-text`, `mbv-theme`, `mbv-ctrl`, `mbv-queue`
- [x] 1.2 `mbv-config`, `mbv-core`, `mbv-feed`, `mbv-ui-model`
- [x] 1.3 Gate: `cargo check -p` each crate above, then `cargo fmt` → verify: no errors

## 2. Provider and transport crates

- [x] 2.1 `mbv-emby` (including `types.rs`), `mbv-audiobookshelf`, `mbv-cast`
- [x] 2.2 `mbv-net`, `mbv-ws`, `mbv-images`
- [x] 2.3 Gate: `cargo check -p` each crate above, then `cargo fmt` → verify: no errors

## 3. Player, daemon, and desktop crates

- [ ] 3.1 `mbv-player`, `mbv-remote-player` (including `connect.rs`)
- [ ] 3.2 `mbv-daemon`, `mbv-desktop`, `mbv-visualizer`. State the headless-versus-Local-process boundary from AGENTS.md in the daemon and desktop docs.
- [ ] 3.3 Gate: `cargo check -p` each crate above, then `cargo fmt` → verify: no errors

## 4. UI crates

- [ ] 4.1 `mbv-render` (root plus nested `components/*` re-exports; its `mbv_ui_model::sort_filter` re-exports are workspace items and get the attribute)
- [ ] 4.2 `#[doc(inline)]` only in `mbv-components` (`lib.rs`, `queue.rs`, `tv_content.rs`, `list.rs`, `media_list.rs`, `library_panel.rs`, `list/tree_browser.rs`, `library_panel/hero_header.rs`) and `mbv-ui-msg`, `mbv-keybinds` (root docs already exist; leave them as they are)
- [ ] 4.3 Gate: `cargo check -p` each crate above, then `cargo fmt` → verify: no errors

## 5. Finish

- [ ] 5.1 `cargo clippy --workspace --all-targets -- -D warnings` → verify: clean (pedantic `doc_markdown` may flag unquoted identifiers in new docs; fix by backticking)
- [ ] 5.2 `cargo doc --workspace --no-deps` → verify: no rustdoc warnings from the touched files
- [ ] 5.3 Read-only check of coverage: `rg -L '^//!' crates/*/src/lib.rs` lists only `mbvd`-style binaries, and `rg -B1 '^\s*pub use ' crates` shows `#[doc(inline)]` above each hit. This is a one-off spot check, not a CI gate.
- [ ] 5.4 Commit the change in one commit that references #890. Do not push.
