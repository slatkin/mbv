## 1. Confirm the #719 baseline

- [x] 1.1 Verify the tree is at or past `8a9132c2` (#719 merged, released in v0.19.3) and re-read the
  inherited facts against design.md Context: parity is `visible_item_index % 2 == 0` → 1st/3rd/5th
  (`src/app/render/components/media_list/wide.rs:97-98`); a selected row keeps its stripe under the
  accent (`media_list/row.rs:117`, `:215-233`); the Queue pair resolves through
  `palette::surface_colors(Surface::QueueColumn, focused)`, not raw hex
  (`src/app/render/components/queue.rs:31-39`). Verify: design.md Context matches the tree; if any
  behaviour differs, update design.md before coding.
  (Accepted: scout-confirmed at `032bd69d` — `8a9132c2` is an ancestor, all behaviours MATCH;
  actual refs: parity `wide.rs:102`/`118`, stripe-under-accent `row.rs`, Queue chain
  `queue.rs:35-43`, policy `mod.rs:211-274`, `set_paint_policy` `panel_list.rs:30-52`.)
- [x] 1.2 Confirm the synced main spec already carries both facts
  (`openspec/specs/canonical-media-lists/spec.md:52`, `:430`, `:437-440`). The archived delta's
  2nd/4th and "selected row always uses the selected-row background" wording is frozen history, not a
  gap this change must close — the first draft's "719 must correct its spec" precondition is already
  satisfied. Verify: no code change; record the confirmation in this change.
  (Accepted: confirmed — spec `:52` gutter-accent sentence, `:430` zebra requirement, `:439`
  1st/3rd/5th, `:447` selected-overrides-zebra; no code change.)

## 2. Make the gutter accent the unconditional Wide treatment

- [ ] 2.1 Delete `with_selected_gutter`, the `selected_gutter` field, and its accessor from
  `WideMediaListPaintPolicy` (`src/app/components/media_list/mod.rs:217-268`). Rename
  `media_list_row`'s `gutter_glyph: bool` parameter to `gutter_accent` (there has been no glyph since
  #719) and update every call site: the Wide adapter passes `true` unconditionally
  (`render/components/media_list/wide.rs:300-308`), the Inline adapter and the `#[cfg(test)]` wrapper
  keep passing `false`. Verify: `cargo check -p mbv`.
- [ ] 2.2 Drop the `.with_selected_gutter()` chain at the Queue call site
  (`src/app/render/components/queue.rs:31-39`), keeping its `QueueColumn` zebra pair. Verify:
  `cargo check -p mbv` and `cargo nextest run -p mbv` pass (the Queue-side tests already pin the accent).
- [ ] 2.3 Correct `row.rs`'s doc comment claiming the owning panel paints a marker outside the panel
  edge (`src/app/render/components/media_list/row.rs:22-29`), and add the design-D5 comment at the Wide
  call site that `selected_bg` now resolves the scrollbar backing only. Verify: comments match the
  as-built rendering (bold focus-accent title, no background, no glyph).

## 3. Stripe both library Wide arms

- [ ] 3.1 In `PanelList::set_paint_policy`
  (`src/app/components/library_panel/panel_list.rs:31-49`), chain a `ZebraStripe` built from
  `palette::surface_colors(Surface::MainContentBox, focused)` onto the `Wide` arm and one built from
  `palette::surface_colors(Surface::LibraryPanel, focused)` onto `WideWorkspace` (design D1). No raw
  colour value anywhere on either arm. Verify: `cargo check -p mbv`.
- [ ] 3.2 Re-pin `selected_row_surface_distinguishes_browser_and_workspace_slots`
  (`src/app/components/library_panel/panel_list.rs:104-140`) to the accent treatment. That test is
  currently the only proof separating the two arms and the accent makes their selected rows identical,
  so rename it to what it now proves and name the surviving owner proof for the arm distinction: the
  per-arm stripe tests from 4.1/4.2 (AGENTS "deletion evidence"). Verify: `cargo nextest run -p mbv`.

## 4. Test

- [ ] 4.1 Add the Browser-pane Wide stripe regression at the Render Component layer: a real
  `WideMediaList` painted through `render_wide_media_list_component` with the `Wide` policy, over 4+
  selectable items with one `Heading` and one `Spacer` in the flow. Assert the 1st/3rd item rows carry
  the `MainContentBox` focused fill, the 2nd/4th carry no secondary background, structural rows carry
  none and do not shift item parity — compared against `palette::surface_colors`, never a colour
  literal. Verify: `cargo nextest run -p mbv`.
- [ ] 4.2 Add the Workspace-arm counterpart asserting the `LibraryPanel` pair, plus one assertion per
  arm that its stripe colour differs from the fill its own list box is painted with (spec scenario "A
  stripe never equals its own panel body"). That assertion is the guard which would have caught the
  invisible-stripe error in the first draft. Verify: `cargo nextest run -p mbv`.
- [ ] 4.3 Add the accent regressions on the library path: focused selected row → bold focus-accent
  title, no selected-row background, and its stripe retained when it lands on a striped position
  (design D4); unfocused list → no row carries the accent and striped positions carry the pair's
  unfocused value. Scope the "no accent" scan to `Item` rows: `Heading` rows legitimately paint bold
  `TEXT_FOCUS_ACCENT` (`src/app/render/components/media_list/row.rs:44-52`). Verify:
  `cargo nextest run -p mbv`.
- [ ] 4.4 Convert `non_adjacent_multi_selected_rows_and_unfocused_cursor_paint_selected_surface`
  (`src/app/render/components/media_list.rs:354-400`) to the component adapter and to the new contract
  (accent title, no fill, own parity) per design D3. Audit the module's other `render_wide_media_list`
  wrapper uses (`:28`, `:174`, `:387`, `:416`, `:483`) and convert any that assert selection paint,
  leaving geometry and scroll-offset tests on the wrapper. Verify: `cargo nextest run -p mbv`.
- [ ] 4.5 Correct the `SelectedRowOnQueueColumn` and `SelectedRowOnLibraryPane` rows of the coverage
  table in `src/app/render/tests_surface_conformance.rs:41-42`, which cite a
  `components/tv_wide_tests.rs` that no longer exists: point them at the surviving proof (the Wide
  scrollbar backing, and the arm tests from 4.1-4.3) or record them as residuals with a concrete
  reason. Verify: `cargo nextest run -p mbv surface_conformance`.

## 5. Terms

- [ ] 5.1 Add **Zebra stripe** and **Gutter accent** to `CONTEXT.md` under Presentation, with `_Avoid_`
  synonyms naming the first draft's drift ("gutter-selected style", "gutter treatment", "selection
  marker"). Verify: the change's spec, design, and code comments use only the CONTEXT.md terms.

## 6. Gates

- [ ] 6.1 Run `cargo fmt`, `cargo check -p mbv`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv`, and
  `cargo test --release -p mbv` (CI runs the release profile, where a debug-only guard proves nothing).
  Verify: all pass.
- [ ] 6.2 Run the app and confirm: the Browser rail stripes lighter than its panel and the Workspace
  list stripes darker than its box, in both focus states; the selected row in each is bold focus-accent
  and keeps its stripe; Ctrl+Click/Visual rows match the selected row; the Queue is unchanged; Narrow
  is visually unchanged; the selected title reading like a `Heading` is acceptable at real widths.
  Verify: manual check recorded in this change before archiving.
