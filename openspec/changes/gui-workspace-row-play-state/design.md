# Design

## Context

`GuiHomeRowStateFrame` (`Ordinary`, `Played`, `Active { progress: Option<u8> }`)
is the play-state type of `FlatRow` and `TreeRow`. `window/library/episode.rs`
holds `is_played_episode` and `episode_progress`. `RowLevel::title_tone` in
`list_row.rs` holds the played-title rule for rows that carry a state
(`gui-unified-list-rows` D4). The Workspace table has its own enum, its own
title rule, and an `f32` progress field that both heroes fill from
`episode_progress`.

A Workspace row differs from a list row on one point: a row with no state
(Audiobooks chapters, the Music tracklist) paints the text tone. A list row
with no state paints its level's tone (muted on a leaf). So the shared rule is
the rule for a row *with* a state; the no-state case stays with each row shape.

## Goals / Non-Goals

**Goals:** one play-state type and one title-tone and progress derivation for
flat, tree, and Workspace rows.

**Non-Goals:** one row component for all shapes; a play-mark slot or selection
fill in the table; any change to hooks, layout, or frames.

## Decisions

### D1. `WorkspaceRow.state` becomes `Option<GuiHomeRowStateFrame>`

The field keeps its name. `WorkspaceRowState` is deleted. Both heroes pass
`Some(episode.state.clone())`; Audiobooks and the tracklist keep `None`.
Alternative: a rename of the type. Rejected: churn only.

### D2. `WorkspaceRow.progress` is removed

The percent is `episode_progress(state)`, the same call `FlatRow` makes. The
table paints the run from it. This removes the second derivation in each hero.
Alternative: keep the field. Rejected: it lets state and percent disagree.

### D3. One title-tone function for a carried state

`list_row.rs` gains `played_title_tone(&GuiHomeRowStateFrame) -> String`
(muted when `is_played_episode`, else text). `RowLevel::title_tone` calls it
for `Some(state)`. The table calls it for a non-playing row with a state.
The table paints `--canvas-heading` for the playing row and the text tone for a
row with no state, as now. The table keeps its CSS-variable strings through
`canvas_var`, as the list rows do.
Alternative: have the table call `RowLevel::title_tone`. Rejected: `None` maps
to the level's tone, which would mute no-state rows on a leaf level.

### D4. The state hook keeps its three values

The table paints `data-library-<hook>-state` as `played`, `active`, or
`ordinary`. A private function in the table maps the type to these values with
`is_played_episode` and `episode_progress`, so `Active { progress: None }`
paints `ordinary` as before. The hook set stays unchanged.

## Risks / Trade-offs

- [A later hook or tone rule needs the table to diverge] → The table keeps its
  row shape; only the state type and derivations are shared.
- [`Active { progress: None }` maps to `ordinary`] → This keeps today's hook
  value. The mapping is one function with a test.

## Test ownership

The table owns the contract "a carried state paints the hook, the title tone,
and the progress run by the shared rules". One `#[case]` table in
`library_workspace_table/tests.rs` covers it (`None`, `Ordinary`, `Played`,
`Active` with a percent, `Active` without). The Shows and Feeds hero tests keep
their played-row cases and only change their fixtures.
