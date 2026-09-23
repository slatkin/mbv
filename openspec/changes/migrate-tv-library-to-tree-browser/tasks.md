# Tasks

## 1. Shared Structural Rows

- [ ] 1.1 Add optional structural heading projection to TreeBrowser's atomic reconciliation and settled row flow; verify focused component tests show skipped headings, stable selection, refresh, and unchanged heading-free Music behavior.
- [ ] 1.2 Paint headings through the shared TreeBrowser Render Component and retain accurate pointer/scroll geometry; verify focused buffer and completed-frame pointer tests at grouped and ungrouped widths.

## 2. TV Show Projection

- [ ] 2.1 On a checkout containing `rework-tv-library-pills`, add stable Show/Season/Episode TV targets and project sorted show-mode rows and group headings into one TreeBrowser; verify component tests for order, group boundaries, and stable refresh identities.
- [ ] 2.2 Wire shell-owned show detail and episode loading to expanded branches without eager all-season fetches or stale completion overwrites; verify mock-boundary tests for loading, out-of-order results, expansion, and selection continuity.
- [ ] 2.3 Replace only the show-mode flat browser list with the TV tree in the Library Panel for every geometry; preserve the Hero's season pills and episode Workspace and verify focused TV content tests for concurrent inline and Workspace episode rows.

## 3. Input and Mode Boundaries

- [ ] 3.1 Translate tree selection, expansion, activation, context, and playback intents by stable TV target through existing typed shell requests where possible; verify component tests for show, season, episode, and non-actionable headings.
- [ ] 3.2 Keep Latest/Upcoming and Inline Search on their flat controls, with current pill and Hero rules; verify mode-switch, search-dismissal, direct episode activation, and selector restore tests.
- [ ] 3.3 Exercise mounted `Application::tick()` for tree keyboard and latest-frame mouse delivery through the Library Panel in Wide, Narrow, and Mini; verify correct target and no second painter or underpaint.

## 4. Completion

- [ ] 4.1 Update the shared-tree and TV specs' main copies from these deltas, add relevant terminology to `CONTEXT.md`, and verify OpenSpec strict validation and cross-references.
- [ ] 4.2 Run `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, and `cargo clippy --workspace --all-targets -- -D warnings`; fix failures and verify a clean checkout after committing planning and implementation together.
