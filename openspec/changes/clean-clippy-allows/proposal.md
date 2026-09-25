# Proposal

## Why

Nine clippy `allow` attributes suppress lints that are either no-ops or paper over real structure problems (issue #792). Four `clippy::unwrap_used` allows guard test modules even though the lint is not enabled anywhere (no `[lints]` table, no `clippy.toml` entry), so they are dead weight that suggests suppression policy exists where it does not. Three `clippy::items_after_test_module` allows in `wide_hero.rs` let test modules sit mid-file, forcing every future reader to hop around test code to find production items. Two `clippy::large_enum_variant` allows on `LeafKeyResult` and `Msg` defer a boxing fix whose recorded rationale ("after migration churn settles") is now stale — the Tuirealm migration (ADR 0022) is complete.

## What Changes

- Delete the 4 no-op `clippy::unwrap_used` allows (test-module attributes in `inline_search.rs`, `feeds_content/mod.rs`, `panel_list.rs`; file-level allow in `library_panel/hero_header/hero_header_tests.rs`).
- In `src/app/render/arrangements/wide_hero.rs`, hoist the production items currently sitting between test modules (the `WideHeroBrowserPane`/`PillBarAreas` block and adjacent functions) above the first test module, so all four test modules sit at the end of the file; delete the 3 `clippy::items_after_test_module` allows.
- In `src/app/components/msg/mod.rs`, box the oversized payloads so `clippy::large_enum_variant` passes without suppression:
  - `LeafKeyResult::Consumed(Option<Msg>)` → `Consumed(Option<Box<Msg>>)`, with the `into_option` conversion adjusting (`Some(msg)` → `Some(*msg)`).
  - Identify `Msg`'s largest variant from clippy's diagnostic once the allow is removed and box that payload arm (e.g. `Shell(Box<ShellRequest>)` if `ShellRequest` is the reported offender), updating construction/match sites mechanically.
  - Delete both `clippy::large_enum_variant` allows and the stale `TODO(migrate-tui-to-tuirealm)` comment.
- Update the `leaf_key_tests` construction sites for the boxed form.
- No user-visible behavior change: boxing is an internal representation change; test-module moves do not alter test identity or coverage.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- None. Pure internal refactor (lint hygiene, code organization, representation change with identical observable behavior), so the change declares `skip_specs: true` per the `remove-production-dead-code` precedent.

## Impact

- Files: `src/app/components/msg/mod.rs` (+ its submodule construction sites that build the boxed arm), `src/app/render/arrangements/wide_hero.rs`, `src/app/components/inline_search.rs`, `src/app/components/feeds_content/mod.rs`, `src/app/components/library_panel/panel_list.rs`, `src/app/components/library_panel/hero_header/hero_header_tests.rs`.
- Verification: `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo nextest run -p mbv` green, `cargo fmt` per change.
- Coordination: `feeds_content/mod.rs` currently carries uncommitted edits from the in-flight `remove-production-dead-code` follow-up; this change must land on top of that state, not interleaved with it.
