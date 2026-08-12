## 1. Current-Main Baseline

- [ ] 1.1 Start from current main including PR #514 and identify the existing mixed-tab keyboard, mouse, refresh, help, context-menu, and inert Audiobookshelf activation checks to preserve.
- [ ] 1.2 Add focused characterization coverage for Home, Emby, Audiobookshelf, and Feeds destination isolation, including unsupported actions leaving other destination and queue state unchanged.

## 2. Destination Identity And Position Mapping

- [ ] 2.1 Rename the Emby tab variant and accessor so provider ownership is explicit at every call site, retaining Home, Audiobookshelf, and Feeds as peer variants.
- [ ] 2.2 Remove count-unaware tab-position conversion and migrate every caller to the mixed-destination mapping that receives current Emby, Audiobookshelf, and Feeds counts.
- [ ] 2.3 Normalize or safely ignore stale Emby and Audiobookshelf destination indexes after Service lifecycle changes without substituting another destination.
- [ ] 2.4 Verify unique mixed-tab positions and keyboard, mouse, save, and restore round trips for zero, one, and multiple libraries from each Remote Service.

## 3. Keyboard And Refresh Dispatch

- [ ] 3.1 Replace guard-ordered browse keyboard routing with one exhaustive destination match beneath the existing global input precedence stack.
- [ ] 3.2 Give Home, Emby, Audiobookshelf, and Feeds provider-specific keyboard handlers that preserve current navigation and unsupported-action behavior while sharing only global commands.
- [ ] 3.3 Match F5 refresh by panel and destination so Home, the selected Emby library, Audiobookshelf, Feeds, and the visible queue invoke only their own refresh behavior.
- [ ] 3.4 Verify Audiobookshelf and Feeds keys cannot enter Emby search, selection, watched-state, shuffle, playlist, route, rescan, or context-menu paths.

## 4. Explicit Emby Action Targets

- [ ] 4.1 Pass the matched Emby library index through cursor movement, paging, first/last navigation, selection, and back-navigation helpers instead of recovering it from the active tab.
- [ ] 4.2 Pass the matched Emby library index through search, refresh, shuffle, watched-state, enqueue, and provider-specific context-action helpers.
- [ ] 4.3 Rename generically named routing-reachable helpers where necessary to expose their Emby-only input or behavior.
- [ ] 4.4 Remove every missing-index-to-Emby-library-zero fallback from browse actions and verify stale or mismatched destinations produce no cross-library mutation.

## 5. Mouse And Render Dispatch

- [ ] 5.1 Split left-panel scroll, selector, single-click, double-click, and right-click entry points by exhaustive destination before interpreting provider-local geometry.
- [ ] 5.2 Keep Home, Emby, Audiobookshelf, and Feeds hit testing provider-local and ensure each renderer clears or overwrites the transient maps and rectangles its mouse handler consumes.
- [ ] 5.3 Match browse rendering exhaustively and handle stale destination state without a final default-to-Emby branch.
- [ ] 5.4 Verify mixed-tab mouse navigation, podcast show and episode selection, selector clicks, scrolling, inert episode activation, and unsupported right-clicks do not change another provider's state.

## 6. Destination-Valid Surfaces

- [ ] 6.1 Classify help by exhaustive destination and add Audiobookshelf read-only navigation help without advertising Emby-only actions.
- [ ] 6.2 Build context menus from the matched panel and destination, preserving Home, Emby, and queue behavior while leaving unsupported Audiobookshelf and Feeds rows without an Emby menu.
- [ ] 6.3 Audit browse action entry points to confirm provider-native catalog types remain separate and only explicit supported play/enqueue actions can construct QueueItems.
- [ ] 6.4 Preserve the provider-specific inert Audiobookshelf episode activation seam required by `play-audiobookshelf-podcasts-locally`.

## 7. Verification

- [ ] 7.1 Run focused tab-selection, input-dispatch, mouse, refresh, help, context-menu, Feeds, and Audiobookshelf nextest suites.
- [ ] 7.2 Run `cargo check -p mbv`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `make check-code-file-lines`, strict OpenSpec validation, and diff checks.
- [ ] 7.3 Manually verify keyboard and mouse behavior across Home, multiple Emby libraries, multiple Audiobookshelf libraries, Feeds, queue focus, stale Service removal, and saved-tab restoration.
- [ ] 7.4 Audit the completed change to confirm it adds no common browse-item model, `mbv-core` browse abstraction, QueueItem variant, Audiobookshelf playback, credential flow, persistence migration, or ctrl change.
