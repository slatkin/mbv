## 1. Current-Main Baseline

- [ ] 1.1 Start from current main including PR #514 and identify the existing mixed-tab keyboard, mouse, refresh, help, context-menu, and inert Audiobookshelf activation checks to preserve.
- [ ] 1.2 Extend existing semantic tests only where they do not already protect the known regressions: full-front-door Service isolation, mixed-position collisions, stale destination normalization, Feeds and Audiobookshelf F5 behavior, Audiobookshelf help classification, and inert episode play/enqueue. Avoid exact pixels, full buffers, full help strings, and context-menu ordering assertions.

## 2. Destination Identity And Position Mapping

- [ ] 2.1 Rename `TabSelection::Library` to `EmbyLibrary` and `library_index()` to `emby_library_index()`, retaining Home, Audiobookshelf, and Feeds as peer variants.
- [ ] 2.2 Delete `TabSelection::from_position` and `to_position`, remove their tests, and migrate every live caller to the count-aware mixed-destination mappings.
- [ ] 2.3 Add `App::normalize_stale_browse_destination() -> bool` to normalize a stale Emby or Audiobookshelf library index to Home and make the triggering destination-specific operation stop when it returns `true`.
- [ ] 2.4 Verify unique count-aware mixed-tab positions and keyboard/mouse round trips for zero, one, and multiple libraries from each Remote Service; preserve startup on Home rather than adding saved-tab restoration.
- [ ] 2.5 Keep `App::apply_tab_position` as the activation owner for direct, next, and previous tab selection, including Service activation, focus, image-dimension reset, visibility, and preference saving.

## 3. Keyboard And Refresh Dispatch

- [ ] 3.1 Replace guard-ordered browse keyboard routing with one exhaustive destination match at `App::handle_key_view_dispatch`, without adding a `CONTEXT_STACK` entry and with shared `handle_global_view_key` behavior preceding destination handling.
- [ ] 3.2 Give Home, Emby, Audiobookshelf, and Feeds Service-specific keyboard handlers that preserve current navigation and unsupported-action behavior while sharing only global commands.
- [ ] 3.3 Match F5 by panel and destination so Home reloads Home, Emby reloads the matched library, Audiobookshelf restarts its catalog request after clearing state, Feeds invokes feed refresh, and Queue refreshes only the visible queue.
- [ ] 3.4 Verify Audiobookshelf and Feeds keys cannot enter Emby search, selection, watched-state, shuffle, playlist, route, rescan, or context-menu paths.

## 4. Explicit Emby Action Targets

- [ ] 4.1 Pass the matched Emby library index through `move_lib_cursor_rows`, `move_lib_cursor`, `jump_lib_cursor`, `current_lib_item`, `select`, and `go_back` instead of recovering it from `App::tab`.
- [ ] 4.2 Pass the matched Emby library index through `handle_key_lib_search`, `refresh_lib`, `shuffle_play`, `toggle_watched`, `enqueue_selected`, `context_menu_lib_idx`, `open_context_menu`, and their routing-reachable Service-specific callees.
- [ ] 4.3 Rename only parameterized helpers whose remaining generic name still conceals an Emby-only input or behavior; audit the named chains rather than broadly renaming unrelated helpers.
- [ ] 4.4 Remove every missing-index-to-Emby-library-zero fallback from browse actions and verify stale or mismatched destinations produce no cross-library mutation.

## 5. Mouse And Render Dispatch

- [ ] 5.1 Split left-panel scroll, selector, single-click, double-click, and right-click entry points by exhaustive destination before interpreting Service-local geometry.
- [ ] 5.2 Add `LayoutMain::browse_destination: Option<TabSelection>`, preserve fresh `AppLayout` replacement per completed frame, tag only the installed completed layout, and no-op browse mouse input unless the tag matches the normalized selected destination.
- [ ] 5.3 Match browse rendering exhaustively and handle stale destination state without a final default-to-Emby branch.
- [ ] 5.4 Audit the Home, Feeds, Audiobookshelf, and Emby render-published fields listed in `design.md` and verify mixed-tab navigation, selection, scrolling, inert episode activation, and unsupported right-clicks cannot consume another Service's layout state.

## 6. Destination-Valid Surfaces

- [ ] 6.1 Classify help by exhaustive destination; put the library-focused destination first, list the exact Home and Audiobookshelf key sets specified by the capability, and put Queue first without changing the retained browse destination when queue-focused.
- [ ] 6.2 Build context menus from the matched panel and destination, preserving supported Home and Emby `EmbyItem` menus while leaving Audiobookshelf, Feeds, non-Emby queue items, and absent/stale targets without an Emby menu.
- [ ] 6.3 Audit browse action entry points to confirm Service-native catalog types remain separate and only explicit supported play/enqueue actions can construct QueueItems.
- [ ] 6.4 Establish inert `activate_audiobookshelf_episode(index)` and `enqueue_audiobookshelf_episode(index)` seams for keyboard and mouse callers, preserving selection and all queue/playback/Service state for #518 to activate later.

## 7. Verification

- [ ] 7.1 Run focused tab-selection, input-dispatch, mouse, refresh, help, context-menu, Feeds, and Audiobookshelf nextest suites.
- [ ] 7.2 Run `cargo check -p mbv`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `make check-code-file-lines`, strict OpenSpec validation, and diff checks.
- [ ] 7.3 Manually verify keyboard and mouse behavior across Home, multiple Emby libraries, multiple Audiobookshelf libraries, Feeds, queue focus, stale Service removal, Home startup, and Audiobookshelf show-position restoration.
- [ ] 7.4 Audit the completed change to confirm it adds no common browse-item model, `mbv-core` browse abstraction, QueueItem variant, Audiobookshelf playback, credential flow, persistence migration, or ctrl change.
