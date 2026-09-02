## 1. Contract and baseline

- [x] 1.1 Add `WideMediaList` and `InlineMediaBrowser` terminology to `CONTEXT.md` before implementation; use the exact names and distinguish Inline Search.
  - Also add `Emby podcast channel list` to `CONTEXT.md` alongside the control terms (`Emby homevideos feed view` is already defined); this term is coined by this slice.
- [x] 1.2 Record current TV Wide→Narrow→Wide cursor, selected target, scroll, and selected-row screen offset by source-reading and manual observation of the running app only; do not add or edit any test or fixture. The metadata-bearing characterization fixture is added later, after task 4.1's explicit user live visual approval (see tasks 4.1 and 4.2).
- [x] 1.3 Confirm the PR #606 stacking rule; this slice is a distinct PR targeting `feat/migrate-tui-to-tuirealm`. Mouse is out of scope and not a dependency: `restore-mouse-support` (#638) lands after every canonical slice.

## 2. Shared foundation

- [x] 2.1 Define provider-neutral `MediaListRow<Target>` item/heading/spacer model and semantic states, with stable opaque targets and bounded `0..=100` active progress.
- [x] 2.2 Re-home `render_plain_rows` into the canonical fixed-row render path without changing accepted generic Emby/TV output; retain semantic theme ownership.
- [x] 2.3 Implement embedded plain `WideMediaList<Target>` with selectable indexing, movement, clamping, viewport/scrollbar, fixed-height one-column geometry, and internal row geometry for painting and scrolling.
- [x] 2.4 Implement embedded plain `InlineMediaBrowser<Target>` with selected-row replacement, fit admission, fallback, anchor, and matching paint geometry; share only private mechanics with Wide.
- [x] 2.5 Implement `ViewportAnchor { selected_target, selected_row_offset }`; preserve target/offset across breakpoint transitions and clamp receiving geometry without shell mirrors.

## 3. Foundation destinations

- [x] 3.1 Split `src/app/components/browser.rs` ownership-preservingly before or with wiring; keep it at or below 800 lines.
- [x] 3.2 Split `src/app/components/tv_workspace.rs` ownership-preservingly before or with wiring; keep it at or below 800 lines.
- [x] 3.3 Compose `InlineMediaBrowser` for hero-bearing generic Emby catalog browsing, Movies, the Emby homevideos feed view, the Emby podcast channel list, and narrow TV Series browsing; leave non-hero two-column Emby catalogs on their existing two-column arrangement policy.
- [x] 3.4 Compose `WideMediaList` for the Wide TV right rail; preserve TV workspace, hero, pills, image handoff, effects, and parent message translation.
- [x] 3.5 Preserve non-hero two-column browsers and prove no second list painter runs at an applicable breakpoint.
- [x] 3.6 Verify this slice's wiring left parent-owned pills, workspaces, overlays, and central keyboard routing unchanged and added no mouse subscription, `MouseGestureState`, `HitRegions<Target>`, or parent-to-child point delegation; confirm the existing bespoke `*HitRegion` paths remain wired and untouched. `restore-mouse-support` (#638) lands after every canonical slice and owns all mouse work.

## 4. Visual-first evidence and gates

- [x] 4.1 Reproduce and correct visuals through live verification at Wide and Narrow widths, including selection, movement, focus, scrolling, images-enabled/disabled behaviour, and TV rail/Inline replacement; obtain explicit user confirmation before UI test edits.
- [ ] 4.2 After confirmation, add/update focused rendered characterization/composition tests with representative metadata, grouping, active progress, focus, breakpoint, and image fixtures; add one display-row/selectable-index test.
- [ ] 4.3 Record one-painter evidence for every migrated destination and manual/live evidence in the slice PR; do not accept metadata-free or inferred screenshot evidence.
- [ ] 4.4 Run `rtk make check-code-file-lines` and verify all changed source files are ≤800 lines.
- [ ] 4.5 Run `rtk openspec validate introduce-canonical-media-list-foundation --strict`.
- [ ] 4.6 Run `rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`, and the relevant `rtk cargo nextest run` suite; fix only failures caused by this slice.

## 5. Slice acceptance

- [ ] 5.1 Verify no registry identity, second router, callback/provider framework, global hit map, duplicate row coordinate path, protocol/provider/daemon/persistence change, or bespoke exception was introduced.
- [ ] 5.2 Attach rendered evidence, live user confirmation, characterization results, file-size and verification outputs; keep this slice independently reviewable/reversible and do not mark umbrella tasks complete.
