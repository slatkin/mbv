# Todo: Selectable Artist Headers

- [x] Run GitNexus impact analysis for the required symbol list before editing.
- [x] Add narrowly scoped custom music artist-header selection state using a revalidated identity.
- [x] Extend `build_grouped_album_display_plan`; do not create a parallel grouping helper.
- [x] Add typed power-left row targets so artist headers are distinct from album rows.
- [x] Gate selectable artist headers strictly to `is_music_group_view` / `render_power_music_group_view`.
- [x] Update render styling for selected artist headers only in the custom music-group view.
- [x] Update keyboard Up/Down/Enter behavior; header Enter must be consumed before album-track focus.
- [x] Update mouse row hit testing so custom music artist headers are selectable.
- [x] Resolve current display-plan albums under the selected artist header.
- [x] Add explicit header-aware context/direct actions and a single multi-album bulk helper for enqueue, play all, and shuffle.
- [x] Add focused tests for render, keyboard, mouse, context actions, non-custom regressions, and partial loaded-member resolution.
- [x] Run targeted tests.
- [x] Run full relevant tests.
- [x] Run GitNexus `detect_changes` before committing.
- [ ] Commit, push branch, and open PR for issue #210.
