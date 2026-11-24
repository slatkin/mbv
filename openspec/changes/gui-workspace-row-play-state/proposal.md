# Proposal

## Why

Issue #1058. `gui-unified-list-rows` gave flat and tree rows one play-state type
(`GuiHomeRowStateFrame`) and one title-tone rule. `LibraryWorkspaceTable` still
carries a second play-state model, so one semantic state has two derivations.
A later state or colour rule can reach flat and tree rows and miss Workspace
rows.

The copies today:

- `WorkspaceRowState` (`library_workspace_table.rs`) repeats `GuiHomeRowStateFrame`.
- `name_color` (same file) restates the played-title rule.
- `row_state` in `library_shows_hero.rs` and `library_feeds_hero.rs` converts one
  type into the other, twice.
- Both heroes also derive the progress percent a second time for the row.

## What Changes

- A Workspace row carries `GuiHomeRowStateFrame`, the type `FlatRow` and
  `TreeRow` use. The type keeps its name.
- The table's title tone for a row with a state comes from the shared
  title-tone rule in `list_row.rs`. The table keeps its playing-row heading tone.
- The table's progress run reads the percent from the row's state through
  `episode_progress`. `WorkspaceRow.progress` goes.
- Removed: `WorkspaceRowState`, both per-Hero `row_state` functions, and the
  table's private played-title rule.
- The Shows and Feeds Workspace callers and their tests follow.
- Kept: the table's own row shape, number, title, duration, playing glyph,
  hooks, hover controls, and geometry. No play-mark slot and no selection fill.

No user-visible change: this is a refactor, so the change has no spec delta
(`skip_specs: true`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. Behaviour is unchanged.

## Impact

- `crates/mbv-gui/src/window/components/`: `library_workspace_table.rs` (+ tests),
  `list_row.rs`, `library_shows_hero.rs`, `library_feeds_hero.rs`,
  `library_audiobooks_hero.rs`, `library_tracklist.rs`.
- No frame, projection, Service, or TUI change.
- Related: #1005 (one provider rule for row state), #995 (one bounded progress constructor).
