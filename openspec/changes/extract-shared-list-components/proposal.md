# Proposal

## Why

mbv has two independently grown list implementations — the canonical flat media
list (`src/app/components/media_list/`, ~1,800 lines, nine destination
consumers) and the Grouped Music tree adapter over `tui-treelistview`
(`src/app/components/music_tree*.rs`, ~2,100 lines, one consumer). They
duplicate seven concerns outright: stable-target selection, viewport and
keep-visible, retained paint geometry, hit resolution, ordered multi-selection,
zebra/selected-bar/marquee presentation, and identity-preserving reconciliation.
The duplication is already drifting — paint invalidation alone is spelled two
ways (`WideMediaList::invalidate_paint` versus the tree's `paint_complete` flag
cleared at three sites).

Migrating TV Shows to a tree is the trigger, but writing a second hand-rolled
tree adapter beside music's would make three implementations of the same
arithmetic. Extracting the seam first is what makes TV — and any later tree
destination — an adoption rather than another copy.

## What Changes

- Introduce a shared list seam: composable traits over a single row-flow
  abstraction, with default implementations carrying the cursor, viewport, and
  retained-geometry arithmetic that both sides currently hand-roll.
- Express the existing flat canonical media list as the seam's flat
  implementation, with no externally observable behavior change.
- Express the Grouped Music tree browser as the seam's tree implementation,
  with no externally observable behavior change — except one intentional,
  user-approved deviation: the tree's PageUp/PageDown paging changes from its
  legacy fixed 5-row stride to the seam's named visible-viewport policy.
- **BREAKING** (internal API only): `MusicTreeBrowser`'s public surface stops
  exposing `tui-treelistview` arena node ids (`usize`) and speaks stable
  `Target` values instead. `MusicNodeKey` and the arena become private
  implementation details. No user-visible behavior changes; the Grouped Music
  tree test suites that assert against arena indices are rewritten against the
  target surface.
- Delete the now-duplicated cursor, viewport, and retained-geometry mechanics
  from both implementations. Production line-count change is recorded as a
  diagnostic: a non-negative result triggers review for a parallel abstraction,
  but no numeric reduction threshold gates an otherwise-correct implementation.
- Filtering is explicitly out of scope. Flat destinations keep their embedded
  `InlineSearch` control and the tree keeps its internal filter session,
  unchanged and un-accommodated.

Non-goals: migrating TV Shows (a separate change that adopts this seam);
unifying filtering; converting any other destination to a tree; adding any
capability neither existing implementation has today.

## Capabilities

### New Capabilities

- `shared-list-components`: The reusable list seam. Defines the row-flow
  abstraction both list shapes satisfy, the composable concerns layered over it
  (cursor, viewport, retained paint geometry and hit resolution, ordered
  marks, expansion), the rule that public list surfaces address rows by stable
  target rather than internal index, and the requirement that shared mechanics
  have exactly one implementation site.

### Modified Capabilities

- `canonical-media-lists`: Three requirements describe mechanics the seam now
  owns and are superseded rather than amended — "One shared owner supports
  list-local extension" (which explicitly carves the tree out of its own scope),
  "Responsive handoff preserves an explicit anchor", and "Canonical geometry
  has no compatibility path". Each is REMOVED with its guarantees migrated to
  the corresponding `shared-list-components` requirement, which states them once
  for every list shape. "WideMediaList owns fixed-row mechanics" is MODIFIED to
  keep its painting contract (selected-row bar, scrollbar, row placement) while
  sourcing row flow, cursor, viewport, retained geometry, and point resolution
  from the seam, and to drop its tree carve-outs.

  The requirements that describe list *content* rather than list mechanics —
  the row model, zebra striping, marquee, Inline Search composition, destination
  composition, stable targets — are untouched.

## Impact

Affected code:

- `src/app/components/media_list/` — `mod.rs`, `wide.rs`, `carrier.rs`,
  `anchor.rs`, `selection.rs`. `mod.rs` currently exceeds the 800-line cap and
  must be split mechanically before behavior changes.
- `src/app/components/music_tree.rs`, `music_tree_model.rs`, `music_tree_view.rs`,
  `music_tree_selection.rs`, `music_tree_label.rs` — the arena id goes private
  and the shared arithmetic is deleted in favour of the seam. These files are
  already below the cap and are split only if implementation growth would push
  one over it.
- `src/app/components/music_content*.rs` — call sites that currently pass arena
  ids move to stable targets.
- New module for the seam itself.

Tests: `media_list/tests.rs`, `music_tree_tests.rs`, `music_tree_browser_tests.rs`,
`music_content_tree_tests.rs`, `render/tests_music_characterization.rs`,
`render/tests_music_groups.rs`, and the music tick-integration suites. Tree tests
bound to arena indices are rewritten against the target surface rather than
translated case by case.

Dependencies: none added. `tui-treelistview` stays pinned at 0.2.2 and stays
behind the tree implementation's boundary — the seam does not leak crate types.

Risk: this is a foundation change with no user-visible payoff until TV adopts
it, which makes "done" easy to fake. Removing the duplicated mechanics is the
acceptance criterion. Production line count is recorded only as a diagnostic:
an unexpected non-negative result must be investigated and explained, but is
not by itself a failure.
