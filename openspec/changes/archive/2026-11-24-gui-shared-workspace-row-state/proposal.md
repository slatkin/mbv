# Proposal

## Why

The `gui-unified-list-rows` change gave every Library and Playlists list row one
play-state contract: a `GuiHomeRowStateFrame` consumed by `FlatRow` and
`TreeRow`, with one title-tone and one progress derivation. The Workspace table
still carries a parallel model: `WorkspaceRowState` restates the frame's three
states, each Hero destination restates the frame-to-enum conversion in its own
private `row_state`, the table's `name_color` restates the played-title rule,
and `WorkspaceRow` carries the resume percent as a second field beside the
state. One semantic row state has two derivations, so a later state or colour
rule can reach flat and tree rows without reaching Workspace rows (issue
#1058).

## What Changes

- `WorkspaceRow` carries the play state as the frame the list rows carry:
  `state: Option<GuiHomeRowStateFrame>`. The separate `progress: Option<f32>`
  field is removed; the table derives the resume run from the frame through the
  one bounded progress constructor (`episode_progress`).
- The Workspace table's played-title rule goes: the table paints the title
  tone through the same derivation the row parts use. The playing row keeps its
  heading tone and its play glyph, which no flat or tree row paints.
- `WorkspaceRowState` and its `wire` mapping are deleted. The state hook
  (`data-library-<hook>-state`) stays and paints the same values it paints
  today (`played`, `active`, `ordinary`), derived from the frame.
- The private `row_state` conversions in `library_shows_hero.rs` and
  `library_feeds_hero.rs` are deleted: each destination passes the frame its
  rows already carry. The Music tracklist and the Audiobooks chapters table
  pass `state: None`, as they carry no play state.
- The Workspace table stays its own row shape. It does not paint through
  `FlatRow` or `TreeRow`, and it gains no play-mark slot, no selection fill,
  and no geometry change.
- The table's number cell, title, duration cell, hover controls, hooks, and row
  grid stay unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the requirement "Library lists and Workspace tables paint
  through shared parts" takes the Workspace table into the shared play-state
  contract: a Workspace row SHALL carry the frame the list rows carry, and the
  played-title tone and resume percent SHALL come from the one derivation the
  row parts use.

## Impact

- `crates/mbv-gui/src/window/components/library_workspace_table.rs` and its
  `tests.rs`: `WorkspaceRow.state` changes type, the `progress` field and the
  `name_color` rule go, and the state-hook value derives from the frame.
- `crates/mbv-gui/src/window/components/list_row.rs`: the played-title tone
  moves to one shared derivation that `RowLevel::title_tone` and the table
  both call.
- `crates/mbv-gui/src/window/components/library_shows_hero.rs` and
  `library_feeds_hero.rs` and their `tests.rs`: the `row_state` conversions and
  their imports go; the rows pass the carried frame.
- `crates/mbv-gui/src/window/components/library_tracklist.rs` and
  `library_audiobooks_hero.rs`: the rows drop the two removed fields' `None`
  bindings.
- `crates/mbv-gui/src/window/library/episode.rs` keeps its two functions as the
  one played test and the one bounded progress constructor; the destinations'
  inline `episode_progress` calls go.
- No Client core, bridge frame, projection, dependency, or TUI change.
  `GuiHomeRowStateFrame` itself is unchanged.

## Assumptions

- Dropping `WorkspaceRow.progress` is part of "one play-state contract": the
  percent is derivable from the frame, so carrying it beside the state is the
  duplication this change removes. If a destination ever needs a percent the
  frame cannot express, that is a frame change, not a second field.
- The state hook keeps today's semantics under the new derivation: a partly
  played row with no percent wires `ordinary`, exactly as the two deleted
  `row_state` conversions map it today.
