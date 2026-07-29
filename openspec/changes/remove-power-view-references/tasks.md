## 1. Source code: doc comments and module docs

Rename "Power View" references in doc comments, module docs, and inline comments across ~45 source files. Replace with domain-appropriate terms (queue, library, panel, main) or drop qualifiers entirely.

- [ ] 1.1 Rename references in `src/app/layout.rs`, `src/app/app_struct.rs`, `src/app/types_*.rs`
- [ ] 1.2 Rename references in `src/app/render/mod.rs`, `src/app/render/list.rs`, `src/app/render/sort_filter.rs`, `src/app/render/chrome_status.rs`
- [ ] 1.3 Rename references in `src/app/action.rs`, `src/app/actions.rs`, `src/app/music_actions.rs`, `src/app/browse_level_actions.rs`, `src/app/feed_actions.rs`
- [ ] 1.4 Rename references in `src/app/images.rs`, `src/app/palette.rs`, `src/app/ui_util.rs`, `src/app/library_route.rs`
- [ ] 1.5 Rename references in `src/app/input_resolver.rs`, `src/app/input_lib_power_keys.rs`, `src/app/input_queue_keys.rs`
- [ ] 1.6 Rename references in `src/app/power_home_actions.rs`, `src/app/power_cw_library_tab_actions.rs`
- [ ] 1.7 Rename references in `src/mpris.rs`
- [ ] 1.8 `cargo build` and `cargo clippy` — green, zero warnings

## 2. Source code: user-facing strings

Update status bar messages and other user-visible strings that still say "Power view".

- [ ] 2.1 Update `"Power view width: ... cols"` in `src/app/input_queue_keys.rs` to use new vocabulary
- [ ] 2.2 Update corresponding string assertions in `src/app/tests_queue_scope.rs`
- [ ] 2.3 Audit all other user-facing strings for "power view" references (case-insensitive search)
- [ ] 2.4 `cargo test` — green

## 3. Source code: test function names

Rename ~20+ test functions across 7 files that use `power_view_*` naming. Drop the `power_view` prefix since it no longer names a distinct view.

- [ ] 3.1 Rename test fns in `src/app/actions_tests_queue.rs`
- [ ] 3.2 Rename test fns in `src/app/tests_queue_scope.rs`
- [ ] 3.3 Rename test fns in `src/app/tests_podcast.rs`
- [ ] 3.4 Rename test fns in `src/app/input_power_library_scope_routing_tests.rs`
- [ ] 3.5 Rename test fns in `src/app/input_power_movie_detail_tests.rs`
- [ ] 3.6 Rename test fns in `src/app/render/tests_queue.rs`
- [ ] 3.7 Rename test fns in `src/app/render/tests_scroll_pills.rs`
- [ ] 3.8 `cargo test` — green, all renamed tests still discovered and passing

## 4. Docs: update ADRs and plans

Update documentation files that still reference "Power View".

- [ ] 4.1 ADR 0013 (`docs/adr/0013-power-view-is-the-only-view.md`): add amendment noting remaining `power_` references are implementation details outside ADR 0013's scope; consider title rename
- [ ] 4.2 ADR 0009 (`docs/adr/0009-v-key-controls-audio-visualizer.md`): verify existing amendment note is sufficient or update
- [ ] 4.3 Rename plan (`docs/plans/2026-07-24-power-view-only.md`): note that the rename is now complete; link to this change
- [ ] 4.4 Update `docs/plans/2026-07-10-power-movie-banner-inline.md` and `docs/plans/2026-07-10-input-phase2-keyboard-spine.md` to replace "Power View" with current terminology
- [ ] 4.5 Update `docs/adr/0011-library-scoped-daemon-routing.md` if it contains actionable "Power View" references (vs. historical context)

## 5. Verification

- [ ] 5.1 `cargo fmt`
- [ ] 5.2 `cargo clippy` — zero warnings
- [ ] 5.3 `cargo test` — fully green
- [ ] 5.4 Final grep: `rg -i "power.view" src/ docs/` — confirm zero remaining references (or only intentional historical ones documented in ADR 0013)
