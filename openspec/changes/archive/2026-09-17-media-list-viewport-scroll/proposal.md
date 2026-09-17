## Why

A canonical media list has no scroll of its own. Every input — the wheel and every keyboard chord —
moves the *selection*, and the visible window is recomputed from it each frame by one rule: the window
moves only far enough to keep the selection visible, so its top edge can never clear the selection's
row. Two consequences reach the user:

- The rows above the selection are unreachable. On a grouped list the first selectable row sits below
  its artist/letter `Heading`, so returning to the top stops one row short and the top row never comes
  back (issue #731, seen in the Music album rail). Nothing in the app can fix it: not the wheel, not
  `Home`, not a repaint.
- The wheel does not scroll. From the window's last row, the first `height - 1` wheel-up notches move
  only the selection, so a short flick moves no content at all.

This is the recurring flavour of the problem rather than a clamp defect: the legacy power-view painter
was patched for the same missing row (`510fe59`), the canonical owner written in September reimplemented
the edge-pinned rule without that patch, and the window's position now has three writers (the fixed-row
painter stores the offset it resolved, the panel's pre-paint sync stores a clamped copy, and the shell
re-seeds it on navigation) plus a fourth derivation for restored positions. A row index that is rewritten
by whoever painted last cannot be reasoned about, which is why each generation of painter re-breaks it.

The keyboard is the primary input here, and mbv's own key map already claims a scroll that does not
exist: the Help overlay documents `PgUp / PgDn` as "Page scroll" while the chord moves the selection by
five items.

## What Changes

- The shared media-list owner gains a **viewport step**: the visible window moves one row in the gesture
  direction, clamped to the content, and the selection is dragged along only when it would otherwise
  leave the window. The window can always reach the first and last display row.
- The window may sit on the `Heading` that labels the selection's group, so the first row and every
  group's label stay reachable. This is the reported bug, and it is fixed for every input, not just the
  wheel.
- The wheel binds to the viewport step on every canonical list surface. Keyboard movement keeps its
  documented meaning (`↑/↓` and `j/k` move the cursor), and a list-local chord (`Ctrl+e` / `Ctrl+y`)
  performs the same one-row viewport step so the verb is reachable without a pointer.
- `PgUp` / `PgDn` become the viewport's page step, making the Help overlay's "Page scroll" label true
  (today they jump five items).
- The window becomes owner state with **one writer**: the input path. The fixed-row painter stops storing
  the offset it resolved, and painting a list no longer mutates list state.
- A row-flow replacement — letter regrouping, Music album reorder or grouping settle, page append,
  refresh — re-anchors the window by its top visible stable target instead of keeping a row number.
  (Latent today; the same class as the reported bug.)
- Retire the hand-patches this model replaces: the Queue's `scroll.min(cursor)` clamp, the
  item-index-based restore derivation, and the paint-time write-back with the test that pins it.
- **BREAKING** (behaviour, not wire or on-disk format): the wheel and `PgUp`/`PgDn` no longer always
  change the selected row.

Not in scope: item activation and playback semantics, the configurable-keybind namespace (list-local
chords stay hard-coded for now), the Wide hero box's keyboard chord (issue #717 owns that), and any
persistence schema or wire change.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `mouse-input`: the uniform wheel policy's canonical-list behavior — a wheel step moves the list's
  viewport one row and keeps the selection visible, instead of advancing the selection; the per-surface
  verification record is updated to match.
- `canonical-media-lists`: a new requirement for the shared owner's viewport (a one-row and page step,
  reachability of the first and last display row, the labelling-`Heading` context, and the single writer),
  plus the existing anchor requirement extended to row-flow replacements.

## Impact

- `src/app/components/media_list/` (`mod.rs`, `wide.rs`, `carrier.rs`) — the viewport step, the window
  rule, the retained-height source, and the removed write-back.
- `src/app/render/components/media_list/wide.rs` — the painter stops calling `set_scroll`; the test that
  pins the write-back is replaced by a read-only pin.
- Wheel and key arms plus their shell echoes: `emby_library_content.rs`, `music_interaction.rs`,
  `music_content.rs`, `tv_content/interaction.rs` + `keyboard.rs`, `home_content.rs`, `feeds_content.rs`
  (whose blanket Ctrl/Alt early-return must admit the new chord), `podcast_content.rs`,
  `book_content.rs`, `inline_search.rs`, `queue.rs`, `library_panel/panel.rs`.
- `src/app/components/msg/shell.rs`, `src/app/shell_messages.rs`, `shell_emby_library.rs` — the resolved
  position the wheel reports (persistence and pagination reach rather than a selection move).
- `src/app/types_browse.rs` (`scroll_for_cursor`), `src/app/library_position_state.rs`,
  `src/app/shell_music_workspace.rs` — the restore/re-anchor path.
- `src/app/render/components/help.rs` — the key list gains the viewport chord.
- `CONTEXT.md` — the new domain terms for the viewport and its step.
- Existing specs: `openspec/specs/mouse-input/spec.md`, `openspec/specs/canonical-media-lists/spec.md`.
- No new dependencies, configuration, wire/API, or on-disk format changes.
