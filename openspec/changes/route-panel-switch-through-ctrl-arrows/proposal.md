# Proposal

## Why

The archived `add-grouped-music-tree-browser` change originally kept the
existing per-layout routing where `panel_left` claims plain `Left` ahead of
any focused leaf in the Both panel layout, so the Grouped Music tree's own
Left/Right parent/child navigation would only reach the tree in the
Library-only layout. During implementation that bespoke per-layout carve-out
was dropped in favor of a simpler global rule: the panel-focus switch moved
to `Ctrl+Left`/`Ctrl+Right`, freeing plain `Left`/`Right` for the focused leaf
in every panel mode, for every destination — not just Music. No spec records
this: `panel_left`/`panel_right`'s default chords are not named anywhere
under `openspec/specs/`, so the decision and its rationale (avoid one more
per-destination special case in the router) exist only in code and commit
history. This documents it before the branch merges.

## What Changes

- Record that the router-owned `panel_left`/`panel_right` actions default to
  `Ctrl+Left`/`Ctrl+Right` (not plain `Left`/`Right`), so a focused leaf's own
  plain-arrow chords are always reachable, in every panel mode.
- No code changes. This is documentation of a decision already shipped on
  `feat/add-grouped-music-tree-browser` (commit `171dc088`, "Route panel
  switching through Ctrl arrows").

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `interactive-component-framework`: adds a requirement that the Keyboard
  Router's panel-focus-switch chords are `Ctrl+Left`/`Ctrl+Right`, not plain
  `Left`/`Right`, so no focused leaf needs a per-destination carve-out to
  reach its own plain-arrow chords.

## Impact

- Documentation only (`openspec/specs/interactive-component-framework/spec.md`).
- No source changes: `crates/mbv-core/src/keybinds.rs` (the `panel_left` /
  `panel_right` `KeybindAction` defaults), `src/app/tests_routing_matrix_focus.rs`
  (`panel_focus_switches_on_the_ctrl_arrows_only`,
  `library_focus_tree_navigation_chords_stay_leaf_local`) already reflect this
  behavior on the branch.
