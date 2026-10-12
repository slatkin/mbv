---
status: accepted
---

# UI Style Preferences Are Not Specs

## Decision

`openspec/specs/` states observable behavior and layout structure only:
what renders, where it renders, when it renders, how focus and navigation
move, and what data a surface shows. It never pins paint: literal strings,
glyphs, icons, colors, roles, spacing, padding, alignment, marquee,
truncation, or other style choices.

An intentional change to the look of a surface supersedes any prior style
text. It is never a spec violation, and it needs no spec delta.
Paint-level `SHALL` statements are removed from the specs when found, and
reviewers flag newly added ones as findings.

## Considered options

- **Keep paint in specs and update it with each restyle (rejected).**
  Every visual tweak then fights a "spec violation" that only restates
  the intentional change. The fighting costs more than the text is worth.
- **Move paint rules to a style guide (rejected).** A second prescriptive
  document recreates the same fight under a new name. Style choices live
  in the code and its tests.

## Consequences

Spec reviews stay silent on look changes. Spec-writing agents (the
OpenSpec artifact rules) and reviewers (the review skill) carry the rule,
so neither writes nor enforces paint prescriptions. `docs/palette.json`
stays the generated theme reference; it is not a spec and never
constrains a restyle.
