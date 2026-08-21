## Context

PR #552 extracted shared painters and colour roles. The completed #584 migration
now provides the settled hero-on-left-wide / selected-row-replacement-narrow baseline across the
hero-bearing surfaces. The remaining design-system problem is ownership:
screens still bypass shared geometry, call raw Ratatui APIs, duplicate hit-target
arithmetic, and select raw palette values directly.

The UI is a single Rust application using Ratatui 0.30.2 and `TestBackend`; no
separate UI crate or plugin system exists. The existing arrangement specs are
`right-panel-arrangements`, `library-list-hero`, and `ui-design-language`; this
change extends and tightens them rather than superseding them.

## Goals / Non-Goals

**Goals:**

- Preserve the completed selected-row replacement / hero-on-left arrangement while enforcing its
  ownership boundaries.
- Keep the live arrangement specs aligned with the shipped selected-row-replacement-narrow
  presentation.
- Make canonical components and arrangements the owners of geometry, painting,
  styling, and interaction geometry.
- Allow screen-specific content and a closed vocabulary of typed visual policies.
- Make bespoke rendering explicit rather than an accidental local copy.
- Give agents precise repository rules and a repeatable UI workflow.
- Add lightweight mechanical checks. Retain focused buffer/layout tests as
  supporting component verification, not as the architecture enforcement
  mechanism.
- Add the new domain terms (component, arrangement, bespoke surface, policy,
  variant) to `CONTEXT.md` per the repo term-coordination rule.

**Non-Goals:**

- Redesign every existing screen's visual presentation in one migration. This
  change does require ownership classification and enforcement for every current
  surface while preserving its output unless separate visual work changes it.
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

Screens may select a centrally named policy or variant and provide semantic data,
but they may not implement an override. Every structural, styling, geometry, or
interaction override belongs in the central component, arrangement, theme, or
future bespoke component/arrangement that owns it.

### Hero additional-content styles do not bypass geometry requirements

The hero has a closed family of approved additional-content styles already
represented by the existing surfaces. This family includes the Movie
overview/detail block, the TV season/pill and episode workspace, the Music
track-list workspace, and the other provider-specific styles already in use.
Provider data and row semantics remain extensible inside a style, and existing
preview or focusable child states remain valid, but screens do not invent another
additional-content style. A genuinely new style or override is a central
design-system change implemented by its owning component or arrangement, not a
screen-local exception.

The implementation must produce a complete matrix mapping every current
hero-bearing surface to one approved style (and any centrally defined content or
row policy it uses). The matrix is part of enforcement: an unmapped surface is a
violation, not a future child-issue migration.

The initial matrix contains no bespoke surfaces. Every existing render surface is
classified under canonical screen, arrangement, component, or theme ownership.
Bespoke rendering is a future extension path and may be introduced only when a
concrete new surface cannot reuse the canonical vocabulary and its central owner,
reason, and verification are defined at that time.

Screens provide the data and interaction state for an approved style; they do not
provide layout rectangles, row arithmetic, spacing, breakpoints, or renderer
callbacks.

The arrangement owns the geometry for every mode, including pane placement,
available-height budgeting, image/text stacking, optional-block placement,
responsive presentation, and aggregation of child hit targets. A mode may change
which semantic child content exists or whether that child content can receive
focus, but it does not create a screen-local geometry exception. Read-only modes
remain inert; focusable modes use the shared focus and hit-target contracts.

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

Module visibility, private theme primitives, typed component APIs, and the explicit
future bespoke component/arrangement path provide the first boundary. A separate
crate is not required for the initial migration because it would add coupling and
move code without solving all same-crate import bypasses.

Centralisation means one authoritative owner and vocabulary for each geometry,
style, interaction, and bespoke concern; it does not mean one monolithic renderer.
Arrangements, components, theme modules, and any future named bespoke
component/arrangement remain distributed and independently testable, while screen
modules only compose them and provide semantic data. Screen modules do not call
Ratatui, construct layout rectangles, or calculate hit targets.

### Guidance plus mechanical detection

`AGENTS.md` will contain concise mandatory rules. The `mbv-frontend` skill will
contain the decision workflow, examples, and completion checklist. These documents
describe and reinforce the normative requirements; they do not turn them into
optional conventions or define a grandfathering path. Source checks or lint
configuration enforce common forbidden patterns and ownership boundaries. Ratatui
buffer/layout and unit tests verify component behavior, but their assertions are
supporting verification rather than the conformance mechanism. Documentation alone
is insufficient; mechanical checks alone cannot judge every legitimate exception.

### Whole-tree enforcement with representative implementation

`#563` closes only after the whole current render tree has been classified and
brought inside the ownership boundary. Every independently rendered surface is
either routed through canonical screen, arrangement, component, and theme
modules or is an explicitly named bespoke surface with a documented reason,
ownership, semantic styling, hit geometry, and focused verification. No surface
may remain an informal exception. These are mandatory requirements with no
grandfathering, not conventions that a screen may choose to follow.

An approved customisation is therefore a named central implementation decision,
not permission for a surface to paint locally. Child issues may document and audit
that decision, but they cannot grant a surface-local override.

The hero-on-left arrangement and one interactive component may serve as the
representative implementation used to establish the APIs, but that pilot does
not limit the compliance scope. Existing visual output is preserved; visual
redesigns are separate work. Child issues record audits and approved
customisations or overrides against this enforced tree and must not defer
classification, ownership, or enforcement for a current surface. New UI work
must follow the boundary immediately after the guidance and initial checks land.

## Risks / Trade-offs

- [Risk] The closed vocabulary can make legitimate new UI work feel centralised
  in one module. -> Keep content models extensible and make named policy/variant
  additions small and well-tested.
- [Risk] Source checks may flag legitimate low-level component code. -> Run checks
  across the whole render tree while allowing direct Ratatui access only in named
  component, arrangement, theme, or future bespoke component/arrangement.
- [Risk] Existing render modules contain mixed screen and component logic. ->
  Classify and isolate ownership as part of this change while preserving output;
  separate visual redesigns are not required.
- [Risk] Agent skills can be ignored or unavailable in another environment. -> Put
  non-negotiable rules in `AGENTS.md` and keep the skill as workflow guidance.
- [Risk] Ratatui's buffer escape hatch permits bypasses inside the crate. -> Keep
  direct buffer access inside approved component/arrangement modules and make
  exceptions explicit in review.
