# Design

## Context

Both choosers hang a card under an anchor and paint rows in the Everforest
`--ef-*` tokens at the Queue rows' height and gap (`ROW_HEIGHT_PX`,
`ROW_GAP_PX`, `MENU_DROP_PX` in `queue_panel/geometry.rs`). Cursor state lives in
the core and arrives in the frame. The owner of each chooser builds one
`MarkHandler` per painted row from its region set and passes them down as props
(`QueuePanel` for the Queue chooser, `QueueTargetControl` for the target
chooser). A row calls its handler from `onmounted`. Invariant 38 rule 5 applies:
sharing the look does not move interaction authority.

The differences between the two copies, observed in source:

- Card: the Queue card has `aria-label="Queue chooser"` and the marker
  `data-queue-chooser-menu`; the target card has no label and the marker
  `data-target-chooser`. Nothing but `app.css` reads these markers.
- Row: the Queue row adds `justify-between` and `aria-label`; the Refresh row
  has no cursor fill, a `margin-top`, and a busy look (grey, 0.6 opacity).
- Row markers: `data-chooser-entry` (Queue), `data-choice` (target),
  `data-target-refresh` (Refresh). `app.css` has one rule on
  `[data-queue-chooser-menu] [data-chooser-entry]:hover`. The target chooser has
  no hover fill rule; the pointer already drives its cursor (ADR 0040).
- The Queue chooser's tests live in `queue_panel/tests/chooser.rs`. The target
  chooser's row selectors live in `queue_panel/tests/cross_panel.rs`. The names in
  issue #1059's nextest command (`queue_chooser_menu queue_target_control`) match
  no test, so the verification below names real filters.

## Goals / Non-Goals

**Goals:**

- One definition of the card and of the row mechanics, used by both choosers.
- No change in what either chooser paints, reports, or does on a key or press.

**Non-Goals:**

- No change to `LibraryDropdown`, `QueueRowMenu`, the track popover, or the
  `menu_card` canvas chrome (other token family).
- No change to the core, the frame, the region sets, or the mark order.
- No new accessible names. The target card keeps no `aria-label`.

## Decisions

**D1. One new module, `chooser_menu.rs`, two components, no new dependency.**
`ChooserMenu` and `ChooserRow` are `#[component]`s in
`crates/mbv-gui/src/window/components/chooser_menu.rs`, registered in
`components.rs`. The module uses only `dioxus_native::prelude` and crate items
already in use. Alternative: a function returning style strings. Rejected: it
leaves the role and mark wiring repeated, which is half of the duplication.

**D2. Props.**

- `ChooserMenu { kind: ChooserKind, label: Option<&'static str>, children:
  Element }`. `ChooserKind` is `Queue` or `Target`. The card renders `top:
  calc(100% + MENU_DROP_PX px); left: 0`, `min-width: 260px; max-width: 28rem`,
  padding 4 px, gap `ROW_GAP_PX`, the `Bg2` fill, the `Bg4` border, radius 8 px,
  and `--hover-veil: transparent`, with `role="menu"`. `label`, when set, is its
  `aria-label`. `CARD_PADDING_PX` moves here; both old copies are deleted.
- `ChooserRow { entry: String, cursor: bool, busy: bool, label: Option<String>,
  class: String, mark: Option<MarkHandler>, children: Element }`. It renders one
  `button` with `role="menuitem"`, class `flex w-full items-center text-left`
  plus `class`, height `ROW_HEIGHT_PX`, padding `0 8px`, gap 8 px, radius 5 px,
  13 px weight 500 type in `Fg`, and the `Bg3` fill when `cursor` is true.
  `busy` swaps the colour to `Grey1` and sets opacity 0.6 (the Refresh row while
  a discovery runs). `label`, when set, is the `aria-label`. `class` carries
  `justify-between` for the Queue row and is empty for the others. The row calls
  `mark` from `onmounted` and does nothing else with the event.

**D3. Row and card markers unify.** A Dioxus component cannot take a
dynamically named `data-*` attribute without attribute spreading, and the
repository has no spread component to follow. So the shared row always paints
`data-chooser-entry="{entry}"` and the shared card always paints
`data-chooser-menu="queue"` or `"target"` from `kind`. The Queue entry values stay
(`raw` queue id or `peer`). The target rows' values stay their `targetChoice`
index (`0`, `1`, …), and the Refresh row's value is `refresh`. The one hover rule
in `app.css` becomes `[data-chooser-menu='queue'] [data-chooser-entry]:hover`,
which keeps the pointer fill on the Queue chooser only and adds none to the
target chooser. Test selectors change from `[data-choice='N']` to
`[data-chooser-entry='N']` (in `cross_panel.rs`); the Queue selectors are
unchanged. Alternative: spread attributes with `extends = GlobalAttributes`.
Rejected: unproven in this repository and Blitz.

**D4. The Refresh row's `margin-top` is dropped.** It repeats the card's own gap
(`ROW_GAP_PX`), and both are 0 today, so nothing moves. The Refresh row uses
`ChooserRow` with `cursor: false`, `busy: refreshing`, and `entry: "refresh"`.
Its glyph and label stay its own children.

**D5. Owners keep what they own.** `QueuePanel` still builds the Queue marks;
`QueueTargetControl` still builds the target and Refresh marks. Each passes the
mark per row through the same props it passes today. The shared row never reads
the frame, the cursor source, or a region set.

**D6. Tests.** No new test pins a style string. The contracts stay with their
current owners: the chooser entry, mark, count and draft contracts in
`queue_panel/tests/chooser.rs`; brand marks and the report order of the open
chooser's rows (Refresh included) in `queue_panel/tests/cross_panel.rs`. The
change updates the one selector family in D3 and adds a test only where a step of
this change leaves a contract without any assertion (task 3.2).

## Risks / Trade-offs

- [A row loses its mark, so a hit region goes dead] → The report-order and entry
  tests in `queue_panel/tests` fail. Task 3.2 checks they assert the Refresh row.
- [The `app.generated.css` drifts from `app.css`] → Run `make gui-css` and commit
  the output with the selector change.
- [The `justify-between` or fill difference changes a layout] → The class and
  fill are passed through unchanged; the live look check is the user's
  acceptance step after implementation.
- [The `data-choice` rename misses a reader] → An `rg` over `crates/mbv-gui` and
  `docs` for `data-choice`, `data-target-chooser`, `data-queue-chooser-menu`, and
  `data-target-refresh` returns no match at the end of task 2.
