## Why

Issue #643 found that the four `is_wide_*_active()` predicates in `src/app/layout.rs` infer breakpoint state from what was painted on the previous frame, so every keyboard/behaviour path that branches on them acts on the wrong breakpoint for one frame after a resize. The TV mount flash was fixed with `prime_wide_tv_geometry`, but the keyboard/activation staleness — and the book/podcast post-paint readback mirrors — remain, with ~12 consumers and growing.

**Tracking issue:** [GitHub issue #643](https://github.com/slatkin/mbv/issues/643). The issue owns Residual B of ADR 0022; this OpenSpec change owns the predicate replacement, the resize-tick fix, and the completion gates.

## What Changes

- Introduce one paint-free breakpoint predicate on `App` generalizing `wide_tv_library_area()`'s gate (`chrome_geometry` → `right_panel_content_area` → `shared_hero_presentation().is_some()`), derived from mount area / current terminal size, never paint history.
- Carry terminal dimensions in `TerminalObserverEvent::Resize` (currently discarded at `root.rs:72`) and update `terminal_width`/`terminal_height` eagerly in `apply_terminal_observer`, so `sync_mounted_surfaces` (prime/handoff/push) reads fresh size on the resize tick itself.
- Migrate all ~12 decision consumers (Series activation, album-folder activation, album modal, book chapter modal, TV browser/workspace mount gates, TV focus gate, context-menu anchor, music/TV/push guards, inline-search wide flag) to the shared predicate.
- Detach the Audiobookshelf book/podcast post-paint wide-flag mirrors (5.3d.10e-style): `render_audiobookshelf_*_component` no longer projects `geometry.wide` back into `LayoutMain`; all readers use the predicate.
- **BREAKING** (internal): delete the four `LayoutMain::is_wide_*_active()` predicates outright; single source of truth, no dual API.
- Add render tests at both breakpoints (narrow + wide) covering the post-resize frame, including the resize-tick mount path.

## Capabilities

### New Capabilities

(none — no new user-facing capability; this is a correctness fix under existing arrangement contracts.)

### Modified Capabilities

- `right-panel-arrangements`: the responsive breakpoint SHALL be derived paint-free from the mount area / current terminal size (not previous-frame paint rects or post-paint component readbacks), SHALL be correct on the resize tick itself, and `is_wide_*_active()` paint-inference SHALL be removed.

## Impact

- **Primary code areas:** `src/app/layout.rs` (predicate deletion), `src/app/components/msg.rs` + `src/app/components/root.rs` (Resize carries size), `src/app/shell.rs` (eager size update), `src/app/shell_{tv_workspace,music_workspace,audiobookshelf_book,audiobookshelf_podcast,browser,library,inline_search,messages,overlays_menus}.rs`, `src/app/{input_browse_dispatch,actions_navigation,audiobookshelf_book_modal_actions}.rs`, `src/app/render/components/{tv_wide,music_wide,audiobookshelf_book,audiobookshelf_podcast}.rs`.
- **Paint readers affected:** context-menu anchor and hit-test paths that consume painted rects must derive what they need from the predicate + chrome geometry; each is proven during migration rather than kept on a legacy predicate.
- **Tests:** resize-tick regression tests (mount gates + keyboard branching on the post-resize frame), narrow/wide render coverage per breakpoint, book/podcast mirror-detach coverage.
- No new dependency, protocol, daemon, provider, configuration, or external API change.
