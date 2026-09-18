# Design: palette enum

## Context

See proposal.md (Why). Current state: `render/theme/primitives.rs` holds 42 `Color` consts (several duplicate `Rgb` literals, two primitive→primitive aliases); `render/theme/mod.rs` holds 51 role consts (47 production, 4 test-only) aliasing primitives; `surface_table.rs`/`surface_resolve.rs` resolve `Surface` + bool to `Color`; `src/app/palette.rs` re-exports roles + resolver to ~66 call-site files. Standing principle for this change: compiler enforcement over tests/lints wherever possible.

## Goals / Non-Goals

Goals: exactly one `Rgb` literal per distinct colour, owned by a closed enum; every role and surface row a compiler-visible assignment to that enum; `Color` materializes only at the ratatui boundary; `docs/palette.json` mechanically tied to the enum.
Non-Goals: no visual change (every resolved value identical); no Surface/Level/FocusSource remodeling; no new lint rules; no call-site logic changes beyond the type migration.

## Decisions

**`enum Palette` with 29 meaning-free variants over `consts + uniqueness tests`.**
Consts + tests cannot distinguish "role references GOLD" from "role smuggles an equal literal" — values, not references, are all a test sees. The enum makes assignments real relationships: adding a colour is adding a variant, and `Palette::ALL` enumerates the set for the JSON-sync test and the viewer. Cost accepted: the `Color`→`Palette` type change ripples through ~65 files.

**Three tiers: `Palette` <- Role <- Surface.**
Roles keep their meaning-names (`TEXT_FOCUS_ACCENT`) so call sites still say what they mean; surface rows keep pointing at roles. Collapsing surfaces onto palettes directly would delete the meaning layer that `ui-design-language` req 1 mandates. Role arrays (`HERO_META_ROLES`, `HINT_PILL_FILLS`) become `[Palette; 3]` — they are role-tier, so call sites convert at use. Alternatives rejected: two-tier (loses req 1), roles-as-`Color`-from-palette (loses reference enforcement — the consts option above).

**`const fn color(self) -> Color` as the single literal owner.**
All `Rgb(...)` expressions live in the match arms of this one function. Whether the frozen-test `Color` shims can be `const` depends on const-match under the repo MSRV — task 1 spikes this; fallback is non-const shims, and only if frozen tests use them in const contexts does this escalate to a rule (unfreezing) decision.

**Frozen-test shims stay thin and test-only.**
`SURFACE_PLAYBACK` etc. become `Palette::X.color()` shims under the existing `#[cfg(test)]` gates. No production path touches them; they shrink the diff to zero for frozen files.

**Raw `Color::` specials stay outside the palette.**
Black (dim blend base), White (headings), Reset (transparency) are not palette colours — they are ratatui mechanics. Routing them through the enum would give meaning-names to non-meanings. Documented rationale in code, no change.

**29 neutral variant names: hybrid hue-literal + numbered clusters.**
Current primitive names leak meaning (`LIBRARY_SIDE_BG`, `PLAYBACK_PANEL_BG`); palette variants must be meaning-free or the palette re-creates roles under new names. Naming rules:
- Plain hue names for visually distinctive colors; keep existing color-words where they already are color-words (`Gold`, `Aqua`, `Red`, `Orange`, `Purple`, `Foam`).
- Numbered within tight clusters only: `Grey1`–`Grey6` for the 6 achromatic greys, `Green1`–`Green3` for the 3 dark green-greys (`#3c4841`, `#48584e`, `#6c766c`). Visually distinct greens (`#83c092`, `#93b259`, `#a7c080`) get plain names.
- The 3 dark blue-grey slates (`#2d353b`, `#333c43`, `#3c424a`): `Slate`, `Storm`, `Flint`.
- PascalCase (idiomatic Rust enum variants).
- Enum definition ordered by hue, then dark-to-light within each hue family.
- The mapping table (old primitive → variant → hex) is committed as `openspec/changes/palette-enum/name-table.md` and reviewed before any code is written.

## Risks / Trade-offs

- [Risk] 66-file mechanical churn hides a real change → Mitigation: value-parity is test-enforced (existing buffer/characterization tests unchanged and green); the diff per file is `.fg(role)` → `.fg(role.color())` only, reviewable by pattern.
- [Risk] MSRV lacks const-match → Mitigation: spike first (task 1); MSRV 1.88 has had const-match since 1.46, so this is near-certain confirmation. Fallback path decided (non-const shims), escalation path defined (rule change, user decision).
- [Risk] Palette grows without bound once divergence needs new entries → Mitigation: accepted by design (divergence rule from exploration); `ALL` + uniqueness test make growth visible in review.
- [Risk] `palette.json` sync test needs JSON parsing in unit tests → `serde_json` is already a workspace dependency; spike confirms it is available to `mbv` dev-dependencies. Fallback is a golden `ALL`-snapshot test in-code plus manual JSON regeneration step.

## Migration Plan

1. Spike: MSRV const-fn + serde_json availability.
2. Name table: 29 variants, reviewed.
3. Add `Palette` + `ALL` + uniqueness test alongside existing code (no callers yet).
4. Migrate roles → `Palette`, surface rows/resolver → `Palette`, bridge re-exports.
5. Mechanical call-site migration; frozen-test shims; full gate run.
6. Delete `primitives.rs`, dead comments; sync `palette.json`; archive.
Rollback: each step before (5) is additive; (5) is one mechanical commit revertible cleanly. No visual change at any step, so no staged rollout needed.

## Open Questions

None. Variant naming rules settled (see decision above); the secondary payoff of numbered clusters is making near-duplicate greys/greens visible for future consolidation.
