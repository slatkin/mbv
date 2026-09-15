## 1. Confirm the 719 baseline

- [ ] 1.1 Re-verify the merged 719 shape this change stacks on (stripe parity positions, gutter-selected rendering, `ZebraStripe`/`with_selected_gutter` API names) and verify the working tree is on the merge commit. If any behaviour differs from design.md D3, update the design before coding.

## 2. Make gutter-selected the unconditional Wide treatment

- [ ] 2.1 Delete the `with_selected_gutter`/`selected_gutter` opt-in from `WideMediaListPaintPolicy` (`src/app/components/media_list/mod.rs`), pass gutter unconditionally `true` from the Wide adapter (`render/components/media_list/wide.rs`), and drop the builder call in `src/app/render/components/queue.rs`, keeping its zebra pair. Add a comment at the Wide call site noting `selected_bg` is Inline-only now (design D2, Risks). Verify: `cargo check -p mbv` passes.
- [ ] 2.2 Correct the stale `row.rs` doc comment claiming the owning panel paints an edge marker if 719 hasn't (design Risks). Verify: comment matches the as-built rendering (bold focus-accent title, no background, no glyph).

## 3. Stripe the library Wide lists

- [ ] 3.1 Chain the library `ZebraStripe` (focused `#48584e`, unfocused `#2d353b`) onto both Wide policy arms in `src/app/components/library_panel/panel_list.rs` (design D1). Verify: `cargo check -p mbv` passes.
- [ ] 3.2 Update the `panel_list.rs` policy tests that pin the old selected-background fills for the Wide arms to the gutter treatment. Verify: the panel_list test module passes under `cargo nextest run -p mbv`.

## 4. Test

- [ ] 4.1 Add a Wide render regression test mirroring the queue zebra test: 4+ library items, zebra-enabled policy, striped positions carry the library focused secondary while the rest carry none, selected row shows bold focus-accent title with no background fill and no stripe. Verify: the new test passes under `cargo nextest run -p mbv`.
- [ ] 4.2 Add an unfocused-library render test: striped positions carry `#2d353b` and no row carries the bold focus-accent title. Verify: the new test passes under `cargo nextest run -p mbv`.

## 5. Gates

- [ ] 5.1 Run `cargo fmt`, `cargo check -p mbv`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv`, and verify all pass. Run the app and confirm the Browser pane, a provider workspace, and the queue show stripes with gutter-selected titles, and Narrow is visually unchanged.
