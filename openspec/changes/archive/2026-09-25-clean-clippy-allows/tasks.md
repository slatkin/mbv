# Tasks

## 1. No-op allow deletions

- [x] 1.1 Delete the three module-level `#[allow(clippy::unwrap_used)]` attributes in `src/app/components/inline_search.rs`, `src/app/components/feeds_content/mod.rs`, and `src/app/components/library_panel/panel_list.rs`, and the file-level `#![allow(clippy::unwrap_used)]` in `src/app/components/library_panel/hero_header/hero_header_tests.rs`. Verify: `rg -n 'allow\(clippy::unwrap_used' src crates` returns nothing and `cargo check -p mbv --all-targets` passes.

## 2. wide_hero test-module relocation

- [x] 2.1 In `src/app/render/arrangements/wide_hero.rs`, move the production block sitting between test modules (the `WideHeroBrowserPane`/`PillBarAreas` structs and adjacent functions, currently lines ~225–320) up to directly follow the last production item before the first `#[cfg(test)]` module; do not alter any moved line's content. Verify: `cargo check -p mbv --all-targets` passes and `cargo nextest run -p mbv` reports the same test outcomes for the wide_hero modules as before the move.

- [x] 2.2 Delete the three `#[allow(clippy::items_after_test_module)]` attributes at the (former) test-module sites. Verify: `rg -n 'items_after_test_module' src crates` returns nothing and `cargo clippy -p mbv --all-targets 2>&1 | rg 'items_after_test_module'` is empty.

## 3. large_enum_variant boxing

- [x] 3.1 Remove the `#[allow(clippy::large_enum_variant)]` from `Msg` and run `cargo clippy -p mbv --all-targets` to capture which variant the diagnostic names (variant + byte sizes); record it in the task notes. Verify: clippy output names exactly one large variant for `Msg`.
  - Clippy identified `Shell(ShellRequest)` as largest (at least 784 bytes); `Service(ServiceRequest)` is second-largest (at least 72 bytes).

- [x] 3.2 Box the variant payload named by 3.1 (e.g. `Shell(Box<ShellRequest>)`) and mechanically update every construction and match site (`rg -n '<PayloadName>' src crates` to enumerate); delete the stale `TODO(migrate-tui-to-tuirealm)` comment. Verify: `cargo check -p mbv --all-targets` passes and `cargo nextest run -p mbv` is green.

- [x] 3.3 Change `LeafKeyResult::Consumed(Option<Msg>)` to `Consumed(Option<Box<Msg>>)`, update `into_option` to map through the box, wrap the `leaf_key_tests` construction sites, and delete both the `#[allow(clippy::large_enum_variant)]` and the "Msg is the large arm; boxing would wrap every request" comment. Verify: `rg -n 'large_enum_variant' src crates` returns nothing and the `leaf_key_tests` test passes via `cargo nextest run -p mbv leaf_key`.

## 4. Final gates

- [x] 4.1 Run `cargo fmt` on touched paths, then `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, and `make check-code-file-lines`. Verify: all gates green with no new suppressions added anywhere; commit the change referencing issue #792.
  - `cargo fmt --all -- --check`, workspace Clippy, and workspace nextest pass. `make check-code-file-lines` still fails on pre-existing over-limit files: `crates/mbv-core/src/player/run/commands.rs` (821), `src/app/render/tests/mod.rs` (813), and `src/app/shell/music_workspace/owner_tests.rs` (843; 831 on HEAD). These unrelated file splits were not included in this change.
