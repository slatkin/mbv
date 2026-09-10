## 1. Split Override Plumbing

- [x] 1.1 Add the session-only list-pane-width override field to `App` and a clamp helper that normalizes an override against a content-area width on every application (both panes at or above `WIDE_HERO_MIN_PANE_WIDTH`, empty range at the threshold yields the default); verify with focused state tests covering bounds, the empty-range case, `None` passthrough, and the terminal-resize clamp (override survives at the new bounds with no resize-event hook).
- [x] 1.2 Add the override parameter to `wide_hero_split`, move breakpoint checks to a dedicated breakpoint predicate so geometry exits only through the override-taking function, and thread the override through the real touch set (~15–20 forwards across ~10 files): shell-direct paths (`shell_browser`, `shell_music_workspace`, TV wide), ctx-bearing components (`BrowserComponent`, `MusicWorkspaceComponent`, Home, Feeds, ABS books/podcasts) via their sync/push render-ctx fields, and `wide_library_panes`; verify with a buffer test proving `None` is byte-identical to the current frame, a test proving an override moves both panes with the gap following, and a new `ast-grep scan` rule confining `wide_hero_split` callers to the arrangement file.

## 2. Boundary Ownership

- [ ] 2.1 Add the boundary Interactive Component (private gesture state, gap-only arming, live-width message emission, DragEnd no-op), its `ComponentId`, and the mount; verify with focused component tests covering exact-edge arming, one-column resolution, clamping, click-only no-op, DragEnd emitting nothing, and eligibility cancellation resetting gesture state.
- [ ] 2.2 Add the shell tab→area mapping for the active wide surface with per-arm painted-split eligibility (empty/loading/no-selection states arm nothing), per-frame sync from the `LayoutMain` rects, the pinned pointer→width resolution (near gap column exact, outer column one wider, tracking outside the gap once changed), and the exhaustive dispatch arm applying the live width; verify with a sentinel buffer test proving the gap's appearance is unchanged on a representative surface and on an empty/loading state, and a test proving pane gestures adjacent to the gap still resolve to their own components.

## 3. Reset Semantics

- [ ] 3.1 Clear the override inside `refresh_current_view`'s `PanelFocus::Library` arm (not at the function top, so a queue-focus refresh leaves the split untouched) and confirm cross-surface sharing; verify with tests that a library-view refresh returns the split to the default ratio, a queue-focus refresh leaves the split unchanged, switching wide tabs keeps the session width (clamped per surface), and an assertion path showing no preference/config write occurs on drag.

## 4. Integration and Documentation

- [ ] 4.1 Extend the live-tick mouse integration coverage with a press-drag-release sequence on a wide surface, an overlay-suppression case (including mid-drag eligibility loss leaving no stale width), and an empty/loading-state case verifying the boundary arms nothing and delivers no messages; verify exact live widths, that only the boundary owner's message is delivered, and that destination components do not handle the gesture through `Application::tick()`.
- [ ] 4.2 Update `docs/architecture/interactive-surface-ledger.md` with the boundary owner, the gap drag gesture, wide-breakpoint availability (all wide surfaces), and the verification evidence.
- [ ] 4.3 Manually verify dragging on at least two different wide tabs (including Home), refresh reverting to default, restart showing the default split, then run `cargo fmt`, targeted `cargo nextest` tests, `cargo check -p mbv`, and `ast-grep scan`.
