# Issue #927 — one re-anchor rule for the media list and the tree browser

## Context

The rule for which operations restore a freely scrolled viewport to following
the selection is hand-written in two places that already disagree:

- `MediaList::re_anchor_selection_following`
  (`crates/mbv-components/src/media_list.rs:257`) keeps a hand-maintained
  positive list of operations. Pointer `Context(target)` and `Scroll` are not
  on it.
- `TreeBrowser` calls `re_anchor_following()` per arm in
  `crates/mbv-components/src/list/tree_browser/operations.rs:432`
  (`apply_operation`, `apply_cursor_operation`). `Scroll`, `Right`,
  `ToggleExpansionTarget`, and the filter ops do not re-anchor.

The spec already states the rule once
(`openspec/specs/shared-list-components/spec.md:291`): a freely scrolled
viewport returns to following the selection on the next keyboard operation on
the list, or the next operation that moves or sets the selection (click,
select, restore, shell re-anchor). The wheel scroll is the release.

Ad hoc fix on the main branch, no OpenSpec change (user decision).

## Decisions (user-confirmed)

- **Curated exclusions.** Default rule: every operation re-anchors except the
  wheel `Scroll`, the tree's pointer `ToggleExpansionTarget`, and the tree's
  filter ops (`EditFilter`, `ClearFilter`, `ClearMarks`). Tree `Right` gains
  re-anchor — it is a keyboard operation, so the spec requires it. This fixes
  a latent gap; `ClearMarks` keeps its current non-anchoring behavior.
- **One dispatch site each.** The shared `Cursored`/`Viewported` traits never
  see the operation vocabulary, so the rule stays at each owner's dispatch
  site, stated once and identical in both places.
- **Keep the pointer-context difference.** The tree's pointer right-click
  rides `Select(target)` (sets the selection, re-anchors); the media list's
  pointer right-click sends `Context(target)` without selecting and keeps
  free scroll. Both stay as-is; the media-list exclusion becomes an explicit,
  commented rule instead of a silent miss.

## Findings

- The apparent `Context` divergence is two different operations: the tree's
  `TreeOperation::Context` is keyboard-only
  (`crates/mbv-components/src/tv_content/keyboard.rs:321`) and matches the
  media list's keyboard `ContextCurrent`; the media list's pointer
  `Context(target)` comes from `ContextClick`
  (`crates/mbv-components/src/media_list/selection.rs:20`).
- `MediaList`'s positive list already covers every variant except `Scroll(_)`
  and `Context(_)`, so restating it as the default rule changes no media-list
  behavior.
- Tree arms that re-anchor today: Move, Page, First, Last, Parent,
  AnchorSelection, Select, PointerToggleMark, Activate, Context. Arms that do
  not: Scroll, Right, ToggleExpansionTarget, EditFilter, ClearFilter,
  ClearMarks.
- `AnchorSelection` must keep its re-anchor *before* `anchor_selection_to`
  writes the persisted offset; hoisting the call to the top of
  `apply_operation` preserves that order.

## Approach

State the rule once on `ViewportAnchor`'s docs in
`crates/mbv-components/src/list/viewport.rs` (the shared anchor type), then
express it as one guarded call per owner:

- Media list: `re_anchor_selection_following` becomes a negative match —
  return early for `Scroll(_)` and `Context(_)`, otherwise set
  `FollowSelection`. No behavior change.
- Tree: remove the ten per-arm `re_anchor_following()` calls from
  `apply_operation` and `apply_cursor_operation`; call it once at the top of
  `apply_operation` unless the operation is `Scroll`, `ToggleExpansionTarget`,
  or a filter op. `Right` thereby gains re-anchor (the one intended behavior
  change).

## Files to modify

- `crates/mbv-components/src/list/viewport.rs` — rule text on `ViewportAnchor`.
- `crates/mbv-components/src/media_list.rs` — `re_anchor_selection_following`
  becomes the default rule with the pointer-`Context` exclusion.
- `crates/mbv-components/src/list/tree_browser/operations.rs` — single
  dispatch-level re-anchor; delete the per-arm calls and now-stale comments.
- `crates/mbv-components/src/list/tree_browser/tests/selection.rs` (or a
  sibling test file) — one test for the `Right` contract change.

## Reuse

- `ViewportAnchor` / `Viewported` (`crates/mbv-components/src/list/viewport.rs`)
- Existing D2 re-anchor tests: `crates/mbv-components/src/media_list/tests.rs`
  (`a_key_after_a_free_scroll_bring_the_selection_back_into_view` and the wheel
  test) keep covering the unchanged media-list behavior.

## Steps

- [x] Add the canonical rule text to `ViewportAnchor`'s docs in `viewport.rs`
      (keyboard op or selection-moving/setting op re-anchors; wheel and the
      pointer-context exclusion do not), citing the spec requirement.
- [x] Rewrite `MediaList::re_anchor_selection_following` as the default rule:
      early-return for `Scroll(_)` and `Context(_)`, otherwise re-anchor. Keep
      the D2 pointer-context comment on the exclusion.
- [x] In tree `apply_operation`: hoist one `re_anchor_following()` call,
      skipped for `Scroll`, `ToggleExpansionTarget`, `EditFilter`,
      `ClearFilter`, `ClearMarks`; delete the per-arm calls and their
      duplicated comments (Move, Page, cursor ops, AnchorSelection, Select,
      PointerToggleMark, Activate, Context).
- [x] Add one tree test: after a free scroll, `Right` re-anchors the viewport
      to the selection (the changed contract; name the spec rule).
- [x] Run `cargo nextest run -p mbv-components`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo fmt`.
- [ ] Commit (no push without user confirmation), referencing #927.

## Verification

- `cargo nextest run -p mbv-components` — existing media-list D2 tests pass
  unchanged; the new tree `Right` test passes.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt`
- Manual (user, live): wheel-scroll a tree panel away from the selection, press
  Right — the viewport returns to the selection; wheel-scroll a media list and
  right-click a visible row — the viewport stays free.
