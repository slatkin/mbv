## 1. Catalog-driven mount reconciliation (D2)

- [ ] 1.1 Add `Model::live_destination_keys(&self) -> HashSet<ComponentId>` (or
      a `Vec` + `contains` on `library_id`) in a new
      `src/app/shell_destination_mounts.rs` (keeps `shell.rs` under the
      800-line cap). It enumerates `app.libs` and `app.audiobookshelf_libraries`
      and emits, per library, every destination `ComponentId::Browser(BrowserKey{..})`
      that library could produce — `Generic`/`Movies`/`HomeVideos`/`TvShows`/
      `Music` for Emby (keyed by `library.library.id`, independent of current
      view mode), `AudiobookshelfBook`/`AudiobookshelfPodcast` for ABS. Verify:
      `rtk cargo check -p mbv`.
- [ ] 1.2 Add `Model::reconcile_destination_mounts(&mut self)` in the same file:
      for every mounted `ComponentId::Browser(key)` and
      `ComponentId::InlineSearch(key)`, if no live library has
      `library_id == key.library_id`, `umount` it and set any
      `emby_browser_id`/`tv_workspace_id`/`music_workspace_id`/`abs_book_id`/
      `abs_podcast_id`/inline-search pointer still equal to it to `None`.
      Iterate a snapshot of mounted ids (collect first) to avoid mutating
      `application` while borrowing it. Verify: unit test in the new file —
      mount two Emby browsers, drop one library from `app.libs`, call
      `reconcile_destination_mounts`, assert only the dropped one is unmounted
      and its pointer is cleared.
- [ ] 1.3 Call `self.reconcile_destination_mounts()` once per tick in
      `src/app/shell_run.rs`, immediately before `self.sync_active_destination()`
      (currently line ~442) and after the `sync_*` mount calls. Verify:
      `rtk cargo nextest run -p mbv` full suite still green.

## 2. Emby browser: stop unmounting on switch (D1)

- [ ] 2.1 In `src/app/shell_browser.rs::mount_emby_browser`, remove the
      `if let Some(id) = self.emby_browser_id.take() { umount }` block and the
      `self.application.active(&id)` call. New body: if `next_id` differs from
      `self.emby_browser_id`, mount `next_id` only when
      `!self.application.mounted(&id)`, then `self.emby_browser_id = next_id`
      (which may be `None`), then `self.push_emby_browser_content()` when
      `next_id` is `Some`. Verify: `rtk cargo check -p mbv`.
- [ ] 2.2 Test in `src/app/shell_browser_tests.rs`: build a two-Emby-library
      app, enter library A, move the browser cursor down N rows, switch to
      library B, switch back to A, assert the `BrowserComponent` for A's
      `BrowserKey` is still `mounted()` and its `cursor()` is N (not 0). Verify:
      `rtk cargo nextest run -p mbv emby_browser`.
- [ ] 2.3 Test: switch away from library A, mutate A's item list in `app.libs`,
      switch back, assert the first `render_emby_browser_component` frame
      reflects the new items (content refresh on re-point, D1 + risk
      mitigation). Verify: `rtk cargo nextest run -p mbv emby_browser`.

## 3. TV / Music / ABS workspaces: same treatment (D1)

- [ ] 3.1 Apply the task-2.1 transformation to `sync_tv_workspace`
      (`src/app/shell_tv_workspace.rs:94-108`): drop `umount` + `active`, mount
      lazily via `mounted()` guard, keep `push_tv_workspace_content()` on
      re-point. Verify: `rtk cargo nextest run -p mbv tv_workspace`; add a
      wide→narrow→wide test asserting `TvWorkspaceComponent` stays mounted and
      its pane/cursor state is preserved across the resize.
- [ ] 3.2 Same for `sync_music_workspace` (`src/app/shell_music_workspace.rs:49-64`).
      Add a test: view a music library's album folders, move the album cursor,
      drill into a track list (`is_viewing_album_folders` → false, pointer →
      `None`), go back, assert the `MusicWorkspaceComponent` is still mounted
      and the album cursor is where it was. Verify:
      `rtk cargo nextest run -p mbv music_workspace`.
- [ ] 3.3 Same for `sync_audiobookshelf_book` and `sync_audiobookshelf_podcast`
      (`shell_audiobookshelf_book.rs:105-...`, `shell_audiobookshelf_podcast.rs:111-...`).
      Verify: `rtk cargo nextest run -p mbv abs_book` and
      `rtk cargo nextest run -p mbv abs_podcast`; add a switch-and-return
      state-preservation test for each.
- [ ] 3.4 Test: no two destination components share a `library_id` after a
      music library is visited in both album-folder and generic views, and
      after retirement both are gone. Verify:
      `rtk cargo nextest run -p mbv`.

## 4. Single focus pass (D3)

- [ ] 4.1 Confirm `sync_active_destination` (`src/app/shell_library.rs:23-41`)
      is now the only site calling `application.active()` for a destination
      child. `rg -n "\.active\(" src/app/shell_*.rs` should show it only in
      `sync_active_destination`, `sync_queue`, and the overlay/modal open
      paths — not in any `sync_<workspace>` / `mount_emby_browser`. Verify:
      the grep result matches that expectation.
- [ ] 4.2 Extend the existing focus tests (`shell_library.rs` `#[cfg(test)]`
      module, plus any `library_parent` / queue-focus tests) to assert: on the
      first tick after startup with no prior `active()` call in `sync_*`, focus
      lands on the active destination child; and after dismissing an overlay,
      focus returns to the active destination child, not a stale
      lazily-mounted sibling. Verify: `rtk cargo nextest run -p mbv` (focus /
      library / queue test names) green.
- [ ] 4.3 Test: a mounted-but-inactive destination component paints nothing
      (its `render_*_component` early-returns because the `*_id` pointer is
      `None`). Verify: `rtk cargo nextest run -p mbv`.

## 5. Ledger + final gate

- [ ] 5.1 Update the Notes cells of the destination rows in
      `docs/architecture/interactive-surface-ledger.md` (Emby browser, TV
      workspace, Music workspace, ABS podcast, ABS book, inline album-track)
      to record the keep-mounted lifetime: component stays mounted while its
      Service library is in the catalog; the `Model` `*_id` field is an
      active-destination pointer; `reconcile_destination_mounts` retires
      components for removed libraries. Verify: `git diff` touches only those
      rows' Notes cells.
- [ ] 5.2 `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`,
      `rtk cargo fmt --all -- --check`, `rtk make check-code-file-lines` —
      all green.
- [ ] 5.3 `openspec validate keep-destination-components-mounted --strict` passes.
