Steps 1–5 are separate PRs, in order. Step 5 repeats per surface and is tracked in
`ledger.md`. Nothing in step 5 starts before step 1 merges.

## 1. Split the render tree (PR 1 — maintainer-decided, mechanical)

Each sub-task below moves one file group and is its own commit within this PR;
the groupings reuse `ledger.md`'s surface list for consistency. Move only — no
logic edits, except splitting functions that both read app state and paint at
their seam (state half to `screens/`, painting half to `components/`). Existing
render tests pass unchanged throughout.

- [x] 1.1 Scaffold `src/app/render/{screens,arrangements,components,theme}/`
      module directories and wire empty `mod.rs` files into `render/mod.rs`. No
      code moved yet.
- [x] 1.2 Move the shared/non-surface group per the classification table in
      `design.md`: `card.rs`, `widgets.rs`, `indicators.rs`, `pills.rs`,
      `chrome*.rs`, `sort_filter.rs`, `visualizer.rs`, `overlays/backdrop.rs`,
      `overlays/modal_frame.rs`.
- [x] 1.3 Move the Emby standard library group: `list.rs`, `list_rows.rs`,
      `list_letter_groups.rs`, `list_plain.rs`, `movies_wide.rs`, `tv_wide.rs`,
      `detail.rs`, `detail_series.rs`, `detail_series_view.rs`, `hero.rs`,
      `hero_left.rs`.
- [x] 1.4 Move the Emby home-video group: `home_video.rs`.
- [x] 1.5 Move the Emby music/album group: `music.rs`, `music_wide.rs`,
      `music_wide_browser.rs`, `album.rs`, `album_art.rs`, `album_cursor.rs`,
      `album_detail.rs`, `album_plan.rs`, `album_rows.rs`.
- [x] 1.6 Move the Home screen group: `home.rs`, `home_hero.rs`,
      `home_latest_row.rs`, `home_list_rows.rs`, `home_pills.rs`, `home_feed.rs`.
- [x] 1.7 Move the Feeds screen (`feeds.rs`, 792 lines), splitting it during the
      move so it drops under the 800-line cap.
- [x] 1.8 Move the Audiobookshelf podcast surface (`audiobookshelf.rs`, 702
      lines), splitting during the move.
- [x] 1.9 Move the Audiobookshelf book group: `audiobookshelf_books.rs`,
      `audiobookshelf_book_browser.rs`.
- [x] 1.10 Move the Search sidebar (`search_sidebar.rs`).
- [x] 1.11 Move the remaining overlay surfaces: `overlays/settings.rs`,
      `overlays/playlists.rs`, `overlays/sessions.rs`, `overlays/feeds_manage.rs`,
      `overlays/help.rs`, `overlays/remote_reanchor.rs`,
      `overlays/library_routes.rs`, `overlays/context_menu.rs`,
      `overlays/daemon_lost_modal.rs`, `overlays/confirm_modal.rs`,
      `overlays/multiselect.rs`.
- [x] 1.12 Move `queue.rs` (697 lines) and fold the remaining top-level
      `render/mod.rs` contents (715 lines) into the new module tree, splitting
      both during the move so nothing crosses the 800-line cap.
- [x] 1.13 Verify: `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
      --all-targets`, `rtk make check-code-file-lines`. No test file edited except
      for module paths.
- [x] 1.14 Add the new domain terms (component, arrangement, bespoke surface,
      policy, variant) to `CONTEXT.md` under Presentation, per the repo
      term-coordination rule.

## 2. Make palette primitives private (PR 2 — compiler-driven)

Each fix-up sub-task below covers one file group (same groupings as step 1) and
is its own commit within this PR.

- [x] 2.1 Move the raw `Color` constants from `src/app/palette.rs` into a private
      module under `render/theme/`; leave only semantic roles public.
- [x] 2.2 Fix compiler errors in the shared/non-surface group and the new
      `theme/`/`components/` core files. Where no existing role fits, add a named
      role rather than re-exporting the primitive.
- [x] 2.3 Fix compiler errors in the Emby standard library group.
- [x] 2.4 Fix compiler errors in the Emby home-video group.
- [x] 2.5 Fix compiler errors in the Emby music/album group.
- [x] 2.6 Fix compiler errors in the Home screen group.
- [x] 2.7 Fix compiler errors in the Feeds screen.
- [x] 2.8 Fix compiler errors in the Audiobookshelf podcast and book groups.
- [x] 2.9 Fix compiler errors in the Search sidebar and remaining overlay
      surfaces.
- [x] 2.10 Fix compiler errors in `queue.rs` and any remaining render files.
- [x] 2.11 Review the added roles as the substantive output of this step. A role
      that is a rename of one primitive with no semantic meaning is a bypass.
- [x] 2.12 Verify no behaviour change: existing buffer tests pass with no
      expected-output edits.

## 3. Guidance and bypass checks (PR 3)

- [x] 3.1 Add mandatory TUI ownership rules to `AGENTS.md`: screens do not call
      Ratatui, construct rects, or compute hit targets; overrides live in the central
      owner; new UI work follows the boundary from this PR forward.
- [x] 3.2 Add `.opencode/skills/mbv-frontend/SKILL.md` with the reuse workflow, the
      controlled-override decision table, Ratatui patterns, and a completion checklist
      covering component ownership, narrow-width behaviour, interaction targets, and
      buffer tests.
- [x] 3.3 Include worked examples distinguishing content changes, named policies,
      central variants, new components, and bespoke surfaces — showing that none of
      them permits screen-owned geometry.
- [x] 3.4 Add ast-grep rules scoped to `render/screens/`: `use ratatui::`,
      `render_widget`, `Layout::`, `Rect` construction, `buffer_mut`.
- [x] 3.5 State in the skill what the checks do not catch — duplicated arrangement
      geometry, hit targets drifted from painting — so review knows what it owns.
- [x] 3.6 Run the checks against the post-step-1 tree and confirm the tooling
      itself is correct: each rule fires on a real bypass, none fire on test
      files, and none fire on files outside `screens/`. Step 1 was a file-level
      move, not a per-function split, so `screens/` is not clean yet — the
      unmigrated `ledger.md` surfaces (including `queue.rs` and `pills.rs`,
      despite being listed there as already handled) still contain raw
      Ratatui calls the checks correctly flag. These checks are a ratchet
      enforced on new and touched code from this PR forward; full-tree
      cleanliness is reached incrementally as step 5 migrates each surface,
      per the proposal's explicit rejection of a whole-tree-or-nothing gate.

## 4. Hit-target ownership: design gate (PR 4 — design only)

- [x] 4.1 Read `src/app/input_mouse.rs`, `input_mouse_dispatch.rs`, and
      `input_mouse_panels.rs` and write down how each surface currently resolves a
      click.
- [x] 4.2 Answer, in writing: where the hit map is stored between frames; what
      invalidates it (resize, tab switch, scroll, repaint-less state change); what a
      mouse event does before first paint and after resize; whether components publish
      into a shared map or return one arrangements aggregate.
- [x] 4.3 Go/no-go. On go, add the typed hit-map contract and migrate every surface
      that handles mouse in one PR. On no-go, record the reason, keep the existing
      coordinate arithmetic, and drop the hit-target requirement from the delta spec.
      **Decision: no-go.** See `design.md` step 4 for the full written design and
      reasoning; the delta-spec edit itself is made in step 6.3.
- [x] 4.4 Do not ship a partial migration. Some surfaces on hit maps and the rest on
      coordinate arithmetic is worse than either alone. Satisfied by the no-go: no
      surface migrates, so none can end up partially migrated.

## 5. Per-surface migration (repeats — one PR per surface, tracked in `ledger.md`)

Migrate the 14 single/dual-file surfaces first (ledger rows 2, 5–19); they're
mechanical and validate the flow cheaply. The 3 multi-file surfaces (rows 1, 3,
4) go last and need an extra step: none of them has an arrangement yet for
their shared Rect/Layout math, unlike hero-bearing screens
(`arrangements/hero_left.rs`). Confirmed on `list.rs` alone (row 1): ~15 raw
`Rect` constructions and 2 direct `ratatui::` imports, with no existing
pattern to reuse.

### 5.1–5.7 apply to each single/dual-file surface (rows 2, 5–19)

- [ ] 5.1 Commit 1: characterization `TestBackend` buffer test capturing current
      output — default, focused, narrow-width, and selected states. No production
      code in this commit.
- [ ] 5.2 Commit 2: route the surface through arrangement/component ownership. The
      characterization test is unchanged and still passes.
- [ ] 5.3 Record the surface's hero additional-content style (Movie overview, TV
      seasons/pills and episodes, Music tracks, or a mapped provider-specific style)
      if it is hero-bearing.
- [ ] 5.4 Represent screen differences as typed content models or named central
      policies, not screen-local painter branches.
- [ ] 5.5 If the surface genuinely cannot reuse the vocabulary, register it as a
      named bespoke component with its reason and its own buffer coverage. It still
      obeys ownership, semantic styling, and verification rules.
- [ ] 5.6 Tick the surface off in `ledger.md` in the same PR.
- [ ] 5.7 Repeat 5.1–5.6 until all 14 rows in this group are ticked.

### 5.8–5.15 apply to each multi-file surface (rows 1, 3, 4 — in ledger order)

- [ ] 5.8 Commit 1: characterization tests for every screen file in the surface
      that doesn't already have one, at the same four states as 5.1.
- [ ] 5.9 Commit 2: extract the surface's shared Rect/Layout math into a new
      arrangement module, following the `Rect`-in/`Rect`-out shape of
      `arrangements/hero_left.rs` (e.g. `hero_on_left_panes`) — no app state in
      the new functions.
- [ ] 5.10 Commit 3+: route each screen file in the surface through the new
      arrangement and existing components, one file per commit.
      Characterization tests unchanged and still pass after each commit.
- [ ] 5.11 Record hero additional-content style for any hero-bearing screen in
      the surface.
- [ ] 5.12 Represent screen differences as typed content models or named
      central policies, not screen-local painter branches.
- [ ] 5.13 If a screen genuinely cannot reuse existing vocabulary, register it
      as a named bespoke component with its reason and its own buffer
      coverage.
- [ ] 5.14 Tick the surface off in `ledger.md` in the same PR.
- [ ] 5.15 Repeat 5.8–5.14 until all 3 rows in this group are ticked.

## 6. Sync specs

- [ ] 6.1 Sync `right-panel-arrangements` with the post-#584 hero-on-left-wide /
      selected-row-replacement-narrow baseline.
- [ ] 6.2 Sync `library-list-hero` and `ui-design-language` with the tightened
      ownership and the private-primitive theme API.
- [ ] 6.3 Reconcile the `ui-design-system` delta spec with step 4's outcome.
- [ ] 6.4 `openspec validate` passes.
