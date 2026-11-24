# Tasks

## 1. Shared title-tone derivation

- [ ] 1.1 In `crates/mbv-gui/src/window/components/list_row.rs`, add
      `pub fn played_title_tone(play: Option<&GuiHomeRowStateFrame>) -> String`
      (design D1) and make `RowLevel::title_tone` delegate its `Some` case to
      it, keeping the leaf-muted default only for the `None` case. Verify
      `cargo nextest run -p mbv-gui list_row` and `cargo fmt` pass; no behaviour
      changes, so no new test.

## 2. Workspace table and destinations

- [ ] 2.1 In `crates/mbv-gui/src/window/components/library_workspace_table.rs`,
      change `WorkspaceRow.state` to `Option<GuiHomeRowStateFrame>`, delete the
      `progress` field, `WorkspaceRowState`, and `name_color`, add the
      module-private state-hook wire function and the playing-row heading
      override (design D2, D3), and derive the resume run from the frame
      through `episode_progress`. Update `library_workspace_table/tests.rs`:
      drop the deleted binding from the fixtures and add the one derivation
      contract test (design D5). Record for the next unit: the crate does not
      compile until 2.2 lands, because the heroes still name the deleted type.
      Verify `cargo check -p mbv-gui` reaches only the two hero files' errors.
- [ ] 2.2 In `library_shows_hero.rs` and `library_feeds_hero.rs`, delete the
      private `row_state` conversions and the unused
      `episode_progress`/`is_played_episode` imports and pass the carried frame
      (`episode.state.clone()` / `row.state.clone()`) with no percent. In
      `library_tracklist.rs` and `library_audiobooks_hero.rs`, drop the
      deleted fields' bindings. Verify
      `cargo nextest run -p mbv-gui library_workspace_table library_shows_hero library_feeds_hero`
      and `cargo clippy -p mbv-gui --all-targets -- -D warnings` pass, and
      `cargo fmt` is clean.
