# Tasks: media-list-viewport-scroll

Reference: `design.md` decisions D1–D9; spec deltas in `specs/mouse-input/` and
`specs/canonical-media-lists/`.
Run `cargo nextest run -p mbv` after each task; keep it green.

Sequencing note: `inline-search-media-list-rows` U2 has landed (legacy search painters, the ctx search
plumbing, and the second copy of the window rule in `plain_rows.rs`/`FixedRowPlan` are gone), so the rule
this change owns now has exactly one home in the tree. Rows below still share files with that change's
follow-up units; coordinate before editing the same file.

## 1. Owner: the viewport step (D1, D3)

- [x] 1.1 Add `MediaListOperation::ScrollViewport(i64)` and the owner method that applies it for a
  painted height: move the window one display row, clamp to the first/last display row, and drag the
  selection into the window only when the step would leave it outside (nearest selectable row inside;
  `Heading`/`Spacer` rows are never selected).
  (Verify: owner unit tests — step inside the window moves only the window; a step at the window's edge
  drags the selection to the nearest shown selectable row; both content ends clamp.)
- [x] 1.2 Add the page form of the same operation for a painted height, reusing the clamp and drag rule.
  This is a height-taking owner method, NOT a reuse of `MediaListOperation::Page` — that variant keeps its
  five-item selection meaning until 6.2 converts the last `PgUp`/`PgDn` arm (Queue, Home, Feeds, podcast,
  book, and Inline Search still route through the variant) and deletes it.
  (Verify: unit tests — a page moves by the height, a page longer than the remaining content clamps, and
  a page drags the selection the same way.)
- [x] 1.3 Handle the new operation exhaustively in `delegate_operation` and count a window-only move as a
  consumed step while a boundary no-op stays unhandled, so a viewport-only wheel step reports no selection
  move. A step never extends the live range: `multi_selection` and the anchored range are untouched even
  when the drag fires.
  (Verify: transition tests — `selected_target` is `None` for a window-only step and `Some` when the drag
  fired; disposition is consumed for a window move and unhandled at a content end; a step during Visual
  mode leaves the multi-selection unchanged.)

## 2. Owner: reachability and leading context (D4)

- [x] 2.1 Stop raising the window onto the cursor: a cursor move moves the window the minimum distance that
  shows the selection, and when that would place the selection on the window's first row with its labelling
  `Heading` directly above, the window moves one row further.
  (Verify: unit tests on a grouped fixture — a move to the first selectable row leaves the `Heading`
  painted; a stored window above the selection is preserved; a step may still scroll the label off.)
- [x] 2.2 Reconcile the existing clamp tests with the new rule and delete the assertions that pinned the
  edge-pinned window.
  (Verify: `cargo nextest run -p mbv` with the updated `components/media_list/tests.rs`.)

## 3. Owner: row-flow replacement anchor (D5)

- [x] 3.1 On `set_content`, record the first selectable target the previous flow showed at the window's top
  plus whether a `Heading` sat directly above it; after installing the new rows re-find that target and
  restore the window to its row — or to the `Heading` directly above it when the previous flow showed one
  there and the new flow still places one (structural match only; `Heading` has no stable identity). When
  the target is gone, keep the selection inside the window and clamp. No display-row index crosses the
  replacement.
  (Verify: unit tests — a reordered flow keeps the window on the same target; a regrouped flow restores
  the label above the target; a target whose new flow has no leading `Heading` anchors to the target's
  row; a missing target falls back; an append far from the window leaves it unchanged.)
- [x] 3.2 Drive the Music grouping settle case deterministically (commit the settled catalog directly; no
  waits) and assert the window keeps its place while the albums reorder.
  (Verify: an app-level test with a reordering settled catalog; the test must not sleep.)

## 4. Paint read-only (D2, D9)

- [x] 4.1 Delete the fixed-row painter's write-back of the resolved offset (`render/components/media_list/wide.rs`)
  and keep the display clamp only; replace `painter_persists_resolved_scroll_offset_across_frames` with a
  pin that a paint does not change the window and a shorter paint does not raise it.
  (Verify: the replaced test plus `cargo nextest run -p mbv`.)
- [x] 4.2 Resolve the step's height from the retained painted content rectangle in the carrier, and make
  `sync_viewport` a geometry clamp that no longer stores a resolved offset. With no retained frame the
  step is a no-op (`Unhandled`); a stale height's overshoot is display-only until the next paint clamp —
  the owner's window is never corrected from a stale height.
  (Verify: carrier unit tests — the height comes from the retained frame; a step with no retained frame
  changes nothing; a height change clamps without raising the window; the shared owner survives the
  transition.)
- [x] 4.3 Keep the panel's per-frame height sync as the single geometry seam and update its tests.
  (Verify: `library_panel/panel_list.rs` and `panel_tests` green.)

## 5. Wheel conversion (D1, D8)

- [x] 5.1 Map `MediaListSurfaceInput::Wheel` to `ScrollViewport` in the shared conversion so every
  carrier-backed surface steps its viewport.
  (Verify: component tests — the wheel moves the window one row at wide and narrow heights.)
- [x] 5.2 Convert the direct `Move` build in the Emby owner and gate its `EmbyLibraryCursorIndex` echo on an
  actual selection move, reporting the window's reached row for pagination instead. Confirm the
  pending-fetch guard means repeated viewport position reports at the loaded end do not trigger repeated
  fetches — a scroll-only wheel reader at the bottom must not spam pagination.
  (Verify: tick test — a viewport-only wheel step emits no cursor index and still reports position;
  pagination still fires near the loaded end; repeated reports at the loaded end fire at most one fetch.)
- [x] 5.3 Convert the Music album rail and track-list wheel arms, gating the album cursor request on a
  dragged selection.
  (Verify: the updated `tests_tick_integration_music_mouse` assertions plus an album-rail wheel test.)
- [x] 5.4 Convert the Queue, Home, Feeds, TV, podcast, book, and Inline Search wheel arms.
  (Verify: per-surface component or tick test that the window moves one row and the selection rides only
  at the window's edge.)

## 6. Keyboard chords (D6, D7)

- [x] 6.1 Add the one-row viewport chord (`Ctrl+e` / `Ctrl+y`) and the page step on `PgUp`/`PgDn` to the
  Emby library, Music album and track, and TV lists.
  (Verify: key tests per surface at wide and narrow heights.)
- [x] 6.2 Add the same chords to the Queue, Home, Feeds, podcast, book, and Inline Search lists.
  (Verify: key tests per surface.)
- [x] 6.3 Admit the two Ctrl chords in Feeds ahead of its blanket Ctrl/Alt early-return, leaving every
  other Ctrl/Alt chord rejected.
  (Verify: a Feeds key test for both chords and for a still-rejected Ctrl chord.)
- [x] 6.4 Add the viewport chord to the Help overlay's key list and confirm its `PgUp / PgDn` row now
  describes the page step truthfully.
  (Verify: the Help key-list test and a painted-frame assertion.)

## 7. Retire the hand-patches (D9)

- [ ] 7.1 Delete the Queue's `scroll.min(cursor)` hand clamp, keeping the explicit scope reset.
  (Verify: updated Queue tests, including the `set_cursor` cases.)
- [ ] 7.2 Replace the item-index restore derivation (`BrowseLevel::scroll_for_cursor`) with seeding the
  selection and deriving the window from its display row at the restore boundary; update the position tests
  that pinned the old derivation.
  (Verify: `tests_library_position*.rs` and `actions_tests_letter.rs` green with the new expectations.)
- [ ] 7.3 Make the Music re-anchor adopt the target and window it is given, with no bottom-edge
  derivation; update the re-anchor characterization and owner tests.
  (Verify: `tests_music_wide_reanchor_characterization.rs` and the owner tests green.)
- [ ] 7.4 Update the remaining carrier/window tests that used the write-back or `set_scroll` as their seam
  (non-Wide library list, feeds, TV, library-panel integration).
  (Verify: `cargo nextest run -p mbv` green.)

## 8. Evidence and gates

- [ ] 8.1 Add tick-integration evidence through `Application::tick()`: a wheel step and the viewport chord
  each move the window one row, drag the selection only at the edge, and reach display row 0 on a grouped
  list in Wide and non-Wide geometry.
  (Verify: new cases in the `tests_tick_integration_*` suites.)
- [ ] 8.2 Update the interactive-surface ledger rows for the converted surfaces and confirm each names its
  viewport-step proof.
  (Verify: `docs/architecture/interactive-surface-ledger.md` updated alongside the spec verification record.)
- [ ] 8.3 Add the change's domain terms (the viewport and its step) to `CONTEXT.md` and use them
  consistently in the code and artifacts.
  (Verify: terms present, no collision with the existing `MediaList`/`Group heading` entries.)
- [ ] 8.4 Full gates.
  (Verify: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo nextest run -p mbv` and `-p mbv-core`; `openspec validate --all`.)
