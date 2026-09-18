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

## Spike outcomes (task 1.1, 2026-09-18)

Both probes passed; both fallback paths are confirmed unneeded.

**(a) `const fn color(self) -> Color` with a full `match` compiles under MSRV 1.88 — PASS.**
The 1.88 toolchain was installed and actually exercised (no divergence):

```text
$ rustup toolchain install 1.88 --profile minimal
  1.88-x86_64-unknown-linux-gnu installed - rustc 1.88.0 (6b00bc388 2025-06-23)
```

Throwaway probe crate (`/tmp/palette-probe`, deleted after the run; never in the repo):
a 29-variant `#[derive(Clone, Copy)] enum Palette` whose `const fn color(self) -> Color`
holds a full, exhaustive `match self` (one arm per variant, no wildcard) returning
`ratatui::style::Color::Rgb(...)`, evaluated in a `const` context (`const FIRST_COLOR:
Color = ALL[0].color();` over a `const ALL: [Palette; 29]`):

```text
$ cd /tmp/palette-probe && cargo +1.88 build
   Compiling ratatui v0.30.2
   Compiling palette-probe v0.0.0 (/tmp/palette-probe)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.37s
$ cargo +1.88 run -q
first=(26,26,26) last=(253,246,227) variants=29
```

Const-match (stable since 1.46) confirmed on 1.88.0 with the real `ratatui::style::Color`
return type. One implementation note surfaced for task 2.1: the enum needs
`#[derive(Clone, Copy)]` (or at least `Copy`) — const-context indexing out of `ALL`
moves the value.

**(b) `serde_json` reachable from `mbv` unit tests — PASS.**
Provided by the mbv package's regular `[dependencies]` table
(`serde_json.workspace = true`, Cargo.toml line 49); `[dev-dependencies]` lists only
`uuid`/`rstest`. Unit tests (`#[cfg(test)]` modules) compile inside the crate, so
regular dependencies are reachable without a dev-dependency entry. A temporary
`#[cfg(test)]` probe appended to `src/app/render/theme/mod.rs` (removed after the
run; worktree verified clean):

```text
$ cargo nextest run -p mbv spike_serde_json_probe
     Compiling mbv v0.20.3 (/home/slatkin/Dev/worktrees/palette-enum)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 33.83s
────────────
        PASS [   0.037s] (1/1) mbv::bin/mbv app::render::theme::spike_serde_json_probe::serde_json_reachable_from_unit_tests
────────────
     Summary [   0.038s] 1 test run: 1 passed, 1723 skipped
```

**Fallback paths: confirmed unneeded.** Non-const `Color` shims (const-fn fallback)
and an in-code `ALL` snapshot test with manual JSON regeneration (serde_json
fallback) are both off the table — the `const fn` and the `palette.json` sync test
can proceed as designed.

## Open Questions

None. Variant naming rules settled (see decision above); the secondary payoff of numbered clusters is making near-duplicate greys/greens visible for future consolidation.
