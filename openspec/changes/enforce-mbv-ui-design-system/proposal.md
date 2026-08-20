## Why

PR #552 extracted shared painters and colour roles, and #560–#562 began replacing
hero-on-top across library surfaces. That work exposed that the arrangement
boundary is convention-only: screens still bypass shared geometry, call raw
Ratatui APIs, duplicate hit-target arithmetic, and select raw palette values
independently.

Issue #584 is the prerequisite that completes the user-facing arrangement change:
every hero-bearing browse surface uses hero-on-left in wide mode and an inline
hero in narrow mode, with no hero-on-top convention. This change starts from that
settled baseline and enforces the ownership boundary so later screens cannot
regress it.

## What Changes

- Define a closed mbv UI design-system boundary for components, arrangements,
  semantic theme roles, and interaction geometry.
- Bring every independently rendered surface inside that boundary: each surface
  must use canonical design-system ownership or an explicitly identified bespoke
  component.
- Keep screen content extensible while making structural and visual variation use
  centrally defined typed policies or variants.
- Make arrangements own placement and composition. Interactive components derive
  and emit hit targets from the same internal geometry they paint; arrangements
  aggregate those targets for screens to consume.
- Treat bespoke rendering only as an exception from component reuse. Bespoke
  components still obey the ownership, semantic styling, hit-geometry, and test
  requirements.
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

- `right-panel-arrangements`: Tightens the arrangement-ownership boundary after
  #584 establishes hero-on-left wide and inline-hero narrow as the only hero
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
- Per-surface compliance audits are tracked as sub-issues of #563.

## Planning Status

The #584 prerequisite is complete. This proposal now starts from the settled
hero-on-left-wide / inline-narrow baseline and covers only the remaining
design-system ownership and enforcement work.

Decisions established in the current exploration:

- Completion requires whole-tree classification and enforcement, not pilot-only
  or changed-code-only compliance.
- A bespoke component is exempt only from reuse; it is not exempt from the design
  system's ownership, styling, interaction, or verification rules.
- Interactive components derive and emit targets from their painted geometry;
  arrangements place components and aggregate their typed targets; screens consume
  the aggregate map.

The next exploration session should begin by:

- Re-reading the resulting render tree and live UI specs as the baseline.
- Defining the exact surface, screen-model, arrangement, component, theme, and
  bespoke module boundaries against the real post-#584 code.
- Reconciling `design.md`, the delta specs, and `tasks.md` against that baseline.
- Choosing a whole-tree enforcement strategy and explicit approved-painting
  boundary that can pass against the existing tree when this change completes.
