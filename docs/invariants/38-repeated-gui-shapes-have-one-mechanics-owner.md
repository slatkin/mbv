# Invariant 38 — A repeated GUI shape has one mechanics owner

**Scope:** GUI components under `crates/mbv-gui/src/window/components/`, their
row-state inputs, and the window code that maps destination data onto them.

## The invariant

1. Two GUI surfaces that share a semantic shape SHALL share the shape's
   mechanics: chrome, state derivation, cursor or selection look, and input or
   layout-report plumbing. A second implementation of those mechanics is a
   review finding.
2. A consumer supplies data, content, hooks, and handlers. It does not repeat
   style strings, state enums, colour rules, row geometry, or mounted-mark
   handling that belong to the shared shape.
3. Shapes with different semantics or token families stay separate. A flat
   Library row, a Workspace table row, and a Queue row need not be one
   component. They share only the rules that are truly the same.
4. A shared semantic state has one derivation. Different row shapes may lay out
   that state differently, but they do not convert it again or derive its look
   again.
5. Sharing presentation does not move interaction authority. The existing
   owner keeps cursor, focus, open state, and command dispatch. A shared part
   reports its regions or marks; it does not invent a second input path.

## Why it matters

The TUI convention is one owner and one painter for each semantic shape. A
destination maps typed data onto that shape and cannot restate its look. The
GUI has the same rule.

Without it, one rule reaches only the surfaces that were wired by hand. The
GUI review found two concrete copies:

- `LibraryWorkspaceTable` has `WorkspaceRowState`, its own played-title rule,
  and duplicate `row_state` conversions in the Shows and Feeds Heroes, while
  flat and tree rows use `GuiHomeRowStateFrame`.
- `QueueChooserMenu` and `TargetChooserPopover` repeat the same card style,
  row geometry, cursor fill, menu roles, and mounted-mark handling.

The copies pass type checks and painted-structure tests. They drift later,
when one surface receives a fix and the other does not.

## How the code upholds it today

- `LibraryListHeader`, `LibraryLetteredList`, `LibraryVirtualList`,
  `LibraryWorkspaceTable`, `LibraryHeroFrame`, `DialogFrame`, `QueueRowMenu`,
  `ArtImage`, `TypedText`, `Truncate`, and `TreeGuide` each own one shape's
  mechanics.
- `QueueRowMenu` serves the Library row menu, Queue row menu, Playlists menu,
  Settings value menus, and other context menus. Their entries differ; the
  menu shell does not.
- The open `gui-unified-list-rows` change makes `FlatRow` and `TreeRow` the
  canonical Library row painters and removes their caller-supplied look props.
- The open `gui-row-hover-actions` change makes `RowHoverActions` the one
  hover-controls painter for every row shape that shows those controls.
- The open `gui-list-header-buttons` change makes `HeaderIconButton` the one
  painter for the Library header's refresh and tree controls.

## Where it currently fails

- Workspace rows still derive play state and the played-title look separately
  from flat and tree rows. Issue #1058 tracks the shared derivation. The
  Workspace table keeps its own layout; only the common state and look move.
- The Queue chooser and playback-target chooser still repeat their card and
  row mechanics. Issue #1059 tracks one `ChooserMenu` and `ChooserRow`.
- A future universal button or menu abstraction is not required. Extract a
  part when the mechanics are the same. Keep different sizes, token families,
  and interaction owners separate.
