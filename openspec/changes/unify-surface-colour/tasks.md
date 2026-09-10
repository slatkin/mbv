## 1. The one behaviour kept from `focus-aware-column-surface`

- [ ] 1.1 `FrameChromeGeometry` carries the right column's focus: add `right_focused` (`src/app/layout.rs`) and derive it in `src/app/render/arrangements/chrome.rs` as `right_visible && PanelFocus::Library`, false whenever the right column is not visible. Verify: geometry tests cover wide `LibraryOnly`, wide `Both` with library focus, wide `Both` with queue focus, `QueueOnly`, and the narrow widths — including that a stale `Library` focus with no visible column yields false.
- [ ] 1.2 The right-column backdrop resolves from that bit: `render_legacy_backdrops`' right arm (`src/app/render/components/chrome.rs`) renders the focused column surface (`#3c4841`) while the library column holds focus and today's `#2d353b` (`SURFACE_BACKDROP`) otherwise, with the left/queue arm untouched. Verify: a buffer test asserts the gutter around a focused library panel at wide `LibraryOnly` and wide `Both`, and the resting value once the queue takes focus.
- [ ] 1.3 The playback panel owns only its content rows: the panel must not paint the row directly above the pill bar in the wide layouts, so that row shows the library column surface from 1.2; `QueueOnly` keeps painting it with its `#2d353b` fill and its `On Now:` title (`src/app/render/components/chrome_player.rs`). Verify: a buffer test shows the row above the pill bar following the library column surface in both focus states while the panel's own rows keep exactly their current colours, and the `QueueOnly` row and title are unchanged.
- [ ] 1.4 Run the gates for section 1: `cargo nextest run -p mbv`, `cargo check -p mbv`, `cargo clippy --workspace --all-targets`, `cargo fmt --all`, `ast-grep scan`. Verify: all pass with no new warnings.

## 2. The inventory

- [ ] 2.1 Classify every production paint site: for each site that writes a surface colour, record `file:line`, the surface identity it is, its nesting level, its owner painter, the rect source, and the focus state it depends on. Cover every production hit of the surface roles and resolvers in `src/app` excluding `theme/`, the re-export modules and tests, and every site the six-mechanism audit names (direct role names, duplicate roles, `SURFACE_BACKDROP`'s six meanings, per-screen colour bits, shell-owned geometry). Verify: the table names a surface identity for every site and every site in the grep appears; the sites that change no colour are listed as such with a reason.
- [ ] 2.2 Reconcile the inventory against the audit in `proposal.md`: state which sites are reachable by a role edit, which are skipped by it, and which choose their own colour. Verify: the numbers in `proposal.md` are either confirmed from the current tree or corrected in place.

## 3. One focus state

- [ ] 3.1 Add `FocusState` (active column plus the active sub-surface) and compute it once per frame in the shell; thread it into `FrameChromeGeometry` and the screens' render contexts in place of `queue_focused` / `right_focused`. Choose names that do not collide: `right_focused` already means the Wide-hero right *pane* bit in `tv_wide.rs` / `music_wide.rs` — a different surface with a different resting value — so the collapsed state names its column explicitly (`Column::Right`). Verify: a geometry/state test shows the value changing with `PanelFocus` and the visible mode, no screen computes its own colour bit, and no name in the new state duplicates a per-screen sub-focus bit.
- [ ] 3.2 Delete the per-screen colour bits (`episode_focused` / `track_active` / `chapter_focused` as colour inputs) wherever they exist, keeping the same values as behaviour inputs for selection gating, cursor ownership and hit geometry. Verify: each screen's panel colours depend only on the supplied `FocusState`, proven by a buffer test per screen that moves the inner selection and observes no fill change.

## 4. One surface table

- [ ] 4.1 Add the closed `Surface` identity set and `surface_colors(surface, &FocusState) -> SurfaceColors`, one row per surface carrying its level and the role it resolves. Verify: a unit test asserts every surface resolves both focus states, and that a level's surfaces share the level's role values while other levels do not.
- [ ] 4.2 Migrate one screen at a time onto the table (Home, Feeds, library rails, TV, Music, ABS books, ABS podcasts, the queue column and panel, the playback strip, the modal frames), each as its own commit, deleting that screen's role names and resolver calls. Verify per screen: `cargo nextest run -p mbv` passes with no expected-value change, and the diff contains no `SURFACE_*` name or resolver call in that screen.
- [ ] 4.3 Retire the duplicate and multi-meaning roles the inventory proves are aliases, one at a time, each with the picture unchanged. Verify: `cargo nextest` green with no expected-value edit per role, and the audit's alias list empty at the end.

## 5. Guardrails

- [ ] 5.1 Add the ast-grep rule that fails on surface role names and `resolve_surface_*` calls in production screens outside `src/app/render/theme/`, and wire it into `ast-grep scan`. Verify: `ast-grep scan` passes on the migrated tree and fails when a role name is reintroduced in a screen (prove it once, then revert the probe).
- [ ] 5.2 Add the conformance test enumerating every `Surface` × breakpoint × focus state, asserting the rendered rect's fill equals the table's value. Verify: the test covers every table row, fails if a row's painter is changed to another level's colour (prove it once, then revert), and its expectations are generated from the table rather than duplicated by hand.

## 6. Verify

- [ ] 6.1 Run the gates on the finished change: `cargo nextest run -p mbv`, `cargo check -p mbv`, `cargo clippy --workspace --all-targets`, `cargo fmt --all`, `ast-grep scan`. Verify: all pass with no new warnings, no test deleted or weakened, and no expected colour value changed except where a task states it should.
- [ ] 6.2 Post the migration report: the inventory's coverage, the aliases retired, the level edits that now reach every surface, and the roles left with more than one meaning. Verify: each claim is reproducible from the tree (a grep or a test name), and anything still aliased is recorded as an open item rather than left silent.
