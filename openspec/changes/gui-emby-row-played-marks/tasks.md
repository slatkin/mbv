# Tasks

All work is in `crates/mbv-gui`. Every verification names `-p mbv-gui`. Tests follow `crates/mbv-gui/AGENTS.md` and `docs/invariants/14-test-ownership.md`: headless painted-structure assertions only, one contract per test, extend an existing table before you add a test. No lint suppression, no `allow` or `expect` attribute, no sleeps. Run `cargo fmt` after each Rust edit. Run cargo commands in the foreground and do not pipe them.

## 1. The projection carries the state (design D1)

- [x] 1.1 In `crates/mbv-gui/src/gui_core/gui_library/emby_projection.rs`, add `state: MediaSemanticState` to `GuiEmbyRow` (documented as design D1 states: the one derivation the TUI's rows use, collections included, music ordinary through `from_emby`'s exception). Set it in `plain_row` and `feed_row` — the only two construction sites — as `MediaSemanticState::from_emby(item)`. Update the struct and module docs. The frame conversion breaks until task 2.1; record that in the handoff. Verify: `cargo check -p mbv-gui --lib` reports only the `frame/library/emby.rs` error.
- [x] 1.2 In the projection's test module, write one `#[case]` table over a played item (to `MediaSemanticState::Played`), a partly played item with a resumable position (to `Active`), a not-played item (to `Ordinary`), and a played music item (to `Ordinary`), for both `plain_row` and `feed_row` or through their public projection path, as the module's existing tests already reach them. The contract is the derivation seam (spec "An Emby library row carries the shared play state"). Do not re-test `from_emby`'s internals; `mbv-ui-model` owns those. Verify: `cargo nextest run -p mbv-gui emby_projection` passes.

## 2. The frame carries the state (design D2)

- [ ] 2.1 In `crates/mbv-gui/src/bridge/frame/library/emby.rs`, add `state: GuiHomeRowStateFrame` to `GuiEmbyRowFrame` and set it in `GuiEmbyRowFrame::from` through the existing `GuiHomeRowStateFrame::from(&MediaSemanticState)` conversion (design D2: no second frame state type). Update the struct doc. Fix every construction site in tests and fixtures the new field breaks. Verify: `cargo check -p mbv-gui` passes.
- [ ] 2.2 Run `cargo nextest run -p mbv-gui` in the foreground and fix every fixture the field breaks. No new test: the conversion is shared and already covered, and the derivation contract landed in task 1.2. Verify: the suite passes.

## 3. The row paints the state (design D3)

- [ ] 3.1 In `crates/mbv-gui/src/window/library/lettered/adapters.rs`, pass `play: Some(crate::window::components::play_mark::RowPlay::of_row(&item.row.state))` in `paint_emby_row`'s `FlatRow`. No other painter change: the flat row owns the whole look (design D3). Update the module doc. Verify: `cargo check -p mbv-gui` passes.
- [ ] 3.2 In `crates/mbv-gui/src/window/components/library_lettered_list/tests/emby.rs`, write one test: a played row paints `data-play-mark="played"` and the played title weight, and an ordinary row paints `data-play-mark="unplayed"`. The contract is the prop pass (spec "An Emby library row carries the shared play state"). Do not re-cover the mark's rendering or the flat row's look; `play_mark` and `list_row` own those. Verify: `cargo nextest run -p mbv-gui library_lettered_list` passes.

## 4. Gates

- [ ] 4.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv-gui`, each in the foreground, with no pipe. Verify: all three exit 0.

## Workflow follow-up

- The user does the live look check of the Emby library rows (a homevideos library, a played item, a partly played item, an unplayed item, and a fully played series or folder) in the GUI window before the PR opens.
- Sync the `gui-client` delta into `openspec/specs/` and archive the change only when the user approves the PR for merge, as the last commit before the merge.
