# Design: palette enum

## Context

See proposal.md (Why). Current state after the revert of the first section-3
attempt: `render/theme/palette.rs` holds the landed 29-variant `Palette` enum
with `const fn color()/name()/hex()` and `ALL`, plus its uniqueness and
`docs/palette.json` drift tests, and has no consumers yet;
`render/theme/primitives.rs` holds 42 `Color` consts (29 distinct literals, two
primitive→primitive aliases); `render/theme/mod.rs` holds 51 role consts (48
production, 3 `#[cfg(test)]`-gated) aliasing primitives, plus the test-only
`resolve_surface_focus` fn; `surface_table.rs`/
`surface_resolve.rs` resolve `Surface` + bool to `Color`; `src/app/palette.rs`
re-exports roles + resolver to ~66 call-site files.

## Goals / Non-Goals

Goals: exactly one `Rgb` literal per distinct colour, owned by the closed enum;
every role and surface row a named reference to a variant; the palette set
enumerable so `docs/palette.json` is drift-tested.

Non-Goals: no visual change (every resolved value identical); **no type change to
any role, surface row, level fill, or resolver return**; no Surface/Level/
FocusSource remodeling; no new lint rules; no call-site changes beyond the one
stray literal; no edits to any frozen test file.

## Decisions

**`enum Palette` with 29 meaning-free variants over `consts + a naming convention`.**
Consts alone cannot say which of two equal literals owns a colour, and cannot be
enumerated, so the `docs/palette.json` drift guard is impossible without an
enum. `Palette::ALL` supplies both. This is the decision the landed core
(`9b740dfb`) already implements, and it stands.

**Roles and surface rows stay `ratatui::Color`, derived via `const fn color()`
(revised 2026-09-19; supersedes the original "roles become `Palette`" decision).**

```rust
pub(in crate::app) const ACCENT: Color = Palette::Aqua.color();
```

`const fn color()` (spike-confirmed under MSRV 1.88, below) makes this a
compile-time constant, so the role tier names a palette variant while its type is
unchanged. What the original type-changing design bought over this: enforcement
against a role smuggling an equal literal instead of referencing a variant. What
it cost, measured on the reverted implementation `4e710eb4`: **~450 `.color()`
call sites** across the 58 files of `4e710eb4`, a frozen-file neutrality rule that could not be
satisfied, and a surface-tier contradiction with `PopupDimBackdrop`. The
enforcement it bought applies to one 190-line file in which every role is
declared — reviewable by one `grep 'Rgb' theme/mod.rs` — and the implementation
did not even obtain it (`SurfaceColors.fill` remained `Color`, and
`chrome_tabs.rs:168` kept a live production literal). Disproportionate. Rejected.

Two consequences worth stating, because they are what made the first attempt fail:
- No frozen test file needs editing. Roles keep the type those tests read, so
  the frozen contract is satisfied literally — `git diff --stat` shows no frozen
  file — and no cross-type `PartialEq`, `cfg`-split role, or shim is needed. The
  2026-09-18 "ruling A" (mechanical `.color()` adaptation of frozen files) is
  withdrawn: it existed only to service the type change.
- `Row.resting` stays `Color`, so `PopupDimBackdrop`'s `Color::Black` blend base
  is an ordinary row value. No `Option<Palette>`, no panic arm, no wrapper enum.
  Decision D-3 as posed in the 2026-09-19 handoff is void.

**Three tiers: `Palette` <- Role <- Surface.**
Roles keep their meaning-names (`TEXT_FOCUS_ACCENT`) so call sites still say what
they mean; surface rows keep pointing at roles. Collapsing surfaces onto palette
variants directly would delete the meaning layer `ui-design-language` req 1
mandates. Role arrays (`HERO_META_ROLES`, `HINT_PILL_FILLS`) stay `[Color; 3]`,
with each element derived from a variant.

**`const fn color(self) -> Color` as the single literal owner.**
All palette `Rgb(...)` expressions live in the match arms of this one function.
Landed and in use already (`mod.rs` currently derives the test-only
`SURFACE_PLAYBACK` this way).

**Two roles at one variant means "equal today, independently editable" — and the
"deliberately not X" comments stay to say so.**
Six primitive pairs share a value on purpose, each documented and each installed
by an accepted change (`now-playing-media-type-titles` D2,
`unify-surface-colour-neutral` 4.2). Under this design those become two roles
referencing one variant — which, read cold, looks like a *sameness contract*.
The comments are the only thing distinguishing "these must match" from "these
happen to match"; deleting them (as the original task 4.1 did) destroys
information no type can recover. They stay, reworded from primitive names to
variant names.

Divergence is then an ordinary edit: the role that must move gets a new variant
with its new hex. The uniqueness test forbids two variants at one hex, so a
*duplicate-now-diverge-later* placeholder variant cannot be added — accepted:
that is the same edit, deferred, and deferring it buys nothing.

**`Palette`'s privacy is enforced by its module path, not its visibility modifier.**
The enum's items are `pub(in crate::app)`, which reads as broader than
`ui-design-language` req 2's "private to the theme", but `mod palette;` in
`theme/mod.rs` is private and never re-exported, so nothing outside `theme` can
name `Palette`. Leave it; noted so the next reader does not mistake the
modifier for a leak.

**Raw `Color::` specials stay outside the palette.**
Black (dim blend base), White (headings), Reset (transparency) are ratatui
mechanics, not palette colours. Routing them through the enum would give
meaning-names to non-meanings. Documented rationale in code, no change.

**29 neutral variant names: hybrid hue-literal + numbered clusters.**
Settled and approved; see `name-table.md`. Naming rules: plain hue names for
visually distinctive colours, keeping existing colour-words (`Gold`, `Aqua`,
`Red`, `Orange`, `Purple`, `Foam`); numbered only within tight clusters
(`Grey1`–`Grey6`, `Green1`–`Green3`); the three dark blue-grey slates are
`Slate`/`Storm`/`Flint`; PascalCase; enum ordered by hue then dark-to-light.

## The value oracle

Original plan claimed the frozen tests enforce value parity. Under this design
they do, honestly: the frozen files are untouched and their asserted expressions
still read `palette::ROLE` at type `Color`, so any changed role value fails them.
That is the primary oracle. Backed by:

- the approved hex table in `name-table.md`;
- the `ALL` uniqueness test (`9b740dfb`);
- the `docs/palette.json` sync test (`9b740dfb`);
- a final comparison of every resolved role and surface value against base
  `69344aff` (row 4.2).

## Risks / Trade-offs

- [Risk] A role could still hold a literal rather than a variant reference →
  Accepted. Every role is declared in one 190-line file; `grep 'Rgb'
  src/app/render/theme/mod.rs` returning nothing is the check, and the palette
  rule in `AGENTS.md`/`CONTEXT.md` is the standard. The 476-call-site alternative
  was measured and rejected above.
- [Risk] Palette grows without bound once divergence needs new entries →
  Accepted by design; `ALL` + the uniqueness test make growth visible in review.
- [Resolved] MSRV const-fn and `serde_json` availability — both spiked and
  confirmed, see below.

## Migration Plan

1. ~~Spike: MSRV const-fn + serde_json availability.~~ **Done** (`5ecffed4`).
2. ~~Name table: 29 variants, reviewed.~~ **Done, approved** (`5ecffed4`).
3. ~~Add `Palette` + `ALL` + uniqueness + JSON-sync tests, no callers.~~ **Done**
   (`9b740dfb`, `9fad1708`).
4. Derive the 51 role consts from variants; delete the two aliases. One green
   commit.
5. Derive the surface rows and level fills from variants; delete `primitives.rs`
   and its `mod` wire. One green commit.
6. Migrate `chrome_tabs.rs:168`; reword the sharing comments to variant names;
   sync `docs/palette.json`; full gates; archive.

Every step is type-preserving and independently green, so every row's acceptance
criterion is true at a commit boundary. Rollback of any step is a clean revert.

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
moves the value. This probe is also the direct evidence for the revised role
decision: `const ROLE: Color = Palette::X.color();` is exactly the const context
it exercised.

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
fallback) are both off the table.

## History: the first section-3 attempt and why it was reverted

`4e710eb4` implemented the original section 3 (roles, surfaces, and ~56 consumer
files migrated to `Palette`, `primitives.rs` emptied). It passed its gates and
review, and was reverted on 2026-09-19 as designed-wrong rather than built-wrong.
The three findings, recorded so they are not re-derived:

1. **The original "Why" misread the code.** It named the duplicate literals and
   "deliberately not X" comments as the defect; they are the deliberate
   edit-isolation mechanism of two accepted changes. Task 4.1's deletion of those
   comments would have discarded that intent, and the implemented result reads
   `ACCENT = Palette::Aqua` beside `PLAYBACK_TITLE_FG = Palette::Aqua` with
   nothing left to say they are independent.
2. **The type change was not needed for any stated benefit.** Enumerability — the
   one thing consts cannot do, and the thing the JSON drift guard requires —
   comes from the enum alone. The type change bought only anti-literal-smuggling
   enforcement in one file, for 476 call sites.
3. **Both escalations were consequences of (2).** The frozen-file rule was
   unsatisfiable only because roles changed type; the surface-tier contradiction
   (a uniform `Palette` return versus `PopupDimBackdrop`'s `Color::Black`) existed
   only because `Row.resting` changed type. Neither arises here.

Secondary corrections carried into the rewritten tasks: rows must not specify red
intermediate states as their verification; the stray production literal is
`chrome_tabs.rs:168` alone, and `media_list.rs:413,478` are test assertions, not
production strays as the original plan claimed. (Those two literals sat at
420/485 while `4e710eb4` was applied; the revert removed the inserted
`.color()` lines above them, so 413/478 is correct for this tree. Re-check the
numbers against HEAD rather than trusting either figure.); `Palette::ALL`/`name()`/`hex()`
are test-only and must be `#[cfg(test)]`-gated, never `#[allow(dead_code)]`, to
survive `clippy --all-targets -D warnings`; and `docs/palette.json`'s role
listing stays hand-maintained because no generator exists and the repo forbids
adding one.

## Open Questions

None.
