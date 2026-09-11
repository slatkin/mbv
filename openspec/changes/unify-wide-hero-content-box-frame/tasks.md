## 0. Preconditions (do not skip)

- [ ] 0.1 Confirm `unify-surface-colour-neutral` has landed on main, then re-verify every `file:line` citation in this change's proposal, design and tasks against the tree as it then stands — its Unit A migrates `arrangements/wide_hero.rs` and Unit C migrates `components/hero.rs` and `components/music_wide.rs`, so this change's anchors will have moved. Verify by re-grepping each cited symbol and correcting the citations in this change before writing any code.
- [ ] 0.2 Read the landed signature of `wide_hero_hero_pane` and record whether it still takes `LeftPaneFocus` or the bool its D3(d) describes; verify the newtype this change adds wraps whatever the landed parameter list is, and that the finding is recorded in this change's design before the return type is changed.

## 1. Settle the target frame with evidence

- [ ] 1.1 Render TV's and Home's Wide hero at the same Wide area into a `TestBackend` buffer and compare each main-content box's leftmost `SURFACE_BACKDROP` column against `wide_hero_presentation(area, None).hero.x + PANE_PAD_X`; verify the comparison (and the chosen target: box edge == pane + `PANE_PAD_X`) is written into this change's `design.md` D1 evidence before any refactor starts.
- [ ] 1.2 Add the cross-destination invariant to `src/app/render/tests_conformance_matrix.rs`: per Wide case, assert the leftmost `SURFACE_BACKDROP` column inside the hero pane equals `pane.x + PANE_PAD_X`, stated relationally (no absolute coordinates, no arrangement return values). Verify it **fails** on the pre-refactor tree for Home, Movies, homevideos, Feeds, ABS Podcasts and ABS Books and **passes** for TV and Music, and record the failing output in the commit message. Commit it together with group 2 so no committed state has a failing test.

## 2. The arrangement owns the horizontal frame

- [ ] 2.1 Add a `Copy` `WideHeroHeroPane { pane, content }` and return it from `wide_hero_hero_pane` (`src/app/render/arrangements/wide_hero.rs:296-317`); verify the pane's existing fill/focus tests pass and the returned `content` is still `padded_rect(hero_panel, PANE_PAD_X, PANE_PAD_Y)`, with the unconditional fill and `LeftPaneFocus` resolution unchanged.
- [ ] 2.2 Retype `wide_hero_hero_content_box` and `_with_surface` (`wide_hero.rs:449-481`) to take `WideHeroHeroPane` plus the caller's `y`/`height`; derive the box's left edge and width from `content` and keep the payload's `(PANE_PAD_X, PANE_PAD_Y)` inset and the surface variants unchanged; verify the arrangement's own box tests pass.
- [ ] 2.3 Update every call site the compiler reports — `components/hero.rs:604`, `components/tv_wide.rs:465/505/512`, `components/music_wide.rs:534`, `components/feeds.rs:206`, `components/audiobookshelf_podcast.rs:425`, `components/audiobookshelf_book.rs:176` — and verify `cargo check -p mbv` is clean and the task 1.2 invariant is green for all seven destinations.
- [ ] 2.4 Delete TV's two `saturating_sub(PANE_PAD_X)` / `saturating_add(PANE_PAD_X * 2)` compensations (`tv_wide.rs:459-465`, `:495-503`); verify TV's rendered buffer is byte-identical before and after (capture both and diff) — the compensation is now applied by the arrangement.
- [ ] 2.5 Check whether the landed surface migration already names TV's focused episode box (`tv_wide.rs:512-517`) through its `Surface::MainContentBox` identity; if it did, record the check as a no-op and change nothing, and if it did not, fold the manual `SURFACE_ACCENT_SOFT` repaint into the primitive's surface parameter while the call sites are being retyped. Verify either way that TV's focused-episode buffer differs from the unfocused one only by that surface role and that the episode rows and hit geometry are unchanged.
- [ ] 2.6 Drop the wide `SELECTED_BLOCK_SIDE_PADDING` inset in `render_book_hero` (`components/audiobookshelf_book.rs:422-425`) so the book hero text and the chapters box share one frame; verify the book hero text and the box's left edge are equal in a buffer assertion, and that Music and TV are untouched by this task.

## 3. Review the visible deltas

- [ ] 3.1 Capture before/after buffers for every moved surface (Home, Movies, homevideos, Feeds, ABS Podcasts, ABS Books) and verify each matches the intended frame and that no cell outside those surfaces' boxes changed, by diffing whole-buffer output across the conformance matrix.
- [ ] 3.2 Re-read every existing expectation that pinned the old offset — `tests_wide_hero_pane_characterization.rs`, `tests_home_characterization.rs`, `tests_library_characterization.rs`, `components/hero_tests.rs`, `components/tv_wide_tests.rs` — and verify each change is explained by the frame rule; delete a test that only pinned the drift rather than retuning it, and confirm none asserts an absolute pane coordinate or an arrangement return value.
- [ ] 3.3 Verify the Narrow path is untouched: the inline Home hero and every narrow destination still paint no main-content box (`overview_pad == 0` path) and their buffers are unchanged.

## 4. Gates, record, and hand-off

- [ ] 4.1 Run `cargo fmt --all`, `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, and `ast-grep scan`; verify all four are clean and report their raw output.
- [ ] 4.2 Run `openspec validate unify-wide-hero-content-box-frame`; verify it passes and that the spec delta matches what was implemented, then report to the parent for acceptance — archiving and syncing the delta into `openspec/specs/right-panel-arrangements/spec.md` is the parent's step after live review.
- [ ] 4.3 Report the four moved surfaces for live sampling (Home/Movies/homevideos overview box, Feeds hero box, ABS Podcasts episode box, ABS Books hero text + chapters box) and verify no other surface moved.
