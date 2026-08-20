## Why

PR #552 extracted shared painters and colour roles, and #560–#562 began replacing
hero-on-top with inline-hero (narrow) and hero-on-left (wide) across library
surfaces. That migration exposed that the arrangement boundary is convention-only:
screens still bypass shared geometry, call raw Ratatui APIs, duplicate
hit-target arithmetic, and select raw palette values independently. Several
surfaces (`feeds`, `audiobookshelf`, `audiobookshelf_books`, `home_video`,
`album`) still use `top_hero_layout` / `SelectedBlockBorderStyle::HeroOnTop`
directly instead of going through the shared arrangement. Finishing and enforcing
that boundary requires typed component ownership, repository guidance, and
mechanical detection before more screens regress.

## What Changes

- Finish the hero-on-top elimination: migrate the remaining surfaces
  (`feeds`, `audiobookshelf` books/podcasts, `home_video`, `album`) to the shared
  inline-hero (narrow) or hero-on-left (wide) arrangement so no screen calls
  `top_hero_layout` or `SelectedBlockBorderStyle::HeroOnTop` directly.
- Define a closed mbv UI design-system boundary for components, arrangements,
  semantic theme roles, and interaction geometry.
- Keep screen content extensible while making structural and visual variation use
  centrally defined typed policies or variants.
- Make components own layout, painting, styling, and hit targets together.
- Sync the stale `right-panel-arrangements` spec: the live spec still says
  "Narrow presentation SHALL be hero-on-top," contradicting the inline-hero change
  in #561. Bring the live spec in line with the shipped narrow-inline behaviour.
- Add repository-level UI rules to `AGENTS.md`.
- Add a committed `mbv-frontend` agent skill describing the UI workflow, reuse
  rules, controlled override decision table, and verification expectations.
- Add practical mechanical checks and focused tests that make common bypasses
  visible during development and review.

## Capabilities

### New Capabilities

- `ui-design-system`: Defines the mbv TUI component, arrangement, theme, variant,
  interaction, and development-guidance contract.

### Modified Capabilities

- `right-panel-arrangements`: The live spec's narrow-presentation requirement is
  stale (still says hero-on-top). This change syncs it to the shipped
  inline-hero behaviour and tightens the arrangement-ownership boundary so
  screens do not call `top_hero_layout` or `SelectedBlockBorderStyle` directly.
- `library-list-hero`: Extends the hero-ownership and hit-target model so
  components own hit geometry and the remaining hero-on-top surfaces are folded
  into the shared inline-hero / hero-on-left arrangements.
- `ui-design-language`: Narrows raw-palette access so screen code consumes
  semantic roles or component style policies, completing the role-layer narrowing
  the prior change intended but did not finish.

## Impact

- `src/app/render/`: component and arrangement boundaries, semantic theme access,
  hit-target ownership, and elimination of direct `top_hero_layout` /
  `HeroOnTop` usage in `feeds`, `audiobookshelf`, `audiobookshelf_books`,
  `home_video`, `album`, and `list`.
- `src/app/`: screen-to-component integration and rendering tests.
- `CONTEXT.md`: new domain terms (component, arrangement, bespoke surface,
  policy, variant) added under Presentation per the repo term-coordination rule.
- `AGENTS.md`: mandatory guidance for UI changes.
- `.opencode/skills/mbv-frontend/`: committed agent workflow and review guidance.
- `openspec/specs/right-panel-arrangements/spec.md`: sync the stale
  narrow-presentation requirement with the shipped inline-hero behaviour.
- Repository checks and test commands used to detect direct screen painting,
  arbitrary styling, and missing component-level visual contracts.
- No runtime protocol, persisted-data, or user-facing media behaviour changes are
  intended.

## Tracking

- Parent issue: https://github.com/slatkin/mbv/issues/563
- Per-surface compliance audits are tracked as sub-issues of #563.
