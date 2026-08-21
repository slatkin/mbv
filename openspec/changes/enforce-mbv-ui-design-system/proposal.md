## Why

PR #552 extracted shared painters and colour roles, and #560–#562 began replacing
separate detail blocks across library surfaces. That work exposed that the arrangement
boundary is convention-only: screens still bypass shared geometry, call raw
Ratatui APIs, duplicate hit-target arithmetic, and select raw palette values
independently.

Issue #584 is the prerequisite that completes the user-facing arrangement change:
every hero-bearing browse surface uses hero-on-left in wide mode and selected-row
replacement in narrow mode. This change starts from that
settled baseline and enforces the ownership boundary so later screens cannot
regress it.

## What Changes

- Define a closed mbv UI design-system boundary for components, arrangements,
  semantic theme roles, and interaction geometry.
- Bring every independently rendered surface in the current render tree inside
  that boundary: each surface must use canonical design-system ownership or an
  explicitly identified bespoke component.
- Start with no bespoke surfaces. Every current surface must be classified under
  canonical screen, arrangement, component, or theme ownership; the bespoke path
  is defined for a future surface only when a concrete no-reuse case exists.
- Require whole-tree classification and enforcement before this change closes.
  These are normative requirements, not conventions or optional guidance, and
  there is no grandfathering for existing surfaces. Existing visual output may
  remain unchanged, but no surface may remain an unclassified or informal
  exception. Visual redesigns are separate work; child issues record
  per-surface audits and approved customisations or overrides rather than
  deferred migrations.
- Keep screen content extensible while making structural and visual variation use
  centrally defined typed policies or variants. Any override lives in the central
  component, arrangement, or theme implementation; a surface may only select a
  named option and supply semantic data.
- Preserve the existing hero additional-content styles, including the Movie
  overview/detail block, the TV season/pill and episode workspace, and the Music
  track-list workspace, along with the other provider-specific styles already in
  use. Require a complete current-surface-to-style mapping. Screens supply
  provider data and row semantics to these approved styles; they do not invent a
  new style or own its geometry or responsive layout.
- Make arrangements own placement and composition. Interactive components derive
  and emit hit targets from the same internal geometry they paint; arrangements
  aggregate those targets for screens to consume.
- Treat bespoke rendering only as an exception from component reuse. Bespoke
  components still obey the ownership, semantic styling, hit-geometry, and test
  requirements, and their overrides live in the central bespoke component or
  arrangement rather than in surface code.
- Add repository-level UI rules to `AGENTS.md`.
- Add a committed `mbv-frontend` agent skill describing the UI workflow, reuse
  rules, controlled override decision table, and verification expectations.
- Add practical source/mechanical checks that make common bypasses visible during
  development and review. Focused unit and buffer tests verify component behavior,
  but test assertions are not the design-system enforcement mechanism.

## Capabilities

### New Capabilities

- `ui-design-system`: Defines the mbv TUI component, arrangement, theme, variant,
  interaction, and development-guidance contract.

### Modified Capabilities

- `right-panel-arrangements`: Tightens the arrangement-ownership boundary after
  #584 establishes hero-on-left wide and selected-row replacement narrow as the only hero
  arrangement.
- `library-list-hero`: Extends the hero-ownership and hit-target model so
  components derive their hit geometry and arrangements aggregate it.
- `ui-design-language`: Narrows raw-palette access so screen code consumes
  semantic roles or component style policies, completing the role-layer narrowing
  the prior change intended but did not finish.

## Impact

- `src/app/render/`: whole-tree component and arrangement boundaries, semantic
  theme access, and hit-target ownership.
- `src/app/`: screen-to-component integration and rendering tests.
- `CONTEXT.md`: new domain terms (component, arrangement, bespoke surface,
  policy, variant) added under Presentation per the repo term-coordination rule.
- `AGENTS.md`: mandatory guidance for UI changes.
- `.opencode/skills/mbv-frontend/`: committed agent workflow and review guidance.
- Existing UI capability specs: tighten ownership requirements against the
  post-#584 arrangement baseline.
- Repository checks and test commands used to detect direct screen painting,
  arbitrary styling, and missing component-level visual contracts.
- No runtime protocol, persisted-data, or user-facing media behaviour changes are
  intended.

## Tracking

- Parent issue: https://github.com/slatkin/mbv/issues/563
- Prerequisite: https://github.com/slatkin/mbv/issues/584 must land before this
  change begins.
- Child issues under #563 are limited to per-surface compliance audits and
  formal records for approved customisations or overrides. They are not
  migration tickets and cannot defer bringing a current surface inside the
  enforced boundary.

## Planning Status

The #584 prerequisite is complete. This proposal now starts from the settled
hero-on-left-wide / selected-row-replacement-narrow baseline and covers only the remaining
design-system ownership and enforcement work.

Decisions established in the current exploration:

- Completion requires whole-tree classification and enforcement, not pilot-only
  or changed-code-only compliance. This is a normative requirement with no
  grandfathering, not a convention. A pilot implementation may establish the
  APIs, but it does not limit the compliance scope.
- A bespoke component is exempt only from reuse; it is not exempt from the design
  system's ownership, styling, interaction, or verification rules.
- Interactive components derive and emit targets from their painted geometry;
  arrangements place components and aggregate their typed targets; screens consume
  the aggregate map.
- The ownership migration preserves existing visual output unless separate visual
  work explicitly changes it; child issues do not defer current-surface
  classification or enforcement.

The next exploration session should begin by:

- Re-reading the resulting render tree and live UI specs as the baseline.
- Defining the exact surface, screen-model, arrangement, component, and theme
  boundaries against the real post-#584 code, while documenting only the future
  path for adding a bespoke component or arrangement.
- Reconciling `design.md`, the delta specs, and `tasks.md` against that baseline.
- Choosing a whole-tree enforcement strategy and explicit approved-painting
  boundary that can pass against the existing tree when this change completes.
