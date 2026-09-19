## 1. Dependency and artist identity

- [ ] 1.1 Add the locked `tui-treelistview` dependency without its optional keymap, create the destination-specific tree module boundary outside `music_content.rs`, and compile a minimal model/state/widget adapter against the exact 0.2.2 API; verify with `cargo check -p mbv` and confirm `music_content.rs` remains below 800 lines.
- [ ] 1.2 Extend Emby item parsing to retain applicable `ArtistItems` name/ID pairs without changing serialized queue compatibility, and add focused parsing cases for present, missing, and equal-name/different-ID artists; verify with the smallest relevant `cargo nextest run -p mbv-core` filter.
- [ ] 1.3 Carry stable artist identity through `GroupedAlbumCatalog`, define deterministic fallback grouping keys, and protect equal-name separation plus fallback stability with pure grouping cases; verify with the smallest relevant `cargo nextest run -p mbv` filter.
- [ ] 1.4 Add the hermetic `mbv-core` client operation for `ArtistIds=<ArtistItems id>&IncludeItemTypes=Audio&Recursive=true`, never accepting IDs sourced from an `/Artists` listing, and verify query construction and parsed tracks with focused mock-HTTP `cargo nextest run -p mbv-core` cases.
- [ ] 1.5 Before starting section 2, manually inspect one configured live Emby Service to confirm an album-level response requested through the existing fields path carries `ArtistItems` and that the task 1.4 `ArtistIds` query returns that artist's Audio tracks; record the Service version and result as manual evidence, and do not replace this gate with a live automated test.

## 2. Stable tree owner and Library panel composition

- [ ] 2.1 Implement the destination-local node arena and shallow artist-root/album-leaf model with monotonic non-reused IDs, settled ordering, atomic model revision, and node-to-domain translation; verify focused model cases for refresh retention, deletion fallback, equal names, and destination reset with `cargo nextest run -p mbv`.
- [ ] 2.2 Implement one tree state owner for selected node, expansion, viewport, marks, and geometry reconciliation across settled refresh and responsive geometry changes; emit album-selection persistence only when the resolved selected album changes, and never overwrite album persistence when an artist root receives focus; verify selected-node, expansion, multi-selection, viewport continuity, and the artist-focus persistence guard with component-level `cargo nextest run -p mbv` cases.
- [ ] 2.3 Integrate the tree as the Grouped Music browser implementation of the existing Library panel list slot while keeping album-track and artist-track Workspaces on `MediaList`; remove the parallel album carrier/flat projection and verify Wide, Narrow, and Mini mounted composition has one browser owner and one painter with targeted `cargo nextest run -p mbv` integration cases.
- [ ] 2.4 Map visible-node movement, Home/End, viewport paging, parent/child movement, and Enter expansion/album activation through `MusicContent` without enabling the crate keymap or changing central precedence; verify focused component behavior and routing-matrix rows with `cargo nextest run -p mbv`.

## 3. Tree rendering and pointer geometry

- [ ] 3.1 Implement artist and album label/column rendering through the crate's supported renderer and column interfaces, using semantic theme roles for indentation, expand/collapse glyphs, year metadata, aggregate marks, and scrollbar; verify focused buffer cases at representative Wide and narrow widths with `cargo nextest run -p mbv`.
- [ ] 3.2 Reproduce the selected-row bar, focus/unfocus treatment, group-relative zebra behavior, truncation, and focused-title marquee through supported tree style/render seams only; verify painter-owned buffer cases and title-clock reset behavior without snapshots or sleeps using `cargo nextest run -p mbv`.
- [ ] 3.3 Retain and invalidate only latest-completed-render tree geometry, translate click/double-click/wheel/right-click gestures inside `MusicContent`, and remove any album-list geometry compatibility path; verify stale-frame rejection and current-frame artist/album hit resolution through component and mounted mouse cases with `cargo nextest run -p mbv`.

## 4. Artist actions and multi-selection

- [ ] 4.1 Resolve focused artist play, enqueue, shuffle, and context intents to ordered album targets, independent of expansion and restricted to matching leaves while filtered; verify collapsed, expanded, filtered, and equal-name artist cases emit album identities only with `cargo nextest run -p mbv`.
- [ ] 4.2 Implement tree Visual mode and modified-click selection so artist toggles affect visible descendant albums, roots derive tri-state marks, and ranges skip roots; verify ordered membership, Partial/Marked state, filtering, and Queue-selection isolation with focused `cargo nextest run -p mbv` cases.
- [ ] 4.3 Connect tree selection summaries and context-origin identity to the existing status and bulk-action paths, preserving capability intersection and clear-only-origin behavior; verify one mounted status/action integration case with `cargo nextest run -p mbv`.

## 5. In-place fuzzy filtering

- [ ] 5.1 Add the Grouped Music filter session using `SkimMatcherV2`, the existing 300 ms duration, composite artist/title/year text, ancestor retention, forced filter expansion, and settled-order projection; verify debounce by injected instants and cover match, no-match, empty-query, and stable-order cases with `cargo nextest run -p mbv`.
- [ ] 5.2 Reuse the Library panel's one-row Inline Search bar for Grouped Music without constructing a flat result carrier or dispatching a full-library fetch, and restore the pre-filter node, persistent expansion, and visible-only multi-selection on dismissal; verify key precedence, responsive continuity, no-fetch behavior, and restoration with component and mounted `cargo nextest run -p mbv` cases.

## 6. Artist Hero and track Workspace

- [ ] 6.1 Build the shell-owned artist-detail cache and request identity on the verified task 1.4 client operation, keyed by Library destination, Service setup generation, `ArtistItems` ID, and settled revision; verify cache reuse and stale completion rejection with hermetic `cargo nextest run -p mbv` cases.
- [ ] 6.2 Fetch artist artwork through the existing image/cache boundary and project immediate artist summary facts from in-scope albums; verify stable-ID artwork requests, fallback no-artwork behavior, filtered album count, and year span with focused `cargo nextest run -p mbv` cases.
- [ ] 6.3 Project cached artist tracks only for in-scope album IDs into a canonical `MediaList` Workspace grouped by settled album order and sorted by disc/track order, falling back to aggregation of existing per-album track fetches when no artist ID exists; verify duplicate titles, multi-disc order, filter re-projection without refetch, and fallback grouping with `cargo nextest run -p mbv`.
- [ ] 6.4 Extend shared Hero/Workspace content so artist focus and album focus switch atomically in Wide and Library Hero overlay presentations, preserve existing album track behavior, and prevent stale artist/album tracks painting under a new title; verify focused component plus shell tick integration cases with `cargo nextest run -p mbv`.

## 7. Contract cleanup and acceptance

- [ ] 7.1 Update or remove superseded Grouped Music flat-list/search tests, retain the canonical Heading/page-navigation cases for other destinations, and confirm no duplicate assertion or Grouped Music dependency remains in `group-aware-page-navigation`; verify the targeted `mbv` test families pass.
- [ ] 7.2 Add the precise focusable Music artist-root term to `CONTEXT.md` while preserving `Group heading` as the non-selectable canonical-list term, and verify proposal, design, specs, code names, and glossary use the distinction consistently by review.
- [ ] 7.3 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv-core`, `cargo nextest run -p mbv`, and a targeted `cargo llvm-cov` review of the new model/filter/artist-completion paths; fix regressions and record any proven pre-existing failures.
- [ ] 7.4 Present the completed slice for live user review in representative Wide, Narrow, Mini, and Library Hero overlay states, including long names, narrow clipping, expansion, filtering, marks, scrollbar, artist/album switching, and focused/unfocused treatments; accept only on explicit user satisfaction, otherwise remove the dependency and all tree-specific implementation rather than adding a bespoke parallel renderer.
