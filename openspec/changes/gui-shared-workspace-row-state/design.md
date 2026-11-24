# Design

## Context

See proposal.md (Why). `gui-unified-list-rows` left the Workspace table with a
parallel play-state model in four files:

- `list_row.rs`: `RowLevel::title_tone` derives the title tone from
  `Option<&GuiHomeRowStateFrame>` — muted when played, else text — and
  `window/library/episode.rs` owns `is_played_episode` (the played test) and
  `episode_progress` (the bounded percent).
- `library_workspace_table.rs`: `WorkspaceRowState` restates the frame's three
  states with a `wire` mapping, `WorkspaceRow` carries `state` and a separate
  `progress: Option<f32>`, and `name_color` restates the played-title rule with
  a playing-row heading override.
- `library_shows_hero.rs` (line 137) and `library_feeds_hero.rs` (line 54):
  identical private `row_state` conversions frame → `WorkspaceRowState`, plus an
  inline `episode_progress(&state).map(f32::from)`.
- `library_tracklist.rs` and `library_audiobooks_hero.rs` pass
  `state: None, progress: None`.

`GuiHomeRowStateFrame` is `Clone + PartialEq`, not `Copy`, so every row
construction clones the frame it carries.

## Goals / Non-Goals

**Goals:**

- One play-state contract: the Workspace rows carry the frame the list rows
  carry, and no destination converts it.
- One title-tone and one percent derivation, reached by every row part and the
  Workspace table.
- The table's own marks — the playing glyph, the playing heading tone, the
  hover swap, the row grid — stay table-owned and unchanged.

**Non-Goals:**

- One universal row component. The table stays its own shape and does not
  paint through `FlatRow` or `TreeRow`.
- A play-mark slot or selection fill for the table (out of scope per issue
  #1058).
- Any bridge frame, projection, or Client core change. No state or colour rule
  changes for the user; every observable hook and mark keeps today's value.

## Decisions

### D1. One played-title derivation in `list_row.rs`

The carried-state tone rule moves out of `RowLevel::title_tone` into a
`pub fn played_title_tone(play: Option<&GuiHomeRowStateFrame>) -> String` in
`list_row.rs`: the muted tone when `is_played_episode(state)`, the text tone
otherwise, the text tone when `play` is `None`. `RowLevel::title_tone` delegates
to it for the `Some` case and keeps its level default (leaf muted) only for the
`None` case, so a level-tone rule stays per-level and a state-tone rule reaches
every row.

The table calls `played_title_tone(row.state.as_ref())`. Alternative
considered: the table calls `RowLevel::Top.title_tone`. Rejected — it drags
level semantics into a table that has no levels, and a later leaf-tone change
would reach Workspace rows by accident rather than by decision.

### D2. `WorkspaceRow` carries the frame; the percent goes

`WorkspaceRow.state` becomes `Option<GuiHomeRowStateFrame>`; the
`progress: Option<f32>` field is deleted and `paint_row` derives the resume run
from the frame through `episode_progress` (the one bounded percent constructor,
issue #995), widening `u8` to the run's percent as the heroes do today.
`WorkspaceRowState`, its `wire`, and `name_color` are deleted. The table keeps a
`pub(crate)`-invisible (module-private) `title_tone(row)` that paints the
heading tone for the row playing at the playhead and otherwise delegates to
`played_title_tone` — the playhead mark is table-owned, exactly as its glyph is.

Construction sites clone the frame they carry: the Shows hero passes
`episode.state.clone()`, the Feeds hero `row.state.clone()`; the Music
tracklist and the Audiobooks hero pass `state: None`. Keeping a separate
percent field was considered and rejected: the percent is derivable from the
frame, so a second field is the duplication this change removes, and a percent
the frame cannot express would be a frame change, not a table field.

### D3. The state hook keeps today's values, derived from the frame

The table keeps painting `data-library-<hook>-state` and keeps a module-private
wire function over the frame: `Played` wires `played`, `Active` with a percent
wires `active`, and everything else — including `Active` with no percent, the
unknown-runtime case — wires `ordinary`, exactly as the two deleted `row_state`
conversions map it today. This is hook vocabulary, not a look rule; it stays
with the hook it feeds. Alternative considered: wiring `active` whenever the
frame is `Active`. Rejected — it changes the observable hook value for
unknown-runtime rows with no need.

### D4. The destinations stop converting

`library_shows_hero.rs` and `library_feeds_hero.rs` delete their private
`row_state` functions and their `episode_progress`/`is_played_episode` imports
(`episode_progress` moves to the table's imports), and pass the carried frame.
`library_tracklist.rs` and `library_audiobooks_hero.rs` drop the removed
bindings. No destination computes a tone, a percent, or a state class.

### D5. Tests

- `library_workspace_table/tests.rs` owns the new contract: one test builds rows
  over `Played`, `Active { progress: Some }`, `Active { progress: None }`, and
  `Ordinary` frames and asserts the title tone and the resume run each derives
  (the shared derivation the spec's new scenarios name). Existing fixtures drop
  the deleted `progress` binding.
- `list_row/tests.rs` keeps its `title_tone` cases unchanged: the delegation
  preserves behaviour, so no new case. One `#[case]` row is added only if the
  moved `played_title_tone` needs a direct owner test the `RowLevel` table does
  not already give it — it does not.
- `library_shows_hero/tests.rs` and `library_feeds_hero/tests.rs` change only
  where the deleted conversion and imports were: their rows already build
  `GuiHomeRowStateFrame` values, and their hook and mark assertions hold.
- No test count goal; each named contract above is one test.

### Dependencies

None. No new or upgraded dependency: the change reuses the frame type and the
helpers already in the crate.

## Risks / Trade-offs

- [The state hook value changes for unknown-runtime rows] → It does not: D3
  keeps the `Active { progress: None }` → `ordinary` mapping the deleted
  conversions had.
- [`GuiHomeRowStateFrame` is not `Copy`, so row construction clones it] → Same
  clone the heroes' conversion paths paid today; the frame is a three-variant
  enum with one `Option<u8>`.
- [A future level-tone rule must reach the table by decision, not by default] →
  Intended: D1 splits the state rule (shared) from the level rule (per-part).
  The design records this as the seam.

## Migration Plan

None. Rollback is a revert; no data, protocol, or persisted state changes.
