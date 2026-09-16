## 1. Palette authority and guards

- [x] 1.1 Declare the non-Wide library body as a named surface identity. Add
  `Surface::NarrowLibraryBody` (column/pane level, library-column focus source, level focused fill
  while focused, app backdrop while resting, declared in `RESTING_DEVIATIONS`) and pin its pair in
  `surface_resolve`'s `pinned_fills`. No existing row's value may move. Verify: no other row's pinned
  pair changes; the deviation list names the resting tone with its reason.
  (Accepted: `2cc73469` — row + deviation in `surface_table.rs`, `pinned_fills` pair
  `(SURFACE_FOCUSED, SURFACE_BACKDROP)`; the standalone primitive drafted first was deleted rather
  than kept as an unused constant.)
- [x] 1.2 Make `Model::library_body_fill` the single authority, guarded by the shared breakpoint
  predicate *and* the library panel-focus bit; Wide resolves the column's fixed backdrop and non-Wide
  resolves the new identity with that bit. Verify: the Wide expression returns the same value the
  column fill returned before the change.
  (Accepted: `2cc73469` — `src/app/shell_library_panel.rs`; the shell's placement fill consumes it.)
- [x] 1.3 The status band's padding rows take the same authority; the status row itself stays the
  status bar panel's paint. Verify: `render_status_bar_panel_at` fills the band with
  `library_body_fill` and then insets the bar row.
  (Accepted: `2cc73469`.)
- [x] 1.4 The Selector row's spacer row takes the owning panel's own identity and focus bit instead
  of a fixed chrome band, threaded through `paint_selector_row`/`paint_pill_row_gap`; the Wide call
  sites keep `PillRowGap`. Verify: both skeletons and the shared slot painters compile with the new
  parameters and the Wide spacer still resolves its old value.
  (Accepted: `2cc73469`.)

## 2. The list owns the surface under it

- [x] 2.1 Add the optional list body fill to `WideMediaListPaintPolicy` (`with_body`/`body_bg`); when
  set, the painter fills its claim rect before rows and the scrollbar column resolves that fill
  rather than the selected-row punch-through. Verify: callers that set no body (`Wide`,
  `WideWorkspace`, the Queue) paint exactly as before.
  (Accepted: `2cc73469` — `components/media_list/mod.rs`,
  `render/components/media_list/wide.rs`.)
- [x] 2.2 Add `PanelListPaintPolicy::Narrow` and resolve the list's body from the `MainContentBox`
  pair with the same focus bit the panel paints with; delete the skeleton's separate body fill so
  one owner remains. Verify: the non-Wide skeleton sets no fill of its own.
  (Accepted: `2cc73469` — `content.rs`, `panel_list.rs`, `narrow.rs`.)
- [x] 2.3 Inset the row flow by `PANE_PAD_Y` above and below while the list claims the whole list
  panel, so the spacer rows belong to the inset's own surface. Retained geometry is the inset.
  Verify: `set_geometry` claim ≠ content, hit resolution and the returned skeleton geometry read the
  inset, and the spacer rows paint the inset's fill.
  (Accepted: `1f086fc4`; the first attempt (`4535af6c`) inset the claim too, which painted the
  spacers as the surrounding panel, and was superseded before any release.)

## 3. Stripe pair

- [x] 3.1 Resolve the non-Wide stripe from `Surface::LibraryPanel` so the secondary zebra row is
  `#3c4841` focused and `#333c43` resting, distinct from the list body in both states. Verify: the
  stripe pair differs from the body pair while focused and while resting.
  (Accepted: `545baa7d`.)

## 4. Contracts and gates

- [x] 4.1 Update the non-Wide skeleton test to assert the inset relation (row flow starts one
  `PANE_PAD_Y` below the List controls row) instead of equality with it.
  (Accepted: `e235ffa2` — `narrow_tests.rs`.)
- [x] 4.2 Record the new identity on the surface-conformance residual list with its reason (painted
  by the shell's placement fill, not a component view; pair pinned by `pinned_fills`).
  (Accepted: `e235ffa2` — `tests_surface_conformance.rs`.)
- [x] 4.3 Gates at the landed head: `cargo fmt --all -- --check`, `cargo check -p mbv --all-targets`,
  `cargo nextest run -p mbv --no-fail-fast` (**1449/1449 pass**), `cargo clippy --workspace
  --all-targets -- -D warnings` (clean).
  (Accepted at `e235ffa2`.)
- [x] 4.4 Manual live check on a real terminal, which carries the appearance claim the suites cannot:
  focused and resting non-Wide library body, inset surface, its spacer rows, the scrollbar column
  and the secondary zebra row; the same in Mini; and that Wide is unchanged in both focus states.
  (Done — verified by the user on a real terminal, 2026-09-16.)

## 5. Durable record

- [x] 5.1 Record the extension rule as architecture: a geometry-scoped appearance is a named
  identity or a geometry's own paint policy resolved at that geometry's call site, never an edit to a
  shared surface-table value.
  (Accepted: ADR 0028, this change.)
- [x] 5.2 File the deferred follow-up for the mini library playback panel colours.
  (Accepted: slatkin/mbv#726.)
