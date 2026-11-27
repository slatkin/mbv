# Proposal

## Why

In the GUI trees, the child rows of an open parent paint plainly and do not read as part of their
parent. The planning brief `~/mbv-tree-rows-brief.pdf` (2026-11-27, Music used as the worked
example) gives each tree row a role — root, branch, or leaf — and a look that joins an open
parent's children to it: a rail on the open root, one group fill with a muted rail under it,
branch rows told apart by a flat colour swatch, and the leaf rows in one recessed well.

The 2026-11-21 change `gui-tree-indent-guides` deferred this "group framing" on purpose. This
change adds it, and it replaces the indent guides that the group framing makes redundant.

## What Changes

- The change applies to every GUI tree list. The code holds three: the Music list (artist →
  album → track), the Shows All view (series → season → episode), and the Playlists screen
  (playlist → item). The brief also names Movies, YouTube, Podcasts, Audiobooks, and Feeds. In
  the GUI these are flat lists that do not expand, so they do not change (design, "Inventory").
- An open root row (artist, series, playlist) shows a bright rail on its left edge. The rows
  under it sit on one group fill with a muted rail that runs to the group's last row.
- A branch row (album, season) is taller. It shows a flat swatch whose colour follows the row's
  position among its siblings, its title, a secondary line, and a state mark at the trailing
  end. An open branch row shows a green-tinted fill and a brighter title. A branch row loads no
  artwork, as today.
- The leaf rows (track, episode, playlist item) of one parent sit in one recessed well, with a
  separator between rows. In the depth-2 tree (Playlists) the well sits directly under the
  open root, in the same group, with no branch rows and no swatches.
- **BREAKING (look)**: the indent guides go. The rails and the group fill replace them. Three
  `gui-client` requirements about guides are removed.
- New canvas tokens carry every new value: the rails, the group, the open branch, the well,
  its edges and separator, and the five swatch colours. No painter names a hex value.
- No behaviour changes: press, double-click, Enter, the hover controls, Expand all and
  Collapse all, the filter, the refresh, the pointer target, the sort, and every data fetch stay
  as they are.
- The domain names "chip" and "tray" collide with `CONTEXT.md` (*Fact pill* avoids "chip";
  *Tray* is the desktop status icon). The change names them *Branch swatch* and *Leaf well*.

## Decisions to confirm

`design.md` → Open Questions holds thirteen Yes/No questions, the three of brief §9 among
them, each with a default. The tasks follow the defaults. A different answer changes the named
tasks before work starts.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: removes the three indent-guide requirements; adds the group, the leaf well, the
  branch swatch, and the open-branch requirements; updates "Library lists and Workspace tables
  paint through shared parts" from two row parts to three (flat, branch, leaf).

## Impact

- `crates/mbv-gui/src/window/components/list_row/`: new `branch.rs` and `leaf.rs` row parts,
  then removal of `tree.rs`; `flat.rs` paints the root rail; `list_row.rs` level rules.
- `crates/mbv-gui/src/window/components/tree_guide.rs`: replaced by `tree_rail.rs`.
- `crates/mbv-gui/src/window/library/list.rs` and `shows_list.rs`: row models carry the sibling
  position, the well edge, and the open-branch facts; branch and first and last leaf heights
  change.
- `crates/mbv-gui/src/window/components/library_list.rs`, `library_shows_list.rs`,
  `playlists_panel.rs`: the three tree painters move to the new parts.
- `crates/mbv-gui/assets/app.css` and the committed `app.generated.css`: new tokens, one
  retuned token, two retired tokens.
- Docs: `CONTEXT.md` (two terms), `docs/architecture/gui-client.md` (tree rows, Blitz gaps),
  `docs/invariants/30-…` and `38-…` (part names).
- Out of scope: the TUI trees, the Hero and its Workspace tables, the Queue and player panel,
  all flat lists, sorting, fetching, and the open/close logic.
- The in-progress change `emby-favourites` also edits the `gui-client` spec. Its Favourites
  lists are flat, so no requirement overlaps. Whichever change syncs second rebases its delta.
