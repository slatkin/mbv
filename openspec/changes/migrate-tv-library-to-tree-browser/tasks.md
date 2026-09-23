# Tasks

## 1. Shared Structural Rows

- [x] 1.1 Add optional structural heading/spacer entries to TreeBrowser's atomic reconciliation and settled row flow; verify focused component tests show skipped structural rows, stable selection, refresh, and unchanged heading-free Music behavior.
- [x] 1.2 Paint headings and between-group spacers through the shared TreeBrowser Render Component and retain accurate pointer/scroll geometry; verify focused buffer and completed-frame pointer tests at grouped and ungrouped widths.
- [x] 1.3 Extend the plain tree-node input with declared child expandability and allow `ToggleExpansion`/`ToggleExpansionTarget` on a declared-expandable node with no loaded children, retaining expansion through reconciliation without fake selectable nodes; verify focused shared-tree tests for pending expansion, empty completed children, and unchanged Music child-derived behavior.

## 2. TV Show Projection

- [x] 2.1 On a checkout containing `rework-tv-library-pills` code and synced specs, and after the missing `canonical-media-lists` delta is created, add stable Show/Season/Episode TV targets and project sorted show-mode rows, headings, and between-group spacers into one TreeBrowser; verify component tests for order, group boundaries, and stable refresh identities.
- [x] 2.2 Wire shell-owned show detail and episode loading to declared-expandable branches, deduplicating existing Hero fetches without eager all-season fetches or stale completion overwrites; verify mock-boundary tests for uncached expansion, loading, empty completion, out-of-order results, and selection continuity.
- [x] 2.3 Replace only the show-mode flat browser list with the TV tree in the Library Panel for every geometry; preserve the Hero's season pills and episode Workspace and verify focused TV content tests for concurrent inline and Workspace episode rows.

## 3. Input and Mode Boundaries

- [x] 3.1a Translate tree selection and expansion by stable TV target in Narrow and Wide; keep headings non-actionable and the Hero Workspace cursor independent. Verify focused component tests for show and season movement/expansion, including Wide navigation and Narrow visual-key handling.
- [x] 3.1b Translate show, season, and episode activation, context, and playback through typed shell requests using the selected tree target, not the flat carrier or Hero Workspace cursor. Verify focused tests for show Hero opening, season toggle, inline episode playback, and heading no-op.
- [x] 3.1c Restore show-mode Esc/Backspace navigation to `TvBack` in both geometries; verify one focused keyboard regression.
- [x] 3.1d Resolve show activation's Hero gate from the tree-selected show rather than the stale flat carrier; verify one focused shell regression.
- [x] 3.1e Stabilize and accept 3.1a–d: fix WIP whitespace, verify focused tree actions and Narrow visual-key handling, run compile/targeted tests, commit scoped code, and obtain focused review before checking these rows.
- [ ] 3.2a Keep Latest/Upcoming on the flat episode control with their current pill and Hero rules; verify mode switches and direct episode activation without opening a show Workspace.
- [ ] 3.2b Keep Inline Search on its separate flat result control without changing the show tree's settled selection or expansion; verify dismissal and selector restore across mode changes.
- [ ] 3.3a Exercise mounted `Application::tick()` through the shell sync pass for TV tree keyboard navigation and activation in Wide, Narrow, and Mini; verify resolved targets.
- [ ] 3.3b Exercise latest-frame mouse delivery through the Library Panel in Wide, Narrow, and Mini; verify the correct tree target and no second painter or underpaint.

## 4. Completion

- [ ] 4.1a Create the required `canonical-media-lists` delta replacing TV show-mode flat-browser obligations while preserving Latest/Upcoming and Hero Workspace flat rows; verify the delta against current behavior.
- [ ] 4.1b Sync affected main specs, including the already-landed TV pills deltas, add TV tree terminology to `CONTEXT.md`, and verify OpenSpec strict validation and cross-references.
- [ ] 4.2 Run `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, and `cargo clippy --workspace --all-targets -- -D warnings`; fix failures and verify a clean checkout after committing planning and implementation together.
