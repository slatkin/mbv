# Design

## Context

The queue column composes `QueuePlaybackPanel` (the player block: header row, artwork,
title, scrubber, transport, track chips), then `QueuePanel` (the recessed queue box), then
`QueueFooter` (the 40 px footer band). The header row (`queue_playback_panel.rs`'s
`HeaderRow`) paints the playback-target control, the now-playing status word, and the
collapse control; the target chooser's popover is `TargetChooserPopover`, anchored
`top-full` below the header. The footer band reports an empty `LayoutContribution` today:
its module comment says "a point on the line resolves to no target". Region resolution is
core-side: the window reports `QueuePlaybackTargetId::{PlaybackTarget, Collapse,
TargetChoice}` regions and the Client core resolves presses, so a control can move between
components without any core change.

## Goals / Non-Goals

**Goals:**

- The queue column's topmost painted content is the artwork, with no band above it.
- The footer band owns the playback-target control and the collapse control, before the
  autosave/source/dirty content in its left group.
- The target chooser opens upward, above the footer band.
- The GUI frame stops carrying the now-playing status fact.

**Non-Goals:**

- No change to the Client core: the collapse toggle, the chooser's cursor and live choice
  list, the `z` chord, and the `q` fold stay exactly as they are.
- No change to the library projection (`NowPlayingStatus`): the TUI Status bar keeps its
  own use of the fact. Only the GUI bridge frame drops it.
- No change to the Queue header band (`queue_header.rs`: the queue chooser, filter, new,
  and close controls) — it is not the header this change removes.
- No new region target ids and no protocol change.

## Decisions

**D1 — The moved controls lead the footer's left group.** Order: the playback-target
control, then the collapse control, then the existing autosave/source/dirty content. The
existing content paints only for a saved playlist, so a plain local queue sees only the
two controls — the user's request ("the left side of the queue footer") with nothing in
front. Alternative rejected: putting the controls after the source text, which would bury
them behind content that rarely paints.

**D2 — The controls compact to the 40 px band.** The header's 44 px button boxes become
28 px-high controls; the footer band keeps its 40 px height, its type, and its border. The
collapse control keeps its aria-label, its `Collapse panel (z)` title, and its muted
Maximize2 glyph; the target control keeps its Cast glyph, host label, and chevron at the
band's 12 px mono size. No footer geometry moves.

**D3 — The chooser popover opens upward, anchored to the target control.** The footer
sits at the window's bottom edge, so the popover flips to `bottom-full` (above the
button) instead of the header's `top-full`. Its rows, the attached output's check, and
the brand icons are the popover's current ones, moved with it. Esc and an outside press
close it through the same core paths.

**D4 — The footer's contribution reports the moved regions; the popover's rows report
under a second key.** The footer component mounts after `QueuePanel`, so its own key lands
after the queue rows in the merged report — the footer's controls win where they paint,
and no extra ordering trick is needed. The popover's rows report under a chooser key the
footer owns, the same second-key technique the playback panel uses today, so the rows win
over the queue rows beneath them. The status word paints no region (it never did).

**D5 — The now-playing status fact leaves the GUI frame.** The header row was the only
consumer of `GuiQueuePlayback::status` and `GuiNowPlayingStatus`, so both are deleted
(`bridge/frame/playback.rs`, the bridge sections builder, and the test fixtures that
construct a playback section). The projection keeps carrying its status — the conversion
is simply dropped. Alternative rejected: keeping the field unpainted, which leaves dead
state in the frame.

**D6 — The artwork starts at the column's top edge.** With the header row gone, no band
replaces it and nothing realigns with the Library top bar: the Library top bar keeps its
own 56 px band, and the queue column's top edge is the artwork's (or, with no item, the
column's backdrop above the queue box). The still slot's own shapes, caption, and badge
are unchanged.

## Risks / Trade-offs

- The collapse control is reachable only through the footer and the `z` chord now. While
  the Queue column is collapsed to its rail, the footer does not paint — but the rail
  already owns the expand control, so collapse and expand stay reachable in every state.
- The popover opening upward can overlay the last queue rows. That is the same trade the
  header's downward popover made over the artwork, and the rows report under the footer's
  chooser key, so a press on a popover row resolves to the row, not the queue row beneath
  it.
- The status word's removal loses one redundant state readout; a paused panel now reads
  from the transport's play glyph alone. The user chose this.
