## Context

See `proposal.md` for the motivation and the two spec deltas for the contract. The mechanics this design
has to work with:

- `MediaList<Target>` (`src/app/components/media_list/mod.rs`) owns the rows, the ascending selectable
  index, the cursor, and a `scroll` display-row offset. `resolve_viewport(height)` is the single rule
  today: the stored offset is clamped to `total - height`, then raised to the cursor's display row when
  the cursor is above it, or lowered so the cursor sits on the last row when it is below. The painter
  (`render/components/media_list/wide.rs`, via `WideMediaList::set_scroll`) stores the resolved offset back
  into the owner, and
  `MediaListCarrier::sync_viewport` stores a second, pre-paint clamped copy.
- Every list input is a selection move. `MediaListSurfaceInput::Wheel` converts to
  `MediaListOperation::Move` in one place (`MediaListSurfaceInput::into_operation` in
  `components/media_list/mod.rs`), except the Emby owner's wheel arm (`EmbyLibraryContent::handle_key`),
  which builds `Move` directly. The keyboard chords (`↑/↓`, `j/k`,
  `PgUp/PgDn` = `Page` = five items, `Home/End` = `First/Last`) are handled per destination and all route
  through the same owner. No chord anywhere in a list moves a viewport.
- `WideMediaList` already retains the frame it painted — claim rectangle, content rectangle, row geometry,
  selected-row rectangle — and hit resolution already runs against that retained geometry (`resolve_current_point`).
- `ViewportAnchor` + `apply_viewport_anchor` already implement "carry a stable target plus a screen-row
  offset across a change to the row flow's geometry"; today it is used only at breakpoint hand-offs.
- `BrowseLevel::scroll_for_cursor(cursor, visible)` derives a restore offset from an *item* index while the
  owner consumes a *display-row* offset, and `QueueComponent::set_cursor` hand-clamps `scroll.min(cursor)`.

## Goals / Non-Goals

**Goals:**

- One viewport rule, one writer, one place to reason about.
- A wheel or key step that moves the content one row, with the selection riding only when it would leave.
- The first and last display row reachable by stepping, from every input.
- The paint stays read-only: no render pass mutates list state.
- The window keeps the user's place when the rows under it are replaced.

**Non-Goals:**

- The non-media-list overlays (Global Search sidebar, Settings, Help, Sessions, Playlists) keep their own
  wheel and window behaviour; unifying them is separate work.
- The Wide hero overview box's keyboard chord (issue #717).
- Leaf-local chord rebinding: `add-configurable-keybinds` covers the prefix namespace and router globals,
  not per-component keys, so the new chord is hard-coded like its neighbours.
- Persisting the window. The on-disk library position stays cursor-only; a restore seeds the window at the
  restore boundary.

## Decisions

### D1 The viewport step is an owner operation, converted in one place

`MediaList` gains a viewport step and a page step next to `Move`/`Page`/`First`/`Last`. The wheel's
conversion (`into_operation`) maps `Wheel` to the viewport step instead of `Move`, so every surface that
accepts a wheel through the shared seam gets the new behaviour at once. `Page` becomes the page step.

Alternative considered: keep the wheel as `Move` and only relax the window rule. Rejected — it fixes the
missing row but leaves the wheel moving a selection, which is the dead-notch and "the wheel does not
scroll" half of the report.

### D2 Height enters the input path from the retained frame, not from new state

The owner stays height-free and takes the height as an argument, exactly as `resolve_viewport` does. The
carrier resolves it from `WideMediaList::current_content_rect()` — the retained rectangle of the frame it
painted, the same source `resolve_current_point` uses for hit resolution.

Alternatives considered: store the height on the owner at paint time (makes the paint a writer again —
the thing this change removes); thread the height from the panel through every destination's input arm
(new plumbing per surface, no single source).

The retained rectangle is the last painted frame, so two edge cases need explicit dispositions: with no
frame yet painted, the step is a no-op (`Unhandled`); with a frame staler than a geometry change, a
step (a page step in particular) may overshoot by one frame, and the display-only clamp at the next
paint recovers it. The owner's window is never corrected from a stale height.

### D3 The drag rule resolves to a selectable row

Display rows include non-selectable `Heading`/`Spacer` rows, so "drag the selection into the window" means
the nearest selectable row at or below the window's top, or at or above its last row, resolved against the
ascending selectable index. A step whose window still contains the selection moves nothing but the window.
A step is never a live-range extension: even when the drag moves the selection, `multi_selection` and the
anchored range are untouched, because a step is a viewport gesture, not a cursor move.

### D4 Cursor moves keep one row of leading context

A cursor move that would leave the selection outside the window moves the window the minimum distance that
shows it. When that places the selection on the window's first row and the row above the selection is the
`Heading` that labels its group, the window moves one row further so the label stays visible. This is the
generalisation of the July fix in the legacy power-view painter (`510fe59`), and it is what makes `Home`
or a walk up the list return the first group's label. It applies to the cursor path only: a *step* may
legitimately scroll the label off, because the step can always be reversed.

### D5 A row-flow replacement re-anchors by stable identity

`MediaList::set_content` records the first selectable target the previous flow showed at the window's top
**plus whether a `Heading` was painted directly above it**, and after installing the new rows re-finds that
target and restores the window to its row — or to the `Heading` directly above it, when the previous flow
showed one there and the new flow still places one. `Heading` carries no stable identity (text only), so
the label is matched structurally ("the row directly above the target in each flow"), never by text. If
the target is gone, the window falls back to keeping the selection visible and clamping. No display-row
index crosses a flow replacement. This is what keeps D4's leading-context invariant across the
asynchronous Music grouping settle: an anchor restored without its label reintroduces the missing-`Heading`
bug this change fixes, on every regroup.

- Alternatives considered: keep the index (today's behaviour) — meaningless after a reorder, which is how the
asynchronous Music grouping settle can misplace the window; always re-anchor to the selection — loses the
user's reading position on every ordinary content refresh.

Context note: the `ViewportAnchor` machinery D5 revives is currently prod-dead — its only producer is
`#[cfg_attr(not(test), allow(dead_code))]` and `apply_viewport_anchor` is test-only since commit `0f9bd770`.
This change puts it back on the production path; no decision changes.

### D6 `PgUp`/`PgDn` become the page step

A page is the painted row-flow height, so the Help overlay's "Page scroll" label becomes true. The old
five-item jump has no other claimant: the hidden-truth cost of keeping it is that the documentation keeps
lying about the only chord named after scrolling.

Migration note: the page step is a height-taking owner method, not a reuse of the `MediaListOperation::Page`
variant. The variant keeps its five-item selection meaning until tasks 6.1 and 6.2 have converted every
`PgUp`/`PgDn` arm (6.2 covers the Queue, Home, Feeds, podcast, book, and Inline Search surfaces, which
still route through the variant via `MediaListSurfaceInput::Page`), at which point the variant is deleted;
the two meanings never coexist inside one surface.

Risk accepted: a page is a height, so a page differs between Wide and narrow. Verification covers both
breakpoints (the mouse-input spec already requires per-breakpoint evidence).

### D7 The one-row chord is `Ctrl+e` / `Ctrl+y`

The less/vim "scroll one line" binding, free in every list's local keyspace (`Ctrl+y` appears nowhere in
the list components; the only `e` binding is Feeds' unmodified enqueue). `Ctrl+d`/`Ctrl+u` are avoided
because they mean *half page* in the same tradition and would be ambiguous with a one-row step.

Feeds' handler currently returns early for any Ctrl or Alt chord before its match
(`FeedsComponent::handle_key`); it needs a narrow exception for these two chords (Feeds has no Ctrl
bindings to collide with), with a test.

### D8 A step reports the position it reached, not a selection move

The destinations' shell echoes split into two facts. A selection move is reported only when the step
actually dragged the selection — `MediaListTransition.selected_target` already carries exactly that signal
(`(before != after)`), so the existing arms gate on it instead of echoing unconditionally. The position the
window reached is reported where an existing operation needs it: the Library panel's deferred resting-scroll
update, and pagination, which resolves the window's last visible display row to the item index it feeds
`maybe_fetch_next_page`.

### D9 The retired hand-patches

- The painter's `set_scroll` write-back and the test that pins it (`painter_persists_resolved_scroll_offset_across_frames`)
  → replaced by a pin that a paint does not change the window.
- `MediaListCarrier::sync_viewport` keeps the height-aware clamp but stops storing a resolved offset.
- `BrowseLevel::scroll_for_cursor` and the item-index restore derivation → a restore seeds the selection;
  the window derives from the selection's display row once the flow exists.
- `QueueComponent::set_cursor`'s `scroll.min(cursor)` clamp → unnecessary once the window is authoritative.

## Risks / Trade-offs

- [The wheel no longer moves the selection, so a wheel-then-Enter user acts on the row the selection is on,
  not the row under the pointer] → The selection is always painted and always inside the window; the
  spec requires it stay visible, and a step that drags it moves it to the nearest shown row.
- [Pagination could stall when the wheel only scrolls] → D8 requires the step to report the window's reach;
  the mouse-input verification scenario pins that a viewport-only step still reports position when an
  existing persistence or pagination operation needs it.
- [A stale window could survive a height change with no input] → the panel's per-frame height sync stays as
  the one clamp, and the "shorter paint does not raise the window" scenario pins the observable result.
- [The leading-context rule adds a display-row exception] → limited to the cursor path, one row, and only
  when that row is the labelling `Heading`; the step path is clamped solely by the content ends.
- [A restore no longer reproduces "selection at the window's bottom edge"] → position tests change with the
  rule; the restore seeds one explicit position and the visible-selection invariant holds.
- [The Music grouping settle reorders rows asynchronously while the user reads] → D5's anchor is the
  mitigation; the verification drives the commit directly rather than waiting on the settle window, keeping
  the test deterministic.
- [Ledger and spec drift] → the mouse-input verification record and
  `docs/architecture/interactive-surface-ledger.md` are updated in the same slice that converts each surface.

## Migration Plan

1. Owner rules first, with no destination changes: the viewport/page step, the single writer, the
   row-flow anchor, and owner unit tests (including the reachability and boundary cases).
2. Make the paint read-only: drop the painter's write-back, keep the panel's height clamp, replace the
   pinned write-back test.
3. Convert the wheel sites to the step (the shared `into_operation` arm plus the direct-build site in
   `emby_library_content.rs`), gating each destination's cursor echo on an actual selection move.
4. Add the chord and `PgUp`/`PgDn` page step per surface, with wide and narrow evidence, and add the chord
   to the Help overlay's key list.
5. Retire the hand-patches from D9 and update the tests that pin them.
6. Integration evidence through `Application::tick()`, then the spec/ledger sweep and the `CONTEXT.md`
   domain terms for the viewport and its step.

Rollback is per slice: revert the commit; there is no persisted format, protocol, or configuration change.

## Open Questions

- Half-page chords (`Ctrl+d` / `Ctrl+u`) are deliberately not added; they can follow without changing this
  design if the page step proves too coarse.
- Whether the non-media-list overlays adopt the same verb later is their own work; nothing here blocks it.
