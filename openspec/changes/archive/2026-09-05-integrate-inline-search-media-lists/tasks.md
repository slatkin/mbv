## 1. Shared Inline Search control

- [x] 1.1 Rebase onto the completed `replace-wide-paint-inference` change, confirm no Inline Search path selects presentation from last-frame paint geometry, and verify with `cargo check -p mbv`.
- [x] 1.2 Convert `src/app/components/inline_search.rs` from a mounted `AppComponent` into an embedded `InlineSearch` control plus the minimal `InlineSearchHost` contract; retain plain and recursive-album pools, store scored result indices, support empty and one-character queries, preserve stable selection on pool replacement, and adapt the existing Inline Search tests to verify query ordering, cursor/page movement, activation identity, and empty-query Backspace dismissal with `cargo nextest run -p mbv inline_library_search`.
- [x] 1.3 Add the shared three-row search arrangement and bordered input/results Render Component, including the too-short fallback and column-aware result geometry; replace the existing brittle render assertion with one buffer test that proves exact input/list placement and verify with `cargo nextest run -p mbv inline_library_search`.

## 2. Destination ownership

- [x] 2.1 Embed Inline Search in `BrowserComponent`, give the active search first refusal for keyboard and mouse events, and paint it at the Browser-owned list composition point for Normal catalogs and Hero-on-left right rails; adapt Browser tests to verify `/`, a shortcut letter entered as query text, and one Wide right-rail placement with `cargo nextest run -p mbv emby_browser`.
- [x] 2.2 Embed the same control in `MusicWorkspaceComponent`, suppress grouped album rows only in the search list area, retain the Hero/track pane in Wide presentation, and verify flat scored results with no artist headers plus dismissal restoring the prior album position using `cargo nextest run -p mbv music_workspace inline_library_search`.
- [x] 2.3 Embed the same control in `TvWorkspaceComponent`, suppress the ordinary series rail only in the search list area, retain the episode/Hero pane in Wide presentation, and verify the input and result rows paint in the right rail rather than the left pane using `cargo nextest run -p mbv tv_workspace inline_library_search`.

## 3. Shell lifecycle and responsive transfer

- [x] 3.1 Replace mounted-search lookup with one active-host adapter for open, candidate-pool/loading pushes, text-entry snapshot derivation, and activation dispatch; retain full-library fetch, recursive album-index, stale-completion, and navigation-effect guards, then adapt the existing shell loading/recursive activation tests and verify with `cargo nextest run -p mbv inline_library_search recursive_album_search album_index`.
- [x] 3.2 Extend the existing TV Normal/Wide active-destination handoff with a one-shot `InlineSearchTransfer` keyed by stable selected target and viewport row offset; add one real `Application::tick()` resize test that crosses Normal→Wide→Normal and proves the query, selected target, visibility, sole painter, and active destination survive, then verify it with `cargo nextest run -p mbv inline_search_survives_tv_responsive_transition`.
- [x] 3.3 Update Keyboard Router snapshot construction and mouse eligibility so the active destination remains the sole event boundary during search; strengthen an existing tick test to prove printable global/list shortcut letters reach the search while ordinary destination mutation is suppressed, and verify with `cargo nextest run -p mbv tests_tick_integration`.

## 4. Remove the obsolete overlay protocol

- [x] 4.1 Delete `ComponentId::InlineSearch`, separate mount/focus/render and area-selection functions, `set_wide`, Browser-only query projection, the draw-frame overlay pass, obsolete dismissal/projection state, and mount-reconciliation tests; verify there are no remaining mounted-search or two-painter references with `rg "InlineSearchComponent|ComponentId::InlineSearch|inline_search_area|set_wide|render_inline_search_component|project_inline_search_active" src/app` returning no matches and `cargo check -p mbv` passing.
- [x] 4.2 Remove or consolidate tests that only assert deleted getters, mount IDs, or overlay plumbing while preserving stronger behavior/integration coverage, and verify the focused suites with `cargo nextest run -p mbv inline_library_search emby_browser music_workspace tv_workspace`.

## 5. Record architecture and verify

- [x] 5.1 Add ADR 0025 for destination-embedded Inline Search, mark ADR 0022's separate-component clause as superseded, correct the Inline Search definition in `CONTEXT.md`, and update the Browser/Music/TV and Inline Search entries in `docs/architecture/interactive-surface-ledger.md`; verify the documents consistently name one owner and one painter per presentation with `rg "Inline Search|InlineSearch" CONTEXT.md docs/adr docs/architecture/interactive-surface-ledger.md`.
- [x] 5.2 Run `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `ast-grep scan`, `make check-code-file-lines`, and `cargo fmt --all -- --check`; fix every failure before marking the change complete. (`cargo nextest` is waived by explicit user directive: no tests. The three `check-code-file-lines` failures predate this section at accepted `d99379cf` and are not changed here.)
