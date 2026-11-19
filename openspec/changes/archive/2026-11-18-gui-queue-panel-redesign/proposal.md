# Proposal

## Why

The user supplied a redesign of the GUI Queue column (`assets/queue-panel-redesign.pdf`, from `~/mbv queue panel redesign.pdf`). It sets one Everforest Dark Hard token set for the column, puts the transport on its own row under a tick-free seekbar, moves the queue operations beside the queue switcher, and gives the queue rows one fixed height with the progress bar inside the row. This change makes the GUI Client match that design. The design is the authority: where it differs from today's GUI or today's specs, the design wins.

## What Changes

- **Tokens.** The Queue column paints from the design's Everforest Dark Hard tokens (`bg_dim`, `bg0`–`bg5`, `bg_green`, `bg_red`, `fg`, `grey0`–`grey2`, `green`, `aqua`, `orange`, `red`) and the player backdrop `#15191b`, with the roles the design gives each one.
- **Panel layering.** From top to bottom: the player (the dark backdrop, with a scrim under the transport), the title block on `bg0`, a 1 px rule, the 48 px queue header on `bg1`, the recessed queue list on `bg_dim`, and the footer on `bg0`. The series or parent line paints in the destructive red hue, as the design shows.
- **Player controls.** The bottom overlay holds two rows. Row 1 is the seekbar alone, across the full width. Row 2 holds play/pause, previous, back 10 s, forward 30 s, and next at the left, and the elapsed and remaining clocks at the right. The transport no longer sits in the seekbar's clock line. The one-third height limit and the scale-down from `gui-art-player-overlay` stay.
- **Seekbar.** The bar shows the track, the played fill, and the thumb. It shows no chapter ticks and no intro or credits marks. Over the bar, the track grows, the thumb grows, and a time tip shows the time at the pointer. The hit area is the whole seekbar row. A drag on the bar keeps the hover state, and the time tip follows the pointer; the drag scrubs as it does today.
  - **Removed:** the chapter ticks, the intro and credits marks, the trickplay frame, and the chapter number in the hover preview.
  - **Removed after implementation (2026-11-19):** the buffered range. It was implemented end to end, broke playback, and was reverted whole (`9567cff30`); see design.md's outcome notes.
- **Queue header.** From left to right: the `QUEUE` label, the switcher, the new-queue control, the close control, then the filter control at the far right. The switcher shows the shown queue's name, its slot count, and a chevron. The switcher shows at all times, also when only one entry exists. The close control keeps its disabled state on the only local queue.
- **Switcher menu.** The menu anchors under the switcher. Each entry shows its name and its slot count. The shown entry carries the check. The playing queue carries the waveform mark. The current mark of a stopped or paused current queue and the peer mark stay.
- **Queue rows.** Every row has one fixed height. The row shows the 2-digit number, the now-playing glyph on the now-playing row, and the title. The progress bar of an in-progress or now-playing row paints inside the row at its bottom and never adds height. The now-playing row has its own fill. A watched row paints muted with a strike. The rows paint no zebra stripe and no left accent stripe.
- The footer keeps its content. Only its tokens change.
- The TUI Client does not change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the player controls' row arrangement (the transport on its own row under the seekbar), and the scrubber (no chapter boundaries and no intro or credits marks, a time-only hover tip, and drag to seek).
- `gui-queue-chooser`: the queue header's controls and their order, the switcher that always shows, and the slot count on the switcher and on each menu entry.
- ~~`ctrl-protocol`: the Player status carries the buffered position as an additive, negotiated field.~~ Withdrawn: the buffered position was implemented and then reverted whole (`9567cff30`); the wire is unchanged and no `ctrl-protocol` delta remains in this change.

## Impact

- `crates/mbv-gui/assets/app.css` (and `app.generated.css`): the Everforest token set, the panel layering, the row and header paint.
- `crates/mbv-gui/src/window/components/queue_playback_panel.rs`, `queue_playback_panel/art_stage.rs`, `queue_playback_panel/scrubber.rs`: the two-row bottom layer, the seekbar, the time tip, the title block.
- `crates/mbv-gui/src/window/components/queue_header.rs`, `queue_chooser_menu.rs`: header order, the `QUEUE` label, the switcher with its count, the menu counts and marks.
- `crates/mbv-gui/src/window/components/queue_panel.rs`, `queue_panel/row_cells.rs`: fixed row height, in-row progress bar, now-playing fill, no zebra.
- `crates/mbv-gui/src/bridge/frame/{playback,queue,overlay}.rs`, `crates/mbv-gui/src/sections.rs`, `crates/mbv-gui/src/core_thread/tick_host.rs`: the slot counts and the trimmed hover preview.
- `src/app/gui_client_state/projection.rs`, `src/app/gui_client_state/queue_chooser.rs`: the entry slot count.
- ~~`crates/mbv-ctrl` (`player.rs`, `protocol.rs`, `hello.rs`), `crates/mbv-daemon`, `crates/mbv-player`, `crates/mbv-ui-model/src/playback.rs`, `protocol/ctrl/v10/`: the buffered position, per `docs/recipes.md` "Add a ctrl protocol variant".~~ Withdrawn with the buffered range (`9567cff30`); the wire is unchanged.
- The chapter, media-segment, and trickplay fetches (`src/app/playback_metadata.rs`, `src/app/playback_trickplay.rs`, `crates/mbv-emby/src/client_item_metadata.rs`, and their model types and caches) feed only the removed scrubber paint. They become dead code and are deleted.
- No new dependencies.
