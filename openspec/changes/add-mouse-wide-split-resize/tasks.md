## 1. Split Override Plumbing

- [ ] 1.1 Add the session-only list-pane-width override field to `App` and a clamp helper that normalizes an override against a content-area width (both panes at or above `WIDE_HERO_MIN_PANE_WIDTH`, empty range at the threshold yields the default); verify with focused state tests covering bounds, the empty-range case, and `None` passthrough.
- [ ] 1.2 Add the override parameter to `wide_hero_split` behind a plain-signature wrapper so breakpoint `.is_some()` checks stay untouched, and thread the override through the wide geometry call sites (browser, TV, Music, Home, Feeds, ABS books/podcasts, `wide_library_panes`); verify with a buffer test proving `None` is byte-identical to the current frame and a test proving an override moves both panes with the gap following.

## 2. Boundary Ownership

- [ ] 2.1 Add the boundary Interactive Component (private gesture state, gap-only arming, live-width message emission, DragEnd no-op), its `ComponentId`, and the mount; verify with focused component tests covering exact-edge arming, one-column resolution, clamping, click-only no-op, DragEnd emitting nothing, and eligibility cancellation resetting gesture state.
- [ ] 2.2 Add the shell tab→area mapping for the active wide surface, eligibility (`active wide surface && panel_mouse_eligible()`), per-frame sync, and the exhaustive dispatch arm applying the live width; verify with a sentinel buffer test proving the gap's appearance is unchanged and a test proving pane gestures adjacent to the gap still resolve to their own components.

## 3. Reset Semantics

- [ ] 3.1 Clear the override in `refresh_current_view` and confirm cross-surface sharing; verify with a test that drag → refresh returns the split to the default ratio, a test that switching wide tabs keeps the session width (clamped per surface), and an assertion path showing no preference/config write occurs on drag.

## 4. Integration and Documentation

- [ ] 4.1 Extend the live-tick mouse integration coverage with a press-drag-release sequence on a wide surface and an overlay-suppression case, verifying exact live widths, only the boundary owner's message is delivered, and destination components do not handle the gesture through `Application::tick()`.
- [ ] 4.2 Update `docs/architecture/interactive-surface-ledger.md` with the boundary owner, the gap drag gesture, wide-breakpoint availability (all wide surfaces), and the verification evidence.
- [ ] 4.3 Manually verify dragging on at least two different wide tabs (including Home), refresh reverting to default, restart showing the default split, then run `cargo fmt`, targeted `cargo nextest` tests, `cargo check -p mbv`, `ast-grep scan`, and `make check-code-file-lines`.
