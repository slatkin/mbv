---
status: accepted
---

# Hierarchical Interactive Surface Components

mbv's completed UI design system centralizes arrangement, painting, and semantic
theme policy, but interactive state, input interpretation, updates, effects, and
render adapters remain concentrated on the global `App`. We propose a hierarchical
interactive surface-component architecture: parents route events by existing focus
and overlay priority, children own local presentation state and update it through
local messages, and children return typed outputs instead of mutating parents,
siblings, Services, or playback state directly.

The application shell remains responsible for terminal lifecycle, Remote Service
and worker lifecycle, Player ownership, canonical queue authority, persistence,
protocols, and external effects. Interactive components receive owned presentation
models and opaque stable action keys; they do not receive `App`, clients, config,
locks, channels, credentials, protocol objects, or `PlayerProxy`.

The existing visual substrate remains: arrangements own placement and breakpoints,
render components own painting, and theme exposes semantic roles. Rows, heroes,
pills, cards, and modal frames do not become interactive components merely because
they are reusable. The interactive boundary is an independently routed state
machine. Library is therefore a parent with destination children, not one monolith
and not one component per visual atom.

ADR 0002's central context precedence remains authoritative. The parent chooses
which child receives a raw event; the selected child determines its local meaning.
This proposal does not adopt a flat event broadcast, strict global TEA update,
Flux store, universal component trait, generic effect scheduler, dependency-
injection framework, or separate UI crate. Those may be considered only after two
structurally different concrete components provide evidence for a shared boundary.

The current `AppLayout` hit-target contract remains binding until a separate
evidence-based decision explicitly supersedes it. This ADR does not authorize a
partial generic hit-map migration.

## Considered Options

- Keep the render-only architecture: rejected because `App` continues to own and
  scatter every interactive concern.
- Rewrite the TUI into a full trait framework, TEA, or Flux architecture at once:
  rejected because it combines ownership migration with a whole-application rewrite.
- Use concrete hierarchical components first: accepted because each migrated
  surface is independently useful and later abstractions can be extracted from real
  interfaces.

## Consequences

The existing `Component` glossary term means a render painter. Canonical naming for
the new interactive concept and its module root must be approved in `CONTEXT.md`
before implementation. The complete current-state and target map is
`docs/architecture/interactive-tui-component-map.md`; issue #603 coordinates the
program.
