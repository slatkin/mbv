## Why

`TvWorkspaceComponent` already owns the browse cursor for the TV workspace,
but `Model::mirror_tv_workspace_cursor` (`src/app/shell_tv_workspace.rs:59`)
still resolves the component's selected item id back into
`BrowseLevel.cursor` on `App`, and is called twice per typed TV request —
once before the `App` effect and once after `push_tv_workspace_content()`
(`shell_tv_workspace.rs:30` and `:42`, for `TvMoveRows`, `TvMoveColumn`,
`TvJumpCursor`, `TvActivate`, `TvBack`, `TvCycleLetterPill`). Calling the same
writer twice around one request is the tell that two authorities (component
cursor, `App` cursor) are being forced to agree instead of one owning the
value. This is slice 2 of 4 under #611 ("Remove two-way interaction-state
mirrors"), following the pattern already established for the Emby browser's
item-targeted `ShellRequest::Browser*` arms (`src/app/shell_browser.rs:17-110`).

## What Changes

- Give `App::activate_selected_series` an item-targeted entry point (the
  established `Browser*` shape) so `ShellRequest::TvActivate` can carry the
  component-resolved series item instead of `App` re-reading it via
  `BrowseLevel.cursor`. This requires threading the resolved `EmbyItem`
  through `TvWorkspaceComponent`'s `Key::Enter` arm into a new
  `ShellRequest::TvActivate { item }` payload.
- Confirm and document (via characterization tests) that `App::go_back` does
  not read the current level's cursor for its pop decision or its parent-cursor
  restoration — the restoration keys off the popped level's own `parent_id`,
  which is shell-owned navigation memory, not a copy of the component's
  current selection. No production code change is expected here beyond
  removing the now-unneeded mirror call ahead of it; if characterization
  proves otherwise, that is a blocking discovery result per D17, not
  permission to redesign `go_back`.
- Confirm and document that `App::cycle_letter_pill` does not read
  `BrowseLevel.cursor` at all (it reads `letter_filter`) — remove the mirror
  call ahead of it with no production change to the method.
- Delete `mirror_tv_workspace_cursor` and both of its call sites in
  `handle_tv_request` once no TV effect re-reads the level cursor.
- Extend `typed_tv_requests_keep_component_cursor_authoritative` and
  `tv_episode_activation_uses_component_cursors_and_cached_season_id`
  (`shell_tv_workspace.rs:200`, `:229`) to assert the mirror is gone and that
  `App.libs[..].nav_stack[..].cursor` is untouched by `TvMoveRows`,
  `TvMoveColumn`, `TvJumpCursor`, `TvCycleLetterPill`, and `TvBack` (except
  for `go_back`'s own sanctioned parent-cursor restoration).
- Season and episode cursors (`shell_tv_workspace.rs:44-45`) are already
  component-local and are out of scope; this change does not touch them.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is an internal ownership refactor inside the TuiRealm interactive
boundary (ADR 0022); per D17 in `openspec/changes/migrate-tui-to-tuirealm/design.md`,
current observable behaviour is the parity authority for this teardown slice,
so no user-visible or spec-level behavior changes. `skip_specs: true` is set
in `.openspec.yaml`.

## Impact

- `src/app/shell_tv_workspace.rs`: remove `mirror_tv_workspace_cursor` and
  both call sites in `handle_tv_request`; extend the two named tests.
- `src/app/components/tv_workspace.rs`: resolve and attach the selected
  series `EmbyItem` to the `TvActivate` request on `Key::Enter`.
- `src/app/components/msg/shell.rs`: `ShellRequest::TvActivate` gains an
  `item: EmbyItem` field (**BREAKING** for any other caller constructing this
  variant, though none exist outside `tv_workspace.rs`/`shell_tv_workspace.rs`).
- `src/app/input_browse_dispatch.rs`: `App::activate_selected_series` gains a
  new item-targeted entry point alongside the existing
  `activate_selected_series(lib_idx)` (cursor-resolving) method — the latter
  stays because it is still called by mouse activation
  (`src/app/mouse_gestures.rs:166,234`), and mouse is accepted-broken for the
  alpha (D16) and out of scope for this change. Only the typed-request path
  (`TvActivate`) moves to the item-targeted method.
- `src/app/actions_navigation.rs` (`go_back`) and `src/app/music_actions.rs`
  (`cycle_letter_pill`): no expected signature or behavior change; touched
  only by characterization evidence gathered during implementation.
- Verification: `rtk cargo check -p mbv`, `rtk cargo nextest run`,
  `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`.
