## Context

PR #552 extracted shared painters and colour roles. The completed #584 migration
now provides the settled hero-on-left-wide / inline-narrow baseline across the
hero-bearing surfaces. The remaining design-system problem is ownership:
screens still bypass shared geometry, call raw Ratatui APIs, duplicate hit-target
arithmetic, and select raw palette values directly.

The UI is a single Rust application using Ratatui 0.30.2 and `TestBackend`; no
separate UI crate or plugin system exists. The existing arrangement specs are
`right-panel-arrangements`, `library-list-hero`, and `ui-design-language`; this
change extends and tightens them rather than superseding them.

## Goals / Non-Goals

**Goals:**

- Preserve the completed inline / hero-on-left arrangement while enforcing its
  ownership boundaries.
- Keep the live arrangement specs aligned with the shipped inline-narrow
  presentation.
- Make canonical components and arrangements the owners of geometry, painting,
  styling, and interaction geometry.
- Allow screen-specific content and a closed vocabulary of typed visual policies.
- Make bespoke rendering explicit rather than an accidental local copy.
- Give agents precise repository rules and a repeatable UI workflow.
- Add lightweight mechanical checks and focused buffer/layout tests.
- Add the new domain terms (component, arrangement, bespoke surface, policy,
  variant) to `CONTEXT.md` per the repo term-coordination rule.

**Non-Goals:**

- Reimplement every existing screen in one migration.
- Make mbv a general-purpose UI framework or support third-party UI plugins.
- Change user-facing visual design as part of the boundary work alone.
- Move application state or provider logic into the UI layer.
- Supersede the existing `right-panel-arrangements`, `library-list-hero`, or
  `ui-design-language` specs; this change extends and tightens them.

## Decisions

### Closed structural vocabulary, extensible content

Arrangements, components, and structural visual variants are closed and centrally
defined. Screen models remain extensible for titles, metadata, rows, images, and
other semantic content. This preserves consistency without forcing every content
difference into the central component.

An enum or sealed trait is preferred for a small closed variant set. Policy
constructors should expose named valid combinations rather than public booleans
that permit invalid combinations. A registration-based extension trait was
rejected because it would allow arbitrary Ratatui painting and would restore the
convention-only failure mode.

### Components own hit targets

Interactive components will derive hit targets from their layout and return or
publish a typed hit map alongside their render result. Screens will consume those
targets rather than repeating coordinate arithmetic. Ratatui's `Widget` and
`StatefulWidget` remain useful painting primitives, but they do not provide this
ownership themselves, so hit-target output is an mbv design-system responsibility.

### Semantic theme API

Components consume semantic theme roles or component style policies. Raw palette
primitives remain implementation details of the theme layer wherever practical.
Screens should not pass arbitrary `Color` or `Style` values into shared components.
This retains the current role-based direction while making the API narrower and
more difficult to bypass.

### Layered module boundary

The UI will be organised conceptually as:

```text
screen models -> arrangements -> components -> Ratatui
                         \-> hit maps
```

Module visibility, private theme primitives, typed component APIs, and explicit
bespoke modules provide the first boundary. A separate crate is not required for
the initial migration because it would add coupling and move code without solving
all same-crate import bypasses.

### Guidance plus mechanical detection

`AGENTS.md` will contain concise mandatory rules. The `mbv-frontend` skill will
contain the decision workflow, examples, and completion checklist. These are
complemented by focused source checks or lint configuration for common forbidden
patterns and by Ratatui buffer/layout tests. Documentation alone is insufficient;
mechanical checks alone cannot judge every legitimate exception.

### Incremental migration

The work will establish the boundary with the hero-on-left arrangement and one
interactive component before migrating every remaining surface. The remaining
hero-on-top surfaces (`feeds`, `audiobookshelf`, `audiobookshelf_books`,
`home_video`, `album`) are migrated incrementally behind the shared arrangement
boundary; the `top_hero_layout` function and `SelectedBlockBorderStyle::HeroOnTop`
variant are removed once no screen references them. New UI work must follow the
boundary immediately after the guidance and initial checks land.

## Risks / Trade-offs

- [Risk] The closed vocabulary can make legitimate new UI work feel centralised
  in one module. -> Keep content models extensible and make named policy/variant
  additions small and well-tested.
- [Risk] Source checks may flag legitimate low-level component code. -> Scope
  checks to screen modules and maintain an explicit approved UI boundary.
- [Risk] Existing render modules contain mixed screen and component logic. -> Use
  incremental migration and do not require an all-at-once rewrite.
- [Risk] Agent skills can be ignored or unavailable in another environment. -> Put
  non-negotiable rules in `AGENTS.md` and keep the skill as workflow guidance.
- [Risk] Ratatui's buffer escape hatch permits bypasses inside the crate. -> Keep
  direct buffer access inside approved component/arrangement modules and make
  exceptions explicit in review.
