# Tasks

## 1. Launch-state model and storage

- [ ] 1.1 Add the versioned `TuiLaunchState`, stable tab/selector/item identity types, state path, tolerant loader, and process-unique atomic replacement writer in `mbv-core`; add focused filesystem tests proving round-trip, malformed/missing fallback, and two distinct temporary paths, then run `cargo nextest run -p mbv-core config`.
- [ ] 1.2 Add `TUI launch state` to `CONTEXT.md` with its exit-snapshot meaning and exclusions, and verify terminology agrees with `Client`, `Session continuity`, `Library position`, and the three delta specs by direct review.

## 2. Selected-destination snapshot extraction

- [ ] 2.1 Add the bounded read-only launch-state query to the Library destination/content-owner boundary and implement stable main-Selector and item identities for Home plus generic Emby/TV/Music destinations; extend the narrowest existing component tests to prove current identities and absence when no pill/item exists, then run the matching `cargo nextest run -p mbv` test filters.
- [ ] 2.2 Implement the same bounded query for Feeds, Audiobookshelf podcast, and Audiobookshelf book destinations, using Feed/show/book identities rather than labels or row indices; extend the narrowest existing component tests for filter/group/show/bucket and selected-item extraction, then run the matching `cargo nextest run -p mbv` test filters.
- [ ] 2.3 Assemble exactly the selected tab's destination snapshot during orderly teardown, include Panel focus, exclude Queue selection and every unselected destination, and verify with an App/shell teardown test that reads the saved file without constructing live externals.

## 3. Hierarchical startup restoration

- [ ] 3.1 Load one pending launch intent at startup, resolve stable tab identity after the live catalog arrives, and cancel or consume that level on explicit user tab movement; verify existing-tab restoration and missing-tab fallback to the first guaranteed tab through the existing startup/catalog test seam.
- [ ] 3.2 Add explicit discrete re-anchor operations for each destination's main Selector and selected library item, resolving pill before item and consuming pending state so later refreshes cannot replay it; verify existing identities, missing-pill first-pill fallback, missing-item first-selectable fallback, no-pill scope, and empty-list selection with focused component tests.
- [ ] 3.3 Restore Library/Queue Panel focus after the selected destination is ready without passing a Queue target; add one real `Application::tick()` integration test proving Queue receives focus while its selected item follows normal Queue initialization, then run that test filter.

## 4. Exit-only lifecycle and legacy migration

- [ ] 4.1 Remove launch-state writes from live tab, Panel-focus, pill, item, refresh, and idle-flush paths; remove obsolete per-library dirty/flush state while preserving unrelated preference, configuration, queue, progress, cache, and auto-reconnect writes; verify a mounted tick test can change tab/pill/item/focus without changing the launch-state file before teardown.
- [ ] 4.2 When the new file is absent, derive at most one initial snapshot from legacy selected-tab/Panel-focus preferences and the selected tab's recoverable stable browse identity; do not infer identities from stale cursor indices or retain unselected-tab state, and verify new-file precedence plus legacy/malformed fallback in deterministic migration tests.
- [ ] 4.3 Add a two-App hermetic persistence test proving independent in-memory divergence and whole-snapshot last-exit-wins behavior, with no sleeps, live daemon, real state directory, locking, merge, or Client identity.

## 5. Verification

- [ ] 5.1 Run `cargo fmt`, `cargo nextest run -p mbv-core`, and `cargo nextest run -p mbv`; fix all failures without broadening the saved snapshot.
- [ ] 5.2 Run `cargo clippy --workspace --all-targets -- -D warnings`, `openspec validate persist-tui-launch-state-on-exit --strict`, and review the final diff to confirm no Queue selection, nested Workspace selector, scroll offset, unselected-tab state, live component mirror, new dependency, or ctrl/daemon protocol change was introduced.
