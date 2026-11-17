# Design

## Context

See `proposal.md` for the reason. The GUI window is a Dioxus tree laid out by flexbox, so the side
of each column is the order of its DOM children:

- `crates/mbv-gui/src/window/app.rs` (the root `div` with `data-arrangement`) paints, in order, the
  Queue column (`data-queue-column`), the Queue `PanelDivider` (`side: DividerSide::Left`,
  `name: DividerName::Queue`, only in two columns and not collapsed), and the `LibraryPanel`. The
  Settings branch paints `SettingsScreen` in place of all three.
- `crates/mbv-gui/src/window/components/library_panel.rs` paints, inside its content row, the Wide
  Hero pane (`data-library-region="hero"`, `flex-1`), the list `PanelDivider`
  (`side: DividerSide::Right`, `name: DividerName::List`), and the list (`data-library-region=
  "list"`, `flex-none border-l` in Wide, `flex-1` otherwise).
- `PanelDivider` (`components/panel_divider.rs`) already supports both sides. `DividerSide::Left`
  means the sized column is left of the divider (a drag to the right widens it). `DividerSide::Right`
  means the sized column is right of the divider (a drag to the left widens it). The band takes no
  width in the row.
- The collapsed rail (`components/collapsed_rail.rs`) paints the Lucide `ChevronsRight` icon as its
  expand control. `ChevronsRight` has no other caller.
- `resize.rs` stores and clamps the widths by column (`queue`, `list`). The clamps and the saved
  file `gui-panel-widths.toml` do not name a side.
- Pointer targets come from the DOM boxes that each region reports, so they follow the boxes.

## Goals / Non-Goals

**Goals:**

- Make the visual order Library panel then Queue column at the root, and list then Hero pane in
  the Wide Library panel, as the delta specs state.
- Keep the drag direction intuitive: the divider follows the pointer, and the sized column grows
  toward the window's center.

**Non-Goals:**

- No user setting for the side. The order is fixed.
- No change to the Settings screen's code. Its sidebar is already at the left edge.
- No change to the TUI, `mbv-render`, the Client core, `mbv-config`, or any saved state.
- No change to the Narrow Hero overlay, Mini, or the arrangement thresholds.

## Decisions

### D1. Reorder the DOM children; do not use `flex-direction: row-reverse` or `order`

The root row becomes `LibraryPanel`, then the Queue divider, then the Queue column. The Wide
content row becomes the list, then the list divider, then the Hero pane.

Alternative: keep the DOM order and add `flex-direction: row-reverse` (or CSS `order`). Rejected.
DOM order would then differ from the visual order. The library-panel tests read
`data-library-region` values "in paint order", region reports and hit testing walk the DOM, and a
reader of the `rsx!` tree would see the old order. One order is simpler to keep correct.

### D2. Flip each divider's `DividerSide`; change no drag math

The Queue divider takes `DividerSide::Right` (the Queue column is now right of it). The list
divider takes `DividerSide::Left` (the list is now left of it). The existing sign in
`DividerDrag` then gives the drag directions that the `gui-client` delta states. `clamp_queue_width`
and `clamp_list_width` stay as they are, because they bound a width and do not use a position.

### D3. Move the list's border to the edge that faces the Hero pane

In Wide the list paints `border-l` today, which is the edge that faces the Hero pane. After the
swap that edge is the right edge, so the class becomes `border-r`. This keeps the existing look;
it is not a new style.

### D4. Replace `ChevronsRight` with a new `ChevronsLeft` icon

The rail is at the right window edge, so its expand control must point into the window, to the
left. Add `ChevronsLeft` to `crates/mbv-gui/src/window/icons.rs` as a port of Lucide `chevrons-left` from the
same pinned `@lucide/svelte` 1.70.0 set (child paths `m11 17-5-5 5-5` and `m18 17-5-5 5-5`, class
`lucide lucide-chevrons-left`). `ChevronsRight` then has no caller, so delete it: one icon in, one out, so the module
comment's icon count stays correct. No new dependency.

Alternative: rotate `ChevronsRight` with a CSS transform. Rejected: the module ports Lucide icons
one to one, and a transform would make the class name lie.

### D5. No saved-state migration

`gui-panel-widths.toml` stores `queue` and `list` widths by column. After the swap the same column
reads the same value, so a width saved before the change keeps its meaning (the `gui-client` delta
scenario "A saved Queue width keeps its column").

### D6. No ADR

ADR 0036 makes the GUI Library panel a GUI-owned surface that does not follow the TUI. Column order
inside the GUI window is a layout choice under that ADR, not a new architectural decision. ADR 0027
(TUI Wide hero order) is not touched.

### D7. Tests change only where they pin the old order

- The library-panel test
  `the_panel_paints_the_top_bar_the_pill_row_and_the_hero_above_the_list_region`
  (`components/library_panel/tests/destinations.rs`) asserts the region order
  `["topBar", "pills", "hero"]`. It becomes the list-then-Hero order and its name changes to match.
- The resize test root in `window/app/tests/resize.rs` copies the window's root shape (Queue column
  then Library column). Its child order changes to match `app.rs`, so the copy stays a true copy.
  It owns the arrangement-per-resize contract, not the column order, so it gains no new assertion.
- One new test owns the Wide order contract ("The Wide list is at the left window edge"): in the
  library-panel fixture at Wide, the list region's box is left of the Hero region's box. It goes in
  `components/library_panel/tests/press.rs` beside the existing `rect_of("[data-library-region=
  \"list\"]")` use, because that fixture already lays out the real `LibraryPanel`.
- The root column order and the divider drag directions get no new unit test. No fixture lays out
  the real `app.rs` root (the resize fixture is a copy, so a test on it proves nothing about
  `app.rs`), and each is a one-line change of child order or of a `DividerSide` value whose sign
  the list divider already exercises. The user checks them in the live run after implementation.

## Risks / Trade-offs

- [Popovers near the new right edge] → The queue row menu anchors with `right` inside the Queue
  panel and the track-chip popover is right-aligned under its chip, so both open toward the window
  center. The Queue column is never narrower than its 400 px artboard (`clamp_queue_width`), so
  popovers inside it fit. The Library item menu anchors at the pointer inside the Library panel,
  which is now on the left, so it has more room than before. No code change is planned for them.
- [Merge conflict with `gui-named-queues`] → That change edits the queue header, rows, and footer
  and may touch `app.rs`. The conflict is limited to the root row's children; whichever change
  lands second rebases its `app.rs` hunk.
- [Docs that name the old side] → `docs/architecture/gui-client.md` and code comments (for
  example `library_panel.rs` "to the left of") describe the old order. A task updates them.

## Migration Plan

None. No saved state or protocol changes. Rollback is a revert of the change's commits.
