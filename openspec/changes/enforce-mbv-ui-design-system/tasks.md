## 1. Establish The UI Boundary

- [ ] 1.1 Map every current render module and independently rendered surface into canonical screen, arrangement, component, or theme responsibilities without changing behaviour. Record the settled hero-on-left-wide / selected-row-replacement-narrow baseline, make the normative no-grandfathering scope explicit, and confirm that no current surface is classified as bespoke.
- [ ] 1.2 Define the approved distributed component and arrangement boundaries, plus the future bespoke component/arrangement path, and document which modules may perform direct Ratatui painting across the whole current render tree; screen modules only compose semantic data, require one authoritative owner per concern, and do not create a monolithic renderer.
- [ ] 1.3 Narrow theme access so screen code consumes semantic roles or component policies rather than raw palette primitives. This is a large cross-cutting diff (100+ call sites across `queue.rs`, `detail_series_view.rs`, `music_wide.rs`, `home_hero.rs`, `indicators.rs`, `album_rows.rs`, etc.); use the pilot surfaces to establish the API, then resolve or explicitly classify every remaining access rather than deferring it to a child migration issue.
- [ ] 1.4 Define typed, centrally owned policy and variant representations for the pilot arrangement, including the closed approved hero additional-content styles (Movie overview, TV seasons/pills and episodes, Music tracks), using private fields or sealed implementations where appropriate. Keep style data and interaction state separate from arrangement-owned geometry, and keep every override in the owning central module rather than in surface code.
- [ ] 1.5 Add the new domain terms (component, arrangement, bespoke surface, policy, variant) to `CONTEXT.md` under Presentation, per the repo term-coordination rule.
- [ ] 1.6 Record a complete current-surface-to-additional-content-style matrix for every hero-bearing surface, including provider-specific content and row policies; an unmapped surface or bespoke designation is a boundary violation, not deferred child-issue work.

## 2. Make Components Own Interaction

- [ ] 2.1 Define the typed hit-map/result contract used by interactive components.
- [ ] 2.2 Migrate the pilot interactive component so its painted layout and hit targets are produced from one geometry calculation.
- [ ] 2.3 Update the pilot screen and input handling to consume component hit targets instead of reconstructing coordinates.
- [ ] 2.4 Add focused Ratatui `TestBackend` coverage for the pilot component's visual states and hit-target geometry.

## 3. Establish The Closed Arrangement Model
- [ ] 3.3 Move the shared hero-on-left geometry and component composition behind the arrangement boundary while preserving current output.
- [ ] 3.4 Represent legitimate screen differences as typed content models or named central policies using the approved hero additional-content style family (including Movie overview, TV seasons/pills and episodes, Music tracks, and mapped provider-specific styles), not screen-local painter branches or newly invented styles. The arrangement continues to own geometry for every style.
- [ ] 3.5 Add focused Rust unit/buffer tests for narrow-width, focus-state, and selected/unselected pilot behavior; keep these tests as component verification, separate from the source-based enforcement checks.
- [ ] 3.6 Confirm that no current surface requires bespoke status. Define the future process for adding a central bespoke component/arrangement only when a concrete new no-reuse case exists, with its reason and verification coverage; do not use that path to exempt current surfaces.
- [ ] 3.7 Classify every remaining independently rendered surface and either route it through canonical ownership or register an explicit bespoke component/arrangement with its reason and verification; do not defer current-surface enforcement to a child migration issue.

## 4. Sync Stale Specs

- [ ] 4.1 Sync the live arrangement specs with the completed selected-row-replacement-narrow / hero-on-left-wide baseline.
- [ ] 4.2 Sync the `library-list-hero` and `ui-design-language` live specs with the tightened component-ownership and role-narrowing requirements this change adds.

## 5. Guide Agents And Developers

- [ ] 5.1 Add concise mandatory TUI architecture and controlled-override requirements to `AGENTS.md`; state explicitly that they are normative requirements, not conventions, and that existing surfaces are not grandfathered.
- [ ] 5.2 Add the committed `.opencode/skills/mbv-frontend/SKILL.md` with the reuse workflow, decision table, Ratatui patterns, and completion checklist.
- [ ] 5.3 Include examples distinguishing content changes (including approved hero additional-content styles), named policies, central variants, new components, and explicit bespoke surfaces; show that none of these permits screen-owned geometry.
- [ ] 5.4 Ensure the guidance requires checking component ownership, narrow-width behaviour, interaction targets, and focused buffer tests before completion.

## 6. Detect Common Bypasses

- [ ] 6.1 Add a practical code-based repository check (ast-grep rule scoped to screen modules, matching the repo's existing tool routing) for direct arrangement bypasses and unapproved painting outside approved UI modules; this source check, not unit-test assertions, enforces the boundary.
- [ ] 6.2 Add a check or lint for raw `palette::` primitive access in screen modules outside the theme layer.
- [ ] 6.3 Make exceptions explicit and reviewable rather than silently excluding whole render areas from the checks; require every exception or override to live in a named central component, arrangement, theme, or future bespoke component/arrangement.
- [ ] 6.4 Add a check or documented test convention for interactive components that do not expose their hit geometry.
- [ ] 6.5 Run the new checks against the whole existing tree and resolve every violation needed to establish the boundary or add an explicit documented exception; do not defer current-surface enforcement to a child migration issue.

## 7. Verify And Document Adoption

- [ ] 7.1 Run the mbv check, test, lint, and file-size commands (`rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace --all-targets`, `rtk make check-code-file-lines`) for the boundary, pilot migration, and new guidance/skill files.
- [ ] 7.2 Review the whole-tree implementation, including the pilot diff, for duplicated geometry, raw screen-level styles, independent hit-target calculations, and uncontrolled overrides.
- [ ] 7.3 Update the UI design-system documentation/spec references with the final module names, complete surface-to-style matrix, and approved exception process.
- [ ] 7.4 Validate the completed OpenSpec change and confirm every current surface is classified and enforced; child issues may audit compliance or record approved customisations and overrides, but do not carry deferred migration work.
