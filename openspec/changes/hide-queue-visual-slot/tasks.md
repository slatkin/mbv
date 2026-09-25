# Tasks

## 1. State, persistence, and keybinding

- [x] 1.1 Add `visual_slot_hidden: bool` to `App` (`src/app/state/app_struct.rs`). In
  `construct.rs` read it from prefs (missing or non-bool → `false`), and write it in
  `save_prefs` (`src/app/input.rs`). Verify with a prefs round-trip unit test next to
  `list_pane_width_prefs_round_trip_width_and_null`: `true` saved then loaded is `true`, and a
  missing key loads `false`.
- [x] 1.2 Declare `hide_visual_slot` in `KEYBIND_ACTIONS` (`crates/mbv-core/src/keybinds/registry.rs`,
  section `Playback`, default `["h"]`, gate `NoBlockingOverlay`, rebindable, prefix-addressable).
  Update the table's count doc comment. Verify: `cargo nextest run -p mbv-core keybinds` passes,
  including the registry's own consistency tests.
- [x] 1.3 Add a `KeyPolicyEntry` named `hide_visual_slot` next to `visualizer`
  (`src/app/input/key_policy.rs`; `global: true`, `NoBlockingOverlay`) with a new
  `KeyPolicyBinding` → a new `Command::ToggleVisualSlotHidden` (`src/app/dispatch/action.rs`,
  exhaustive arm, no wildcard). The command flips the flag, calls `sync_visualizer`, and calls
  `save_prefs`. Verify with router unit tests:
  - `h` resolves to the command with no overlay open.
  - `h` does not fire while a text entry owns focus.
  - `h` resolves to the command, not the leaf's message, while the Feeds content component has
    focus.

## 2. Collapse and paint

- [x] 2.1 Add one `App` predicate, "visual slot shown" = not `Idle` and not `visual_slot_hidden`.
  Use it in `Model::sync_queue_card_geometry` (publish `CardGeometry::default()` when not shown)
  and at the slot paint in `Model::render_queue_playback_panel` (skip
  `render_queue_playback_slot`; keep the transport). Leave `last_card_*` untouched. Verify with
  unit tests at both breakpoints (<100 and ≥100 columns) with playback active and hidden:
  - The published card geometry is zero.
  - The Queue placement begins directly below the header + transport + separator rows.
  - Showing the slot again restores the previous card geometry.
- [x] 2.2 In `queue_playback_transport_area` (`src/app/render/arrangements/chrome.rs`) apply
  `SLOT_TRANSPORT_GAP` only when `card_width > 0`. Verify with a unit test: at the wide
  breakpoint with `card_width = 0` the transport's `x` equals the slot region's `x` and its width
  equals the region width. The existing arrangement tests must still pass.

## 3. Visualizer and artwork

- [x] 3.1 `toggle_visualizer` returns immediately while hidden (no selection change, no prefs
  write). `visualizer_should_run` adds `!visual_slot_hidden`. Verify with unit tests in
  `src/app/infra/visualizer.rs`:
  - `v` while hidden leaves `visualizer_enabled` unchanged.
  - A hidden slot makes `visualizer_should_run` false with the visualizer selected and playback
    active.
- [x] 3.2 Add the hidden check next to the active check that gates `refresh_queue_card_image`
  in `src/app/shell/queue.rs`. Verify with a unit test: with playback active and the slot hidden
  the projection pass issues no card fetch, and after un-hiding it does.

## 4. Gates

- [x] 4.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo nextest run -p mbv -p mbv-core`. All must pass.
- [x] 4.2 Manual check in the running app on a short terminal: `h` hides and shows the slot at
  both breakpoints, the transport stays usable by mouse, `v` does nothing while hidden, and the
  hidden state survives a restart.
