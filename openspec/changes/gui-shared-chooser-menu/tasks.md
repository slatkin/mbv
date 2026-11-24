# Tasks

## 1. Shared components

- [ ] 1.1 Create `crates/mbv-gui/src/window/components/chooser_menu.rs` with `ChooserKind`, `ChooserMenu`, and `ChooserRow` as design D1–D3 state, with a module doc that says what each owns and that cursor and dispatch stay with the chooser's owner. Register `pub mod chooser_menu;` in `components.rs`. Verify: `cargo check -p mbv-gui` passes.

## 2. Move both choosers onto them

- [ ] 2.1 Rewrite `QueueChooserMenu` in `queue_chooser_menu.rs` to render `ChooserMenu { kind: Queue, label: Some("Queue chooser") }` and one `ChooserRow` per entry (`entry` from `entry_key`, `cursor`, `label`, `class: "justify-between"`, `mark` from `row_marks`). Delete its card style, row style, `CARD_PADDING_PX`, and its `onmounted` block; keep the row content, `EntryPaint`, and `QueueChooserMenuState`. Update the module doc where it describes the card. Verify: `cargo check -p mbv-gui` passes and `rg -n 'padding: 0 8px|border-radius: 8px' crates/mbv-gui/src/window/components/queue_chooser_menu.rs` returns nothing.
- [ ] 2.2 Rewrite `TargetChooserPopover` in `queue_target_control.rs` to render `ChooserMenu { kind: Target, label: None }`, one `ChooserRow` per choice (`entry` is the index string), and the Refresh row as a `ChooserRow { entry: "refresh", cursor: false, busy: refreshing }`. Delete its card and row style strings, `CARD_PADDING_PX`, the `margin-top` (design D4), and the `onmounted` blocks. Drop the now-unused imports. Update the file's popover doc. Verify: `cargo check -p mbv-gui` passes and `rg -n 'padding: 0 8px|border-radius: 8px' crates/mbv-gui/src/window/components/queue_target_control.rs` returns nothing.
- [ ] 2.3 Change the `app.css` hover rule to `[data-chooser-menu='queue'] [data-chooser-entry]:hover`, run `make gui-css`, and commit the generated file. Change the `data-choice` selectors in `queue_panel/tests/cross_panel.rs` to `data-chooser-entry`. Verify: `rg -n 'data-choice|data-target-chooser|data-queue-chooser-menu|data-target-refresh' crates/mbv-gui docs --glob '!*.generated.css'` returns nothing, and `rg -c 'data-chooser-menu' crates/mbv-gui/assets/app.generated.css` returns 1.

## 3. Contracts and docs

- [ ] 3.1 Run `cargo nextest run -p mbv-gui queue_panel::tests::chooser queue_panel::tests::cross_panel gui_core::input::tests::queue_chooser gui_core::input::tests::target_chooser` and verify it passes with the same tests green as before the change.
- [ ] 3.2 Read `an_open_chooser_reports_its_rows_after_the_queue_and_footer_regions` in `cross_panel.rs`. If it asserts no Refresh-row region, extend it to assert the Refresh row reports last (extend the test; add no new one). Verify: the extended test passes.
- [ ] 3.3 Update `docs/invariants/38-repeated-gui-shapes-have-one-mechanics-owner.md`: remove the Queue-chooser bullet from the failure list and the "still repeat" note, and add `ChooserMenu` and `ChooserRow` to the upheld list. Update the Queue chooser and target chooser prose in `docs/architecture/gui-client.md` only where it names the old markers or two cards. Verify: `rg -n 'TargetChooserPopover' docs` shows no claim of a copied card.

## 4. Gates

- [ ] 4.1 Run `cargo fmt`, then `cargo clippy -p mbv-gui --all-targets -- -D warnings`, each unpiped in the foreground, and verify both exit 0. Run `make check-code-file-lines` and verify it exits 0.

## Workflow follow-up

- The user does the live look check of both choosers (open, cursor, rename, Refresh, brand icons) before the PR opens.
- Archive the change as the last commit before merge, with `skip_specs: true` (no delta to sync).
