# Tasks

## 1. Shared Structural Rows

- [x] 1.1 Add optional structural heading/spacer entries to TreeBrowser's atomic reconciliation and settled row flow; verify focused component tests show skipped structural rows, stable selection, refresh, and unchanged heading-free Music behavior.
- [x] 1.2 Paint headings and between-group spacers through the shared TreeBrowser Render Component and retain accurate pointer/scroll geometry; verify focused buffer and completed-frame pointer tests at grouped and ungrouped widths.
- [x] 1.3 Extend the plain tree-node input with declared child expandability and allow `ToggleExpansion`/`ToggleExpansionTarget` on a declared-expandable node with no loaded children, retaining expansion through reconciliation without fake selectable nodes; verify focused shared-tree tests for pending expansion, empty completed children, and unchanged Music child-derived behavior.

## 2. TV Show Projection

- [x] 2.1 On a checkout containing `rework-tv-library-pills` code and synced specs, and after the missing `canonical-media-lists` delta is created, add stable Show/Season/Episode TV targets and project sorted show-mode rows, headings, and between-group spacers into one TreeBrowser; verify component tests for order, group boundaries, and stable refresh identities.
- [x] 2.2 Wire shell-owned show detail and episode loading to declared-expandable branches, deduplicating existing Hero fetches without eager all-season fetches or stale completion overwrites; verify mock-boundary tests for uncached expansion, loading, empty completion, out-of-order results, and selection continuity.
- [ ] 2.3 Replace only the show-mode flat browser list with the TV tree in the Library Panel for every geometry; preserve the Hero's season pills and episode Workspace and verify focused TV content tests for concurrent inline and Workspace episode rows.

## 3. Input and Mode Boundaries

- [ ] 3.1 Translate tree selection, expansion, activation, context, and playback intents by stable TV target through existing typed shell requests where possible; verify component tests for show, season, episode, and non-actionable headings.
- [ ] 3.2 Keep Latest/Upcoming and Inline Search on their flat controls, with current pill and Hero rules; verify mode-switch, search-dismissal, direct episode activation, and selector restore tests.
- [ ] 3.3 Exercise mounted `Application::tick()` for tree keyboard and latest-frame mouse delivery through the Library Panel in Wide, Narrow, and Mini; verify correct target and no second painter or underpaint.

## 4. Completion

- [ ] 4.1 Sync all affected main specs, including the prerequisite TV pills deltas and the required `canonical-media-lists` delta, add relevant terminology to `CONTEXT.md`, and verify OpenSpec strict validation and cross-references.
- [ ] 4.2 Run `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, and `cargo clippy --workspace --all-targets -- -D warnings`; fix failures and verify a clean checkout after committing planning and implementation together.
