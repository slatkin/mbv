# Design

## Context

See `proposal.md` for the motivation and `specs/` for the behavior. The visual reference is `assets/queue-panel-redesign.pdf` (two pages: the full column mock, then the token table and the annotated parts 02–06). Its numbers are the paint numbers this design names.

Current state that shapes the approach (main at `6920d211b`, after `gui-art-player-overlay` merged):

- **Player.** `QueuePlaybackPanel` (`crates/mbv-gui/src/window/components/queue_playback_panel.rs`) paints `ArtStage` (`queue_playback_panel/art_stage.rs`): a stage with the clipped media box, the top layer (`ChipsAndBadge`), and the bottom layer with one `ControlRow`. `ControlRow` paints a 56 px play/pause button at the left of `Scrubber`, and the transport sits inside the scrubber's clock line through its `center` prop. `BOTTOM_FULL_HEIGHT_PX` is 68 and `player_scale` keeps the layer within one third of the stage. Auto-hide is CSS on `data-auto-hide`/`data-playing`.
- **Scrubber** (`queue_playback_panel/scrubber.rs`): 24 px bar box with a 6 px track, intro and credits blocks, played fill, 2×14 chapter ticks, hover line, 16 px thumb, a hover preview with a trickplay tile and a `Ch. n` label, then a 30 px clock line. The seekbar region is the bar box. A press seeks (`PlaybackLeaf::press`); a left-drag anchored in the bar scrubs with throttled seeks (`admit_scrub`). The core sets the `hover` section on every pointer move over the bar (`core_thread.rs`, `set_scrubber_hover`).
- **Scrubber data.** Chapters, intro and credits marks, and trickplay grids come from `src/app/playback_metadata.rs` (Emby `get_item_chapters`, `get_media_segments`, `get_trickplay`), and trickplay sheets from `src/app/playback_trickplay.rs` (`get_trickplay_tile`), served to the window through the art scheme (`core_thread/tick_host.rs`). Nothing else reads them.
- **Queue header** (`queue_header.rs`): a 64 px band (`--canvas-queue-header-height`); the chooser button only with two or more entries; filter, new, close at the trailing edge.
- **Chooser menu** (`queue_chooser_menu.rs`): check on the shown entry, accent dot on the current queue, `AudioLines` on the playing queue, brand mark on the peer. `GuiQueueChooserEntry` carries no count.
- **Rows** (`queue_panel.rs`, `queue_panel/row_cells.rs`): 30 px rows (`ROW_HEIGHT`, `ROW_HEIGHT_PX`, and literal `30px` in several styles), zebra stripes from `GuiQueueRow::Item::striped`, the progress bar in a flex column under the title.
- **Tokens.** `assets/app.css` defines `--canvas-*` hexes; `app.generated.css` is built by `make gui-css`.
- **Status.** `mbv_ctrl::PlayerStatus` crosses the ctrl wire. `mbv_ui_model::playback::PlaybackState` (`active`, `active_idx`, `position_ticks`, `runtime_ticks`, `paused`) feeds `QueuePlaybackProjection.playback` and `GuiPlayhead`.

## Goals / Non-Goals

**Goals:**
- The Queue column matches the PDF in arrangement and paint.
- One token set for the column, named after the design's Everforest tokens.

**Non-Goals:**
- The TUI Client, the Library column, the collapsed rail, and the compact strip do not change.
- The footer keeps its content and order; only its fill token changes.
- The row hover chrome (grip, actions button), the row menu, drag reorder, the start and playing marks, the filter's in-place text entry, auto-hide, and the right-click player menu keep their behavior.
- No video rendering.

## Decisions

### D1. Token set

`assets/app.css` gains one block of Everforest Dark Hard tokens under the names the PDF uses, prefixed `--ef-`:

| Token | Hex | Role in the column |
| --- | --- | --- |
| `--ef-player` | `#15191b` | player backdrop |
| `--ef-bg-dim` | `#1e2326` | queue list (recessed) |
| `--ef-bg0` | `#272e33` | title block, footer, row hover |
| `--ef-bg1` | `#2e383c` | queue header bar, count chip |
| `--ef-bg2` | `#374145` | switcher rest, menu, row progress track |
| `--ef-bg3` | `#414b50` | rules, control hover, active row, menu item active/hover |
| `--ef-bg4` | `#495156` | control borders |
| `--ef-bg5` | `#4f5b58` | hover border, strike line |
| `--ef-bg-green` | `#3c4841` | now-playing row |
| `--ef-bg-red` | `#4c3743` | close-control hover |
| `--ef-fg` | `#d3c6aa` | primary text |
| `--ef-grey0` | `#7a8478` | open switcher border |
| `--ef-grey1` | `#859289` | row numbers, watched titles |
| `--ef-grey2` | `#9da9a0` | secondary text, glyphs, seek track base |
| `--ef-green` | `#a7c080` | played fill, play button, now-playing text, check |
| `--ef-aqua` | `#83c092` | playing mark in the menu |
| `--ef-orange` | `#e69875` | partial progress fill |
| `--ef-red` | `#e67e80` | parent line, destructive |

`grey0` is not in the PDF's swatch table but its part 02 names it; `#7a8478` is Everforest Dark Hard's `grey0`. The Queue column's components read only these tokens. A `--canvas-*` token that no other surface reads after the change is deleted; a `--canvas-*` token that another surface still reads stays. The Library column is out of scope and keeps its tokens.

*Alternative:* change the `--canvas-*` values in place. Rejected: those names describe old roles (`--canvas-ink`, `--canvas-track-current`) and are shared with the Library column, which this change must not move.

### D2. Panel layering

From top to bottom (PDF part 06): the stage on `--ef-player`; the title block on `--ef-bg0`, padding `16px 20px 18px`, title 18/600 `--ef-fg`, parent line `--ef-red`; a 1 px `--ef-bg3` rule; the queue header, 48 px on `--ef-bg1` with a 1 px `--ef-bg3` bottom border; the list on `--ef-bg-dim`; the footer on `--ef-bg0`. The no-image and stopped arrangements from `gui-art-player-overlay` keep their order and take the same fills.

### D3. Player bottom layer: two rows

`ControlRow` becomes two rows inside one column:

1. **Seek row**: `Scrubber` alone, full width, 14 px tall at scale 1.0. The whole row is the `seekbar` region (`data-target="seekbar"` moves from the inner bar box to the row element).
2. **Transport row**: play/pause (round, `--ef-green` fill, 44 px), previous, −10, +30, next at the left; elapsed and `−remaining` at the right (mono, `--ef-fg`, remaining at reduced opacity as today).

The scrim is 104 px at scale 1.0 (PDF part 04: "scrim 104 tall"), a vertical gradient from transparent to `rgb(0 0 0 / 0.6)`. `BOTTOM_FULL_HEIGHT_PX` becomes `104.0`; `player_scale` and the one-third rule do not change. `Scrubber` loses its `center` prop and its clock line; the clocks move to the transport row. Both rows multiply their sizes by `var(--player-scale)` as today.

### D4. Seekbar paint and hover

At rest: a 4 px track, radius 2, `--ef-grey2` at 28 %; the buffered range at 42 %; the played fill `--ef-green`; a 14 px thumb, `--ef-green` with a 2 px `--ef-bg-dim` ring, as the PDF draws it. While the `hover` section is `Some` (pointer over the row, also during a scrub drag): the track is 6 px, the thumb 16 px, and the time tip paints above the pointer position with `hover.time_label` in mono on `--ef-bg2`. The hover state comes from the core's `hover` section, not from `:hover`, so the time tip and the track grow together and survive the drag.

No chapter ticks, no intro or credits blocks, no hover line, no trickplay tile, no chapter label.

### D5. Dead scrubber data is deleted

With D4, the frame's `chapters`, `intro_mark`, `credits_mark`, `trickplay`, and `GuiScrubberHover::{chapter, chapter_number, tile}` have no reader. They are deleted from the frame (`bridge/frame/playback.rs`, `bridge/frame.rs`, `window/sections.rs`), from `QueuePlaybackProjection`, and from the core:

- `src/app/playback_metadata.rs` and its tests, the `playback_metadata_cache` and its fetch path;
- `src/app/playback_trickplay.rs` and its tests, `trickplay_sheets`, `trickplay_sheets_loading`, `trickplay_sheet_failed`, `drain_trickplay_sheet_events`, and the sheet-bytes answer in `core_thread/tick_host.rs`, with the public re-exports in `src/app.rs` and `src/lib.rs`;
- `crates/mbv-emby/src/client_item_metadata.rs` and its tests in `crates/mbv-emby/tests/emby/client.rs`, and the `EmbyChapter`, `EmbyMediaSegment`, and `EmbyTrickplayGrid` types in `mbv-emby-model` when nothing else reads them.

`GuiScrubberHover` keeps `fraction` and `time_label`.

*Alternative:* keep the fetches for a later inline-video change (#989). Rejected: the repository rule is to delete dead code, and git keeps it.

### D6. Buffered position end to end

- `mbv_ctrl::PlayerStatus` gains `#[serde(default)] pub buffered_ticks: i64` (0 = unknown). Per `docs/recipes.md` "Add a ctrl protocol variant": no version bump; a new `CTRL_CAP_BUFFERED_POSITION = "buffered-position"` with a doc comment, re-exported, advertised by the Owner through `CtrlHello`/`DaemonHelloCaps`, with a `supports_buffered_position` accessor and a handshake case; the wire round-trip test and the `protocol/ctrl/v10/` fixtures and schemas regenerate (`MBV_PROTOCOL_BLESS=1`). The Owner sends the field to every Client: an older Client ignores an unknown field, so no per-connection gate is needed.
- `mbv-player` observes mpv's `demuxer-cache-time` (`Format::Double`, seconds of media time) on the next free observe id and stores `seconds_to_ticks(value)` in `status.buffered_ticks`; it resets to 0 on a new file, as `position_ticks` does.
- A Cast receiver or an Emby Session output leaves the field 0.
- `mbv_ui_model::playback::PlaybackState` gains `buffered_ticks: i64`; every constructor sets it (0 where no local player reports it). `GuiPlayhead` carries it; the window's `Playhead` exposes `buffered_fraction` = `clamp(buffered / runtime)`, painted as is (not predicted). The scrubber paints the buffered range only when the fraction is above the played fraction.

### D7. Queue header

Order (PDF part 03): a `QUEUE` label (11/600, letter-spacing 0.08em, `--ef-grey2`), the switcher, new-queue, close; a flex spacer; the filter control. Bar 48 px, padding `0 12px`, gap 4 px. Icon buttons 32×32, radius 6, glyph 16 at stroke 1.6, `--ef-grey2`, `--ef-fg` with `--ef-bg3` fill on hover; the enabled close control hovers `--ef-bg-red` with a `--ef-red` glyph. The disabled close control keeps its current opacity and reports no region.

The switcher is always the `button` (the plain-title branch goes): list glyph, name (13/600 `--ef-fg`), count chip (`--ef-bg1`, radius 4, 11 px mono), chevron (up while the chooser is open). Trigger 32 px tall, radius 6, padding `0 8px 0 10px`, gap 8; rest `--ef-bg2` + 1 px `--ef-bg4`, hover `--ef-bg3` + 1 px `--ef-bg5`, open `--ef-bg3` + 1 px `--ef-grey0`. The count is `GuiQueueHeaderFacts.shown_len: usize`, the shown queue view's `total_queue_len()` (the filter narrows nothing). `GuiQueueHeaderFacts.entries` loses its paint gate; it stays only if another reader needs it, else it is deleted. `--canvas-queue-header-height` becomes 48 px.

### D8. Chooser menu

The card anchors 4 px under the switcher, left-aligned, min-width 260, `--ef-bg2`, 1 px `--ef-bg4`, radius 8, padding 4. Items are 34 px, radius 5, `--ef-bg3` on the cursor and on hover. Each item: the check (`--ef-green`) on the shown entry, the name, then at the right the playing mark (`AudioLines`, `--ef-aqua`) or the current mark (the dot) or the peer brand mark, then the count in mono `--ef-grey2`.

The count rides the projection: `QueueChooserEntryProjection.len: usize` and `GuiQueueChooserEntry.len: usize`. `App::queue_chooser_entries` sets it from the freshest view: the shown resting queue's live view when the entry is that queue; `local_view.total_queue_len()` for the current queue; `NamedQueueEntry.len` for any other resting queue (the Owner keeps it fresh, see `NamedQueueList::entries`); `remote_view`'s `total_queue_len()` for the peer.

### D9. Queue rows

Row height 34 px, gap 2 px between rows, radius 6 (PDF part 05). `ROW_HEIGHT`/`ROW_HEIGHT_PX` move to 34 and the row stride to 36 (row plus gap), and every literal `30px` row size in `queue_panel.rs` (rows, spacer, ghost, actions button, grip offset, insertion-line math) reads the constants instead. The viewport capacity math keeps using the stride.

Cells: the 2-digit number (`--ef-grey1`, mono 11), the playing mark on the playing row, the title. The progress bar is absolute inside the row: 2 px tall, `bottom: 4px`, inset 54 px left and 12 px right, track `--ef-bg2`, fill `--ef-orange` (partial) or `--ef-green` (now playing). The title column is no longer a flex column with the bar under it.

States: the now-playing row fills `--ef-bg-green`, title `--ef-green` weight 600; the active and marked rows fill `--ef-bg3`; a hovered row fills `--ef-bg0`; a watched row paints `--ef-grey1` with a `--ef-bg5` strike. No zebra stripe and no left accent stripe: `GuiQueueRow::Item::striped`, its computation in `bridge/frame/queue.rs`, and `ItemRow::striped` are deleted. The hover chrome (grip, actions button) stays and sizes to the new row.

*Alternative:* keep the per-row `striped` fact unused. Rejected: dead code.

### D10. Module size

`queue_panel.rs` is at 796 lines. The row-size constants and the row-geometry helpers (`row_background`, insertion math, capacity) move to a new child module `queue_panel/geometry.rs` before other edits land, so the file stays under 800 lines.

## Risks / Trade-offs

- [mpv's `demuxer-cache-time` is unavailable for some sources (it reports an error before the demuxer starts)] → The observer leaves 0, so no buffered range paints.
- [The cache end can be behind the playhead after a seek] → The window paints the range only when it is ahead of the played fraction (D6).
- [Losing the trickplay preview and the chapter and segment marks loses information on Emby episodes] → Accepted: the design shows a time tip and a tick-free bar.
- [A 48 px header and 34 px rows change the queue's viewport capacity] → Capacity comes from the measured list height and the stride, so paging and scroll clamp follow.

## Migration Plan

None. `buffered_ticks` reads as 0 from an older Owner; a newer Owner's field is ignored by an older Client.
