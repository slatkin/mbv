# Proposal

## Why

A plain Emby library's rows (movies, home videos, generic libraries, and the feed-group mode) carry no play state in the GUI Client, so they show no played or unplayed mark, no dimmed played look, and no resume state — even though Emby tracks `Played` and `PlaybackPositionTicks` on those items, and even though the TUI paints that same state on the same rows today. `gui-played-row-marks` left these rows out of scope because `GuiEmbyRowFrame` had no state field; the frame was the gap, not the rule.

## What Changes

- The plain Emby-library projection (`GuiEmbyRow`) and its frame (`GuiEmbyRowFrame`) carry one play state per row, derived with `MediaSemanticState::from_emby` — the one derivation the TUI's rows already use (`row_for` in `crates/mbv-components/src/emby_library_content.rs`), so the two Clients cannot drift.
- The Emby lettered row passes that state to the flat row's `play` prop. With it, the existing `gui-played-row-marks` machinery does the whole look: the played or unplayed mark in the trailing cluster, the dim played title and secondary text, the dimmed thumb, and the title weight. No new painter, no new look, no new token.
- The rule applies to every row of the list, a collection row (folder, boxset, series) included, exactly as the TUI's derivation does. A fully played collection is a played row.
- A partly played row is an unplayed row with its resume state, as everywhere else. The row's percent does not paint in the lettered list today (no trailing duration column), and this change adds none — the played/unplayed mark and the title look are the whole scope.
- Music items stay `Ordinary` through `from_emby`'s own music exception, so a music item in a mixed library shows the unplayed mark, as the TUI's row does.
- The change is one frame field and one prop pass. It adds no Service call, no setting, no new component, and no TUI change.

### In scope

- Plain Emby libraries (movies, homevideos, generic) in plain mode.
- Emby feed-view libraries in feed-group mode (the All list and each folder list).

### Out of scope (unchanged, as `gui-played-row-marks` left them)

- Shows Latest and Upcoming rows, Music, Audiobooks, Playlists, and search rows: their frames still carry no play state.
- The Emby Hero: it has no Workspace and no played or unplayed mark today.
- The TUI: it already paints the same derivation's state.

### Decisions made while planning

- The derivation is `MediaSemanticState::from_emby`, not a new function, so the GUI and the TUI share one body, and `is_played_episode`/`RowPlay` keep their single played test through the existing `GuiHomeRowStateFrame::from(&MediaSemanticState)` conversion.
- No `NowPlaying` row in the Emby list: the list has no playing-row concept today, the Queue owns the playing mark, and the shared conversion already folds `NowPlaying` into `Active`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the requirement "Library lists and Workspace tables paint through shared parts" changes: an Emby library row's item now carries a play state, so the scenario "A row with no play state shows no mark" no longer names the Emby library row, and the requirement gains a scenario for the Emby row's carried state.

## Impact

- `crates/mbv-gui/src/gui_core/gui_library/emby_projection.rs`: `GuiEmbyRow` gains the state; `plain_row` and `feed_row` derive it.
- `crates/mbv-gui/src/bridge/frame/library/emby.rs`: `GuiEmbyRowFrame` gains the state; the frame conversion maps it.
- `crates/mbv-gui/src/window/library/lettered/adapters.rs`: `paint_emby_row` passes `play` to the flat row.
- The projection, frame, and lettered-list tests beside those modules. No core (`mbv-client-core`), ctrl, Service, or dependency change.
