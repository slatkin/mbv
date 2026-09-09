# Fix TV Series Media-List Ownership Tasks

## 1. Characterize TV Series ownership

- [x] 1.1 Add or update focused TV component/media-list tests proving retained-current-frame series and episode target hits, invalidation after geometry/content changes, blank/header/above-list no-op behavior, click selection updates the persistent control, wheel movement uses the retained claim, and ordinary refresh preserves/clamps control-owned selection; verify the targeted tests pass.
- [x] 1.2 Add Wide AND Normal/Narrow shell-tick integration coverage through `Application::tick()`, `draw_frame`, and the shell sync pass proving navigation paints the same selected row, clicks resolve painted stable targets while blank/header space is unclaimed, wheel input moves the Wide series control, exactly one list painter owns each active surface with no base-frame underpaint, and Wide→Normal plus Normal→Wide handoff preserves selected target and row offset; model on `tests_tick_integration_home.rs` and `tests_tick_integration_browser.rs` and verify the targeted tests pass.

## 2. Transfer TV Series list authority

- [x] 2.1 Change the source-of-truth `TvHit::SeriesRow` and `TvHit::EpisodeRow` payloads from ordinals to stable `String` targets, then refactor `TvWorkspaceComponent` to delete its parent cursor field and all write-backs, make series clicks select the persistent control, resolve series/episode hits only through `claims_current_point` / `resolve_current_point` (plus `current_detail_rect` only where the detail pane requires it), leave blank/header/above-list space unclaimed, and gate wheel input on the retained series claim; verify focused tests cover click/control agreement, invalidation, and retained geometry.
- [x] 2.2 Refactor TV shell click, double-click, and context-menu arms to consume the stable component-resolved target without calling `set_resting_cursor`, preserving existing focus and semantic effects; search all callers of `resolve_ordinal_at_y` and delete it only if TV was the final caller, otherwise leave it unchanged and record the remaining caller; verify shell tests cover stable-target resolution and unresolved-target no-op behavior while TV seasons, episode workspace content, detail, images/effects/persistence, and existing typed keyboard/activation semantics remain unchanged.

## 3. Verify the bounded repair

- [x] 3.1 Run relevant TV component, media-list, render, shell, and shell-tick tests at Wide and Normal/Narrow breakpoints; verify retained hits and invalidation, unclaimed blank/header geometry, working wheel input, bidirectional target-plus-offset handoff, no base-frame underpaint, and exactly one list painter per active surface.
- [x] 3.2 Run `cargo fmt --all -- --check`, `cargo check -p mbv`, relevant `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, and `ast-grep scan`; record any unrelated failure without widening this change.
