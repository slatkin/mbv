# Design

## Context

See proposal.md for the why and the scope.

The played/unplayed look and its mark already exist end to end, built by `gui-played-row-marks`:

- `crates/mbv-gui/src/window/components/play_mark.rs` holds `RowPlay` (the one derivation over `GuiHomeRowStateFrame`), the look methods (`title_tone`, `title_weight`, `secondary_tone`, `trail_tone`, `thumb_opacity`), and the `PlayMark` component.
- The flat row (`crates/mbv-gui/src/window/components/list_row/flat.rs`) takes `play: Option<RowPlay>` and paints the whole treatment. A `play: None` row keeps its present markup.
- `GuiHomeRowStateFrame::from(&MediaSemanticState)` (`crates/mbv-gui/src/bridge/frame/library/home.rs`) already converts the shared provider-neutral state into the frame vocabulary, folding `NowPlaying` into `Active`.
- `MediaSemanticState::from_emby` (`crates/mbv-ui-model/src/media_list.rs`) is the one item-level derivation for an Emby row. The TUI's rows already use it (`row_for` in `crates/mbv-components/src/emby_library_content.rs` passes it as `semantic_state` on every row, collections included). `EmbyItem` carries everything it needs: `played`, `playback_position_ticks`, `runtime_ticks`.

The Emby lettered row already paints through `FlatRow` (`paint_emby_row` in `crates/mbv-gui/src/window/library/lettered/adapters.rs`); it simply passes no `play` today. The gap is exactly one carried field through three layers: projection row → frame row → flat-row prop.

Current facts that shape the approach:

- `GuiEmbyRow` is built in exactly two places: `plain_row` and `feed_row` in `crates/mbv-gui/src/gui_core/gui_library/emby_projection.rs`.
- `GuiEmbyRowFrame::from(&GuiEmbyRow)` in `crates/mbv-gui/src/bridge/frame/library/emby.rs` is the single frame conversion.
- `RowPlay::of_row` maps `Ordinary` to `Unplayed`, so every row of the list shows a mark — the dot or the check — as the spec requirement "Rows mark their played state" states. This matches the Home and Podcasts rows, not an opt-in.
- The lettered Emby list has no duration column; the trailing text is the feed-group gutter date or nothing. `PlayMark` sits in the trailing cluster per `gui-played-row-marks` design D3, so a row with no trailing text shows the mark at its right edge. No percent or progress run paints in this list, and none is added.

## Goals / Non-Goals

**Goals:**

- The Emby library row carries the same play state the TUI's row carries, derived by the same function.
- The whole look comes from the existing `play` prop path, so no destination converts the state or restates the look.

**Non-Goals:**

- No change to the Emby Hero, to row heights, to the hover controls, or to the letter rail.
- No progress percent in the lettered list.
- No change to any other destination's rows.
- No core, ctrl, Service, or TUI change; no new dependency.

## Decisions

### D1. `GuiEmbyRow` carries `state: MediaSemanticState`, derived in the two row builders

`plain_row` and `feed_row` set `state: MediaSemanticState::from_emby(item)`. There is no third construction site, so the derivation cannot be skipped in one mode. The projection reads `App` data only, as now; the derivation is a pure function of the item.

A collection row (folder, boxset, series) takes the same derivation, as the TUI's row does: Emby sets `played` on a fully watched container, and such a row is a played row. A container with a resumable position is an edge case Emby barely produces, and the shared body handles it the same way the TUI does. Music items are `Ordinary` through `from_emby`'s music exception.

### D2. `GuiEmbyRowFrame` carries `state: GuiHomeRowStateFrame` through the existing conversion

`GuiEmbyRowFrame::from` sets `state: GuiHomeRowStateFrame::from(&row.state)`. Reusing `GuiHomeRowStateFrame` — rather than adding a second frame state type — keeps `RowPlay::of_row` the single derivation the flat row reads, as the requirement "Library lists and Workspace tables paint through shared parts" states ("no destination SHALL convert the play state into a second state of its own"). The conversion's `NowPlaying`-into-`Active` fold never fires here, because `from_emby` never yields `NowPlaying`.

### D3. `paint_emby_row` passes `play: Some(RowPlay::of_row(&item.row.state))`

The one-line prop pass is the whole painter change. The flat row then paints the mark cluster, the title tone and weight, the subtitle tone, the trail tone, and the thumb opacity exactly as the Home and Podcasts rows do. The letter heading branch is unchanged.

`RowPlay::of_row` never returns `None` — a carried state always maps to `Unplayed` or `Played` — so every item row of the Emby list keeps the same-size mark slot, and no row moves when its state changes.

### D4. State freshness follows the list's existing refresh rules

The state is a fact of the fetched item, exactly as in the TUI. A play or a Mark watched does not repaint the row until the list reloads through its existing paths (the destination's Refresh button, the reload after a Service event). No invalidation machinery is added: the TUI has lived with the same rule, and a reactive played-state push is a separate change if it is ever wanted.

### D5. Tests

The tests follow `crates/mbv-gui/AGENTS.md` and the test-ownership invariant: each test owns one contract, no inflation.

- The projection test beside `emby_projection.rs` (or the existing projection test module) gains one case: a played item and a partly played item project `Played` and `Active`, and a music item projects `Ordinary`. This is the derivation contract — the seam where the state enters the GUI.
- `library_lettered_list/tests/emby.rs` gains one case: a played row paints `data-play-mark="played"` and the played title weight, and an ordinary row paints `data-play-mark="unplayed"`. This is the prop-pass contract — the seam where the frame reaches the flat row. The mark's own rendering and the flat row's look are already covered by `gui-played-row-marks`'s tests and are not re-covered.

## Risks / Trade-offs

- [A fully played series or folder shows the played check, which can surprise on a container] → It is the TUI's behavior on the same rows today, so the change is parity, not invention. If the user dislikes it live, restricting the state to `GuiEmbyRowKind::Media` rows is a one-line gate in the two builders.
- [The state goes stale in a long-lived list] → Accepted in D4, as in the TUI.
- [A row's unplayed-count decoration (`Name [3]`) now sits beside an unplayed dot, two facts about the same state] → They answer different questions (how much is unwatched inside vs. whether the container itself is done); the TUI already paints both. No change.

## Migration Plan

A look-and-frame change in one crate, with no data or protocol change. The frame is rebuilt on every projection, so no migration exists. To roll back, revert the commits.
