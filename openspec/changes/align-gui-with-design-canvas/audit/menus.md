# Menus artboard — gap list (canvas is the spec)

Scope: `Menus.dc.html` (subtitle popover from the CC chip, row hover + row context menu, drag-reorder)
against the current UI (`crates/mbv-gui/ui/src/lib/components/TrackChips.svelte`,
`QueueRowMenu.svelte`, `QueuePanel.svelte`) and the wire DTOs
(`crates/mbv-gui/src/bridge/frame/{overlay,playback,queue}.rs`). Read-only recon; no fixes proposed.

## G1 — Subtitle offset stepper: whole control missing, no wire data for it

- Canvas: an `Offset` row (`"Offset"`, `12px #9da9a0`) with a stepper —
  `"−"` button (`32x32 #2d353b`), value `"+0.25s"`
  (`JetBrains Mono 12px, width 52px, #e9e1cc`), `"+"` button — plus the footer hint
  `"c · cycle   [ ] · offset"` (`mono 11px #9da9a0`).
- Current UI: no offset row, no stepper, no footer hint anywhere in the popover
  (`TrackChips.svelte` `popoverList` renders only choice rows + `forced` tag).
  The omission is deliberate today: `TrackChips.svelte:18` —
  `There is no subtitle-offset control`.
- Kind: **DATA-GAP** (also MISSING in paint).
- Needed: a current-offset fact on the wire (e.g. `offsetSecs`/`offsetTicks` on the
  subtitle chip or the `trackPopover` overlay) plus step commands (earlier/later,
  and whatever `[` / `]` should do while the popover owns the keyboard).
  The data does **not** exist in the core: no `sub-delay`/`sub_delay`/offset state
  in `src/` or `crates/mbv-player` (searched, zero matches); the subtitle system
  only cycles tracks (`cycle_subtitle_mode`, `src/app/dispatch/audio_subtitle.rs`).
  New core state + player wiring required, not just a DTO field.

## G2 — "Remove from queue" is not destructive red

- Canvas: `"Remove from queue"` + `"dd"` painted in `"#e67e80"`, separated by a
  `"#475258"` divider from the four neutral entries.
- Current UI: `QueueRowMenu.svelte:45-70` paints every entry with the same
  cursor-vs-neutral tone; nothing distinguishes the remove entry (no red role).
- Kind: **DATA-GAP** (visual consequence).
- Needed: e.g. `destructive: boolean` (or a `tone`/`kind`) on `GuiRowMenuEntry`
  (`bridge/frame/overlay.rs`). Does not exist in the core either:
  `ContextMenuEntry` (`crates/mbv-ui-model/src/context_menu.rs:143`) carries only
  `label` / `action` / `key_hint`; the TUI render side has no destructive paint
  (no matches in `mbv-render`). Core must mark the entry before the DTO can carry it.

## G3 — Open CC chip has no expanded/highlighted state

- Canvas: the open chip `"CC EN"` carries `"aria-expanded=\"true\""` with
  `"background: #475258; border: 1px solid #e69875"` (the other two chips sit on
  `"background: #343f44; border: 1px solid #475258"`).
- Current UI: the audio/subtitle chip buttons (`TrackChips.svelte:227,245`) restyle
  never — no `aria-expanded`, no open-state border/fill; the open popover is the
  only sign.
- Kind: **WRONG-VISUAL**.

## G4 — Track popover chrome diverges from the canvas dialog

- Canvas: dialog `"width: 260px; background: #343f44; border: 1px solid #475258;`
  `border-radius: 8px; box-shadow: 0 12px 32px rgba(0,0,0,0.45); padding: 6px"`,
  rows `"height: 36px ... font-size: 13px"`, chosen row `"background: #3d484d;`
  `color: #e9e1cc"` with a `"stroke=\"#a7c080\""` check, divider
  `"height: 1px; background: #475258; margin: 4px 0"`.
- Current UI: `TrackChips.svelte` popover snippet is `absolute bottom-full left-0 ...
  min-w-48 rounded-md border py-1 shadow-md` in theme vars, padding-sized rows, no
  fixed 260 px / 36 px rows, check in the generic accent role, and no divider
  (nothing to divide without the offset section).
- Kind: **WRONG-VISUAL**.

## G5 — Popover key-hint footer line missing; hint glyphs differ

- Canvas footer: `"c · cycle   [ ] · offset"`. Canvas menu hints: `"⏎"`, `"n"`, `"w"`,
  `"g"`, `"dd"` (`mono 11px #9da9a0`).
- Current UI: no footer line in the popover (see G1). `QueueRowMenu.svelte:59-63`
  paints `entry.keyHint` verbatim, so Play now shows `"Enter"`, not `"⏎"`
  (core sends `key_hint: Some("Enter")`, `src/app/gui_client_state.rs` row-menu
  builder; tests pin `"Enter"`, `QueuePanel.test.ts:97`). The `c` cycle exists in
  the core (`GuiCycleSubtitle`, `key_policy.rs:242`) but is undiscoverable in paint.
- Kind: **MISSING** (footer line; glyph mapping is WRONG-VISUAL).

## G6 — Row hover chrome diverges from the canvas hovered row

- Canvas hovered row: `"background: #3d484d; border-radius: 4px"`,
  `"padding: 0 2px 0 4px"`, six-dot grip svg (`fill="#9da9a0"`, in-flow at the
  leading edge), title ellipsis, and a `"width: 30px; height: 30px ...`
  `background: #475258; border-radius: 4px"` three-dot button (`Row actions`).
- Current UI (`QueuePanel.svelte` hover block): `GripVertical` lucide icon absolutely
  positioned at `left: 2px`, `Ellipsis` button at `right: 2px` with `p-0.5` on
  `var(--mbv-secondary)` — smaller hit area, different grip glyph, no 30 px button
  box; row fill comes from theme vars.
- Kind: **WRONG-VISUAL**.

## G7 — Row menu placement and size differ

- Canvas: `role="menu"`, `"width: 230px; margin-left: 154px; margin-top: -40px"` —
  the menu overlaps the anchor row, entries `"height: 34px"`, cursor entry
  (`"Play now"`) filled `"#3d484d"`.
- Current UI: `QueueRowMenu.svelte` is `min-w-52` anchored *below* the anchor row
  (`menuTop` = header + below-anchor, `QueuePanel.svelte` `menuTop` derived), right
  offset `var(--mbv-cell-col)`; entry heights are padding-derived; cursor styling is
  a text-accent tone, not a filled row.
- Kind: **WRONG-VISUAL**.

## G8 — Drag ghost styling diverges

- Canvas ghost: `"position: absolute; left: 28px; right: -6px; top: 22px; ...`
  `background: #475258; border-radius: 4px; ... transform: rotate(-1deg)"`,
  content is title + `"5%"` (`mono 12px #e69875`).
- Current UI (`QueuePanel.svelte:506` ghost block): full-width `left-0 right-0` card
  pinned at the insertion line, `background: var(--mbv-popover)` with an accent
  border, no rotation, no x-offset; progress renders as a thin bar (`rowCells`),
  never as a `"5%"` label.
- Kind: **WRONG-VISUAL**.

## G9 — Insertion line styling diverges

- Canvas: `"height: 2px; background: #83c092; margin: 0 8px; border-radius: 1px"`.
- Current UI (`QueuePanel.svelte:459`): `height: 2px`, `background: var(--mbv-accent)`,
  `left-0 right-0`, no inset margin, no radius.
- Kind: **WRONG-VISUAL** (minor).

## G10 — Chip and track label richness: canvas suffixes have no wire source

- Canvas chips: `"FHD · direct"`, `"EN 2.0"`, `"CC EN"`; popover rows
  `"English · SRT"` and `"English SDH · embedded"` (codec/source suffixes,
  SDH/forced distinction in the label itself).
- Current UI: video chip is the bare resolved label (`"1080p"` — core emits only
  `short_resolution_label(height)`, `queue_playback_projection.rs:track_chips`);
  choice labels paint verbatim from `TrackChoice.label`, which is the mpv-provided
  label passed through. Whether mpv labels ever yield `"· SRT"` / `"· embedded"` /
  `"2.0"` / `"· direct"` suffixes was not verified in this recon (needs a live
  player); nothing in the DTOs carries source-medium or channel-count facts.
- Kind: **DATA-GAP** (possible; confirm against live `status_snapshot` labels —
  if mpv labels lack them, new fields are needed).

## G11 — Dragged row dims in place (canvas shows no dim)

- Canvas drag section: соседние rows paint normally; the lifted content lives only
  in the floating card.
- Current UI: `QueuePanel.svelte:443` — `class:opacity-40={queue.drag?.slot === row.slot}`.
- Kind: **WRONG-VISUAL** (minor addition, not a missing element).

## G12 — `[` / `]` conflict with the canvas offset hint

- Canvas: `"[ ] · offset"` — brackets nudge the subtitle offset.
- Core: in the GUI key context `[` / `]` are seek back-10 s / forward-30 s
  (`src/app/input/gui/tests.rs:505`, `seek_relative_request_matches_the_seek_chords`).
  If the offset stepper lands, the popover-open key rules must shadow or rebind
  these chords, or the canvas hint is unimplementable as drawn.
- Kind: **WRONG-BEHAVIOR** (latent conflict; bites only when G1 is built).
