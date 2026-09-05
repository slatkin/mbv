## 1. Baseline and the shared primitives

- [x] 1.1 Add characterization buffer tests capturing the current Wide left-pane output of all
  seven hero-on-left destinations (Movies/home-videos/Emby-podcasts/feed-group browser, TV,
  Music, Home with a non-Emby Latest selection, Feeds with and without a selected entry, ABS
  Books, ABS Podcasts) — verify `rtk cargo nextest run -p mbv` passes and the new tests are in
  their own commit before any paint change, per the ledger migration flow.
- [x] 1.2 Record the pre-existing `Block::default().style(<Color>)` hits across the tree — verify
  the list is captured in the change folder or the PR body, so task 5.3 can land a clean rule.
- [x] 1.3 Add `LeftPaneFocus { ReadOnly, Workspace(bool) }` and
  `hero_on_left_pane(f: &mut Frame, content_area: Rect, focus: LeftPaneFocus) -> Option<Rect>` to
  `src/app/render/arrangements/hero_left.rs`, next to `hero_on_left_list_panel_border`. It calls
  `shared_hero_presentation(content_area)?` itself (callers cannot supply a pane extent), fills
  the resulting `left_panel` with `SURFACE_RESTING` for `ReadOnly` and
  `resolve_surface_focus(held)` for `Workspace(held)`, and returns
  `padded_rect(left_panel, PANE_PAD_X, PANE_PAD_Y)` — verify a unit test in that module asserts
  the returned rect's offsets, that `ReadOnly` never resolves to the focused surface, and that a
  sub-breakpoint `content_area` returns `None` without painting.
- [x] 1.4 Rename `hero_on_left_recessed_box` to the committed "main content box" name, **remove
  its `pad_x`/`pad_y` parameters** (D9), and hard-code `(PANE_PAD_X, PANE_PAD_Y)` inside it;
  update its doc comment to state it is present on every hero-on-left surface with a
  kind-dependent payload and one padding value — verify `rtk cargo check -p mbv` passes with all
  four call sites updated (`tv_wide.rs:369`, `:392`, `music_wide_tracks.rs:28` are argument-only
  changes; `hero.rs:604` previously passed `overview_pad, 1`, which review confirmed was already
  `(2, 1)` in every reachable state — so this call site is behavior-preserving too, not a D9 shift).

## 2. Per-surface conformance: the four broken destinations

- [x] 2.1 ABS Podcasts (`audiobookshelf_podcast.rs:~222-260`): call `hero_on_left_pane(frame,
  area, LeftPaneFocus::Workspace(focused && interaction.episode_selection.is_some()))` before
  `render_podcast_hero` and pass its returned rect as the hero's content rect, replacing the
  `SELECTED_BLOCK_SIDE_PADDING` inset at `:429-432`. This is the surface **gaining** focus-green
  under D8 — passing the in-scope bare `focused` is the specific mistake to avoid — verify the
  surface's characterization test shows a filled resting-surface left pane with the show list
  focused, and a focused-surface left pane with an episode selected.
- [x] 2.2 ABS Books (`audiobookshelf_book.rs:182-187`): replace the `.style(<Color>)` call with
  `hero_on_left_pane(frame, area, LeftPaneFocus::Workspace(focused &&
  interaction.chapter_selection.is_some()))`, take the hero content rect from its return value
  instead of `panes.left_area`, and delete the dead outer `right_panel` paint at `:208-213` —
  verify the characterization test shows a filled pane in both focus states and `rtk cargo clippy
  --workspace --all-targets` is clean.
- [x] 2.3 Feeds (`feeds.rs:198-209`): hoist the pane fill out of the `if let Some(entry)` guard so
  it is unconditional, passing `LeftPaneFocus::ReadOnly` — verify the no-selection
  characterization test shows a filled resting pane with no hero content, and that the pane stays
  resting in every focus state Feeds can reach.
- [x] 2.4 Home (`home.rs`): delete the `HeroData::Generic` height clamp at `:237-242`, make the
  fill unconditional via `hero_on_left_pane(..., LeftPaneFocus::ReadOnly)` at `:244-249`, and keep
  the content-height calculation `hero_content.height = rows.min(hero_col_height)` at `:231`.
  Change `render_home_latest_detail`'s cover anchoring from bottom-of-rect to top-anchored with
  the text, or the full-height pane strands the cover (design.md Risks) — verify the non-Emby Home
  characterization test shows a full-height filled pane with top-anchored content and cover.

## 3. Per-surface conformance: the three already-filling destinations

- [x] 3.1 Movies/home-videos/Emby-podcasts/feed-group browser
  (`src/app/components/browser/paint.rs`): route the fill through `hero_on_left_pane(f, body_area,
  LeftPaneFocus::ReadOnly)`, take the hero content rect from its return value, and **delete** the
  second inset at `:82` (`padded_rect(left_area, PANE_PAD_X, 0)`) — the effective inset today is
  `(4, 1)`, not `(2, 0)`. Also delete `left_panel.height = left_content_area.height;` at `:57` and
  the now-orphaned `left_panel`/`left_content_area` bindings; keep the `wide_library_panes` call
  at `:35`, which still produces `right_area` — verify the Movies characterization test diff shows
  a **two-column horizontal shift and no row shift**, and nothing else.
- [x] 3.2 TV (`tv_wide.rs:188`, `:201-205`): route the fill through `hero_on_left_pane(f, area,
  LeftPaneFocus::Workspace(ctx.focused && ctx.episode_cursor.is_some()))` and take the hero
  content rect from its return value. Keep the `wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y)`
  call for `right_area` — its `pad_y` also shapes the right pane (`arrangements/library.rs:19-23`)
  — verify the TV characterization test is unchanged (TV already uses the shared inset and focus
  resolution).
- [x] 3.3 Music (`music_wide.rs:99`, `:515-519`): route the fill through `hero_on_left_pane(f,
  area, LeftPaneFocus::Workspace(ctx.focused && ctx.track_cursor.is_some()))` and take the left
  content rect from its return value instead of `panes.left_area`. Keep the
  `wide_library_panes(area, 0, PANE_PAD_Y)` call for `right_area`; do not change its arguments —
  verify the Music characterization test diff shows only the left-pane horizontal inset change and
  the right rail is byte-identical.
- [x] 3.4 Delete the redundant one-row `SURFACE_BACKDROP` strips below the left pane at
  `browser/paint.rs:60-68`, `music_wide.rs:497-506`, and `audiobookshelf_book.rs:174-181` —
  verify all three characterization tests show no painted row between the left pane's bottom and
  the status bar, and that the pane still bottoms out exactly one row above the status bar.
- [x] 3.5 Extend the main content box to the three destinations that lack it — ABS Books (chapter
  listing), ABS Podcasts (episode listing), Feeds (entry description + metadata) — carrying each
  surface's existing payload into the shared primitive, and paint the box even when its payload is
  empty. **Feeds gains a box it has never had (D11); flag it for live review, do not treat it as
  incidental** — verify each surface's characterization test shows the backdrop inset in both the
  populated and empty-payload states, and that all three use the primitive's single padding value.
- [x] 3.6 Grep for surviving left-pane paint outside the primitive
  (`bg(palette::SURFACE_RESTING)`, `bg(palette::resolve_surface_focus` on a left-pane rect,
  `padded_rect(left_area`, and any `left_panel` mutation) across
  `src/app/render/components/` and `src/app/components/browser/` — verify the only remaining hits
  are inside `arrangements/hero_left.rs`.

## 4. One UI-level `Hero` abstraction, with artwork

- [x] 4.1 Add a `SURFACE_ARTWORK_PLACEHOLDER` semantic role to `src/app/render/theme/` and a
  `render_artwork_placeholder(f, area)` component that paints it — verify the role is not a
  re-exported primitive and a unit test asserts the painted extent matches the requested rect.
- [x] 4.2 Define the initial `Hero` trait in `src/app/render/components/` and implement it for
  `EmbyItem` — verify `rtk cargo check -p mbv` passes with the Emby hero path routed through the
  trait and all characterization tests unchanged from their phase-3 state.
- [x] 4.2a Narrow `Hero` to title, ordered metadata, optional description, and semantic artwork;
  delete its listing body and give the shared arrangement named artwork, overview, and optional
  media-list viewport slots. Keep `Rect`, targets, cursor, scroll, and hit state out of `Hero` —
  verify the existing Home/Emby path remains unchanged and the compiler finds no `HeroBody::Listing`.
- [x] 4.2b Establish the shared stacked-artwork/title gap policy: when Music Wide stacks album
  art above title with images on, reserve exactly one blank row; do not apply it images-off or to
  side-by-side layout. Update the existing Music publish-versus-paint geometry test or add the
  smallest durable geometry assertion that catches a lost row.
- [x] 4.2c Route TV Wide through the Hero slots: use shared landscape geometry and a locally
  verified Emby artwork candidate chain (with `Thumb` first only when supported by the client
  mapping), then render title, ordered metadata, blank row, and an overview main-content box.
  Re-baseline the existing TV buffer characterization as the intended visual delta.
- [x] 4.2d Replace TV's hand-painted episode table with a parent-owned embedded
  `WideMediaList<String>` in a separate recessed media-list box below the overview; season pills
  remain parent chrome. Preserve component-owned selection, scroll, viewport anchoring, and hit
  resolution through the canonical control; add only the smallest buffer/integration coverage for
  the two boxes and canonical episode rows.
- [x] 4.2e Migrate Music tracks and Audiobookshelf episode/chapter left workspaces individually to
  parent-owned embedded `WideMediaList` controls in their Hero media-list slots. Preserve each
  parent's typed targets, local selection, scroll/anchor, and hit semantics; do not introduce a
  media-list trait. Verify each migration with the existing narrowest interaction/buffer coverage.
- [x] 4.3 Make the images-off collapse a layout decision, not a `HeroArtwork` variant (D10): the
  hero layout takes the global images setting and returns `Option<Rect>` for the artwork region,
  `None` when images are off, with text and metadata taking the full content width — verify a test
  renders two different surfaces with images off and asserts both collapse identically with no
  reserved or placeheld region.
- [x] 4.4 Implement `Hero` for the Audiobookshelf/generic entry and for feed entries (D11: no
  Feeds exception), and route `render_home_hero_content` (`home_hero.rs:483-521`) through the
  trait — verify the renderer contains no `match` on `HeroData` variants, the non-Emby Home
  characterization test is unchanged from its task-2.4 state, and Feeds renders an image region.
- [x] 4.5 Route every hero artwork region through `render_artwork_placeholder` when `artwork()`
  returns `Placeholder`, on all seven destinations, so no hero renders an empty image region while
  images are on — verify a test renders an artwork-less item on Feeds and on Home's non-Emby
  Latest path and asserts the artwork region is filled.
- [x] 4.6 Confirm the no-artwork placeholder is distinct from the existing loading placeholder
  (`album_art.rs:191-195` `BORDER_UNFOCUSED`) at the call-site level — verify no call site uses one
  to mean the other, so "still loading" stays distinguishable from "has none".
- [x] 4.7 Delete the `HeroData::Generic` variant (`home_hero.rs:97`) and collapse `HeroData` to the
  layout-carrying form — verify `rtk cargo check -p mbv` passes and `grep -rn "HeroData::" src/`
  shows no `Generic` hit, so the clamp deleted in 2.4 has no branch to return to.
- [x] 4.8 Measure `home_hero.rs` and split it if it crossed 800 lines — verify `rtk make
  check-code-file-lines` is clean.

## 5. Drift-proofing

- [ ] 5.1 CUT FROM SCOPE (2026-09-05, user decision): new conformance-matrix left-pane
  background-cell assertions across all seven surfaces. Judged not worth the added
  test-infrastructure weight for this change; revisit only if a real regression surfaces here.
- [ ] 5.2 CUT FROM SCOPE (2026-09-05, user decision): file split that existed only to host 5.1's
  new assertions. No longer needed since 5.1 is cut.
- [ ] 5.3 Add an `ast-grep` rule under `rules/frontend-boundary/` rejecting
  `Block::default().style(<expr of type Color>)` and requiring explicit `.bg()`/`.fg()`, with
  fixtures — verify `rtk ast-grep test` passes and the unscoped `rtk ast-grep scan` reports zero
  findings tree-wide (fix or separately file any pre-existing hits recorded in task 1.2; a
  standing baseline is not a conforming resolution).
- [ ] 5.4 CUT FROM SCOPE (2026-09-05, user decision): main-content-box conformance assertion
  across all seven surfaces. Same rationale as 5.1 — not worth the added test-infrastructure
  weight for this change.

## 6. Canon

- [ ] 6.1 Add the "Hero pane" term to `CONTEXT.md`'s Presentation section (`#333c43`
  `SURFACE_RESTING` fill of the hero-on-left left pane; `_Avoid_`: recessed box, hero panel,
  detail panel) and the "Main content box" term (the `#2d353b` `SURFACE_BACKDROP` inset within it,
  kind-dependent payload, one padding value; `_Avoid_`: overview box, recessed box) — verify the
  two terms are unambiguously distinguishable to a reader who has seen neither.
- [ ] 6.2 Add `hero_on_left_pane`, `LeftPaneFocus` and the `Hero` trait to the `mbv-frontend`
  skill's reuse list (`.agents/skills/mbv-frontend/SKILL.md` ~`:125` and ~`:195`) and mirror the
  edit to `.opencode/skills/mbv-frontend/SKILL.md` — verify both copies are identical.
- [ ] 6.3 Run the full gate set: `rtk cargo fmt`, `rtk cargo clippy --workspace --all-targets`,
  `rtk cargo nextest run -p mbv`, `rtk ast-grep scan`, `rtk make check-code-file-lines` — verify
  all pass clean.
- [ ] 6.4 SHRUNK FROM SCOPE (2026-09-05, user decision): one manual pass, not a full
  selection-state matrix. Run the app, view all seven destinations once at Wide geometry with an
  artwork-less item and images off — confirm each shows a filled pane and no right-column
  backdrop bleed, and that Feeds' new image region and main content box look intended.
