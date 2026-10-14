# Invariant 23 — A freely scrolled viewport re-anchors only through the origin-carrying operation vocabulary

**Scope:** the shared `ViewportAnchor` seam (`crates/mbv-components/src/list/viewport.rs`)
and its two owners, `MediaList` and the shared `TreeBrowser`.

Umbrella: #927.

## The invariant

1. A freely scrolled viewport returns to following the selection only on the
   next keyboard operation on the list, or the next operation that moves or
   sets the selection: a click, a select, a restore, or a shell re-anchor.
   The wheel scroll releases it.
2. The re-anchor rule is stated once, at each owner's single dispatch site
   (`MediaList::delegate_operation_with_viewport_len`,
   `TreeBrowser::apply_operation`). No other arm restates it.
3. When one operation serves two input origins with different re-anchor
   behavior, the vocabulary carries the origin: the keyboard and pointer
   forms are different variants, never one shared variant with a
   per-origin exclusion. The rule then matches on the variant, not on the
   caller.
4. The media-list default re-anchors: a future variant re-anchors silently
   unless it is added to the exclusion. Every exclusion is load-bearing and
   owns one contract test.

## Why it matters

A doc paragraph on `ViewportAnchor` restated the rule and named only the
wheel scroll and the pointer context click as exclusions, while the tree
excluded five operations. The prose masked a real gap: keyboard and pointer
both routed tree expansion through `TreeOperation::ToggleExpansionTarget`,
and that one shared operation was excluded from re-anchoring — so the
tree's keyboard expansion keys never re-anchored, against the spec
requirement. One shared variant keyed on operation identity cannot express
an origin-dependent rule. The second copy of the rule went stale while the
code stayed wrong.

## How the code maintains it today

- The vocabulary separates origin. `TreeOperation::ToggleExpansion` is the
  keyboard form: it acts on the selected node and re-anchors.
  `TreeOperation::ToggleExpansionTarget` is the pointer form: it acts on a
  resolved target and keeps the free scroll. Pointer paths select the target
  first (`Select` re-anchors), so the pointer exclusion protects the
  no-select pointer gesture shape rather than any gesture used today.
- Each owner applies the rule at one dispatch site. The media list returns
  early for `Scroll` and pointer `Context`, and re-anchors for all else.
  The tree re-anchors for all but `Scroll`, the pointer expansion toggle,
  and the filter operations (`EditFilter`, `ClearFilter`, `ClearMarks`).
- The deliberate media-list difference stays: the tree's pointer
  right-click rides `Select` and re-anchors, while the media list's pointer
  `Context` does not select and keeps the free scroll.
- Code carries no normative copy of the rule. `ViewportAnchor` and
  `MediaList::re_anchor_selection_following` point at the spec requirement
  and this file. The spec (`shared-list-components`, re-anchor requirement)
  is the single normative text.
- Two contract tests guard the load-bearing exclusions and the fixed arm: a
  pointer `Context` after a free scroll keeps the media-list offset, and a
  keyboard `ToggleExpansion` after a free scroll re-anchors the tree
  viewport to the selection. The sibling `Right` test owns `Right`'s arm;
  each fails for a different production change.

## What remains unenforced

No type forces a new operation variant to declare its origin or its
re-anchor behavior. A new shared-identity variant reused across origins
recreates the masked gap, and a new owner that restates the rule per arm
recreates the second copy. The media-list default-on rule cuts the other
way: a future variant re-anchors silently, with no compiler prompt to ask
whether it should. Review of the vocabulary diff and the per-exclusion
contract tests is the only guard.
