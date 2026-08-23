---
status: proposed
---

# Incremental Interactive Component Migration

Moving mbv from global `App` ownership to interactive surface components is a
multi-step program. We propose an explicit, stable migration ledger rather than a
big-bang rewrite, an automatic touch-to-migrate rule, or optional opportunistic
cleanup.

The ledger has one row per independently interactive surface and only two committed
states: `legacy` and `migrated`. New interactive surfaces conform from creation.
Existing surfaces migrate through named, behavior-preserving OpenSpec changes.
Narrow bug fixes and shared visual-policy changes may touch legacy surfaces without
forcing full migration, but may not add a new surface-specific `App` state cluster,
new `impl App` interaction subsystem, or duplicated geometry without an explicit
exception.

A migration moves state once: the migrated component owns its local state, messages,
updates, rendering, and typed outputs, while old `App` fields and handlers are
removed rather than mirrored. Migrated paths are guarded by Rust privacy and path-
scoped static checks rejecting `App`, Service-client, `PlayerProxy`, protocol,
channel, persistence, and raw theme dependencies. Existing input precedence,
responsive behavior, images-disabled behavior, geometry contracts, and durable
behavior tests remain regression requirements.

Search is the proposed first proof because it spans local state, contextual input,
debounce, asynchronous completion, stale-result handling, rendering, viewport state,
dismissal, and navigation without crossing canonical playback authority. Search
implementation does not begin until the architecture map, terminology, and its
specific timer, request-identity, viewport, and shell-output contracts are reviewed.
A second, structurally different mouse-driven component is required before adopting
a shared lifecycle trait, effect executor, or generic geometry contract.

## Considered Options

- Big-bang conversion: rejected because failure would cross the entire TUI and
  process integration layer.
- Migrate every touched surface: rejected because narrow visual or bug work would
  acquire unrelated architecture scope.
- Opportunistic migration without enforcement: rejected because the existing global
  architecture would continue to grow.

## Consequences

The visual migration ledger archived with #563 is evidence, not the interactive
ledger. The interactive ledger lives at the stable, non-archived path
`docs/architecture/interactive-surface-ledger.md`. Each implementation slice
receives its own OpenSpec change and may ship independently; #603 remains the
umbrella tracker.
