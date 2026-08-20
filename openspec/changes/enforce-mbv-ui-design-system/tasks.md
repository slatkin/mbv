> **Prerequisite:** [#584](https://github.com/slatkin/mbv/issues/584) must land
> before implementation of this change begins. Once it lands, rebase this plan
> on its all-hero-on-left wide / inline-hero narrow baseline and remove or revise
> tasks already completed by that prerequisite.

## 1. Establish The UI Boundary

- [ ] 1.1 Map the current render modules into screen, arrangement, component, theme, and bespoke responsibilities without changing behaviour. Record which surfaces still call `top_hero_layout` or `SelectedBlockBorderStyle::HeroOnTop` directly.
- [ ] 1.2 Define the approved component and arrangement module boundaries and document which modules may perform direct Ratatui painting.
- [ ] 1.3 Narrow theme access so screen code consumes semantic roles or component policies rather than raw palette primitives. This is a large cross-cutting diff (100+ call sites across `queue.rs`, `detail_series_view.rs`, `music_wide.rs`, `home_hero.rs`, `indicators.rs`, `album_rows.rs`, etc.); scope it to the pilot surfaces first and track the remainder explicitly rather than hedging with "wherever the existing migration allows."
- [ ] 1.4 Define typed, centrally owned policy and variant representations for the pilot arrangement, using private fields or sealed implementations where appropriate.
- [ ] 1.5 Add the new domain terms (component, arrangement, bespoke surface, policy, variant) to `CONTEXT.md` under Presentation, per the repo term-coordination rule.

## 2. Make Components Own Interaction

- [ ] 2.1 Define the typed hit-map/result contract used by interactive components.
- [ ] 2.2 Migrate the pilot interactive component so its painted layout and hit targets are produced from one geometry calculation.
- [ ] 2.3 Update the pilot screen and input handling to consume component hit targets instead of reconstructing coordinates.
- [ ] 2.4 Add focused Ratatui `TestBackend` coverage for the pilot component's visual states and hit-target geometry.

## 3. Eliminate Hero-On-Top And Pilot The Closed Arrangement Model

- [ ] 3.1 Migrate the remaining hero-on-top surfaces (`feeds`, `audiobookshelf`, `audiobookshelf_books`, `home_video`, `album`) to the shared inline-hero (narrow) or hero-on-left (wide) arrangement. Do not enshrine hero-on-top behind a boundary; eliminate it.
- [ ] 3.2 Remove `top_hero_layout` and `SelectedBlockBorderStyle::HeroOnTop` once no screen references them.
- [ ] 3.3 Move the shared hero-on-left geometry and component composition behind the arrangement boundary while preserving current output.
- [ ] 3.4 Represent legitimate screen differences as content models or named central policies, not screen-local painter branches.
- [ ] 3.5 Add narrow-width, focus-state, and selected/unselected visual regression coverage for the pilot arrangement.
- [ ] 3.6 Record any necessary bespoke surface explicitly with its reason, ownership, and verification coverage.

## 4. Sync Stale Specs

- [ ] 4.1 Sync `openspec/specs/right-panel-arrangements/spec.md`: the live spec still says "Narrow presentation SHALL be hero-on-top," contradicting the inline-hero behaviour shipped in #561. Replace with the inline-hero narrow presentation.
- [ ] 4.2 Sync the `library-list-hero` and `ui-design-language` live specs with the tightened component-ownership and role-narrowing requirements this change adds.

## 5. Guide Agents And Developers

- [ ] 5.1 Add concise mandatory TUI architecture and controlled-override rules to `AGENTS.md`.
- [ ] 5.2 Add the committed `.opencode/skills/mbv-frontend/SKILL.md` with the reuse workflow, decision table, Ratatui patterns, and completion checklist.
- [ ] 5.3 Include examples distinguishing content changes, named policies, central variants, new components, and explicit bespoke surfaces.
- [ ] 5.4 Ensure the guidance requires checking component ownership, narrow-width behaviour, interaction targets, and focused buffer tests before completion.

## 6. Detect Common Bypasses

- [ ] 6.1 Add a practical repository check (ast-grep rule scoped to screen modules, matching the repo's existing tool routing) for direct `top_hero_layout` / `SelectedBlockBorderStyle::HeroOnTop` / `f.render_widget` usage outside approved UI modules.
- [ ] 6.2 Add a check or lint for raw `palette::` primitive access in screen modules outside the theme layer.
- [ ] 6.3 Make exceptions explicit and reviewable rather than silently excluding whole render areas from the checks.
- [ ] 6.4 Add a check or documented test convention for interactive components that do not expose their hit geometry.
- [ ] 6.5 Run the new checks against the existing tree and record or resolve the initial violations needed to establish the boundary.

## 7. Verify And Document Adoption

- [ ] 7.1 Run the mbv check, test, lint, and file-size commands (`rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace --all-targets`, `rtk make check-code-file-lines`) for the boundary, pilot migration, and new guidance/skill files.
- [ ] 7.2 Review the pilot diff specifically for duplicated geometry, raw screen-level styles, independent hit-target calculations, and uncontrolled overrides.
- [ ] 7.3 Update the UI design-system documentation/spec references with the final module names and approved exception process.
- [ ] 7.4 Validate the completed OpenSpec change and confirm the next screen migrations can proceed under the new convention.
