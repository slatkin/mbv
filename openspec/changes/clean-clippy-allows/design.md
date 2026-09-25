# Design

## Context

Issue #792 names three suppression families. Verified current state (2026-09-25, after the `remove-production-dead-code` wave):

- `clippy::unwrap_used` × 4: `inline_search.rs:458`, `feeds_content/mod.rs:444`, `panel_list.rs:138` (module attribute), `hero_header/hero_header_tests.rs:5` (file-level). The lint is not enabled anywhere — no `[lints]` table in any Cargo.toml, `clippy.toml` only sets `too-many-arguments-threshold` — so these allows are inert.
- `clippy::items_after_test_module` × 3: `wide_hero.rs:90,322,469`. File layout: production items occupy lines 1–88 and 225–320, with four `#[cfg(test)]` modules interleaved at 89–223, 321–466, 468–559. The lint fires because production items follow test modules.
- `clippy::large_enum_variant` × 2: `msg/mod.rs:39` (`LeafKeyResult::Consumed(Option<Msg>)`) and `:80` (`Msg`). The stale `TODO(migrate-tui-to-tuirealm)` deferral is resolved: the Tuirealm migration (ADR 0022) is complete, and the user confirmed boxing now.

The Tuirealm migration being done also means `Msg` construction churn has settled, so mechanical site updates are low-risk.

## Goals / Non-Goals

**Goals:**
- Zero `clippy::unwrap_used`, `items_after_test_module`, or `large_enum_variant` allows in the workspace; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `wide_hero.rs` reads top-to-bottom as production code, then tests.
- `Msg`'s oversized payload boxed with the smallest possible churn.

**Non-Goals:**
- Enabling `unwrap_used` as a restriction lint (that is a lint-policy decision, out of scope).
- Revisiting the other test-file allows or the `clippy.toml` threshold (both handled by #788 / prior changes).
- Any behavior, API, or test-coverage change.

## Decisions

**D1 — Delete the `unwrap_used` allows, don't enable the lint.** The allows are proven no-ops (lint not enabled), so deletion is zero-risk; enabling `unwrap_used` would surface new findings across all test code and is a separate policy question. Alternative considered: keep them in case the lint is enabled later — rejected as speculative (YAGNI).

**D2 — Hoist the production block in `wide_hero.rs` rather than pushing test modules down.** Moving the ~96-line production block (the `WideHeroBrowserPane`/`PillBarAreas` structs and adjacent functions, lines 225–320) up to sit directly after line 88 leaves all four test modules after production code and moves fewer lines than relocating the two early test modules (~135 lines) to the end. Test module contents are untouched; `super::*` imports still resolve because the modules stay in the same file.

**D3 — Let clippy name the `Msg` offender; box exactly that arm.** Remove the `large_enum_variant` allow first, run clippy, and read its diagnostic (it reports the offending variant and byte sizes). Box only that payload — e.g. `Shell(Box<ShellRequest>)` if that is the reported arm — and mechanically update construction and match sites. Alternatives considered: box every request payload (churn for no lint benefit) and raising the size threshold (suppression by another name). The `LeafKeyResult` fix is known in advance: `Consumed(Option<Box<Msg>>)`; `into_option` maps through the box (`message.map(|b| *b).or(Some(Msg::TerminalEvent(KeyClaimed)))`-shaped), and `leaf_key_tests` construction sites wrap accordingly.

**D4 — Delete the stale TODO comment with the allow.** The `TODO(migrate-tui-to-tuirealm)` on `Msg` and the "Msg is the large arm; boxing would wrap every request" note on `LeafKeyResult` describe the deferral this change reverses; leaving them would contradict the code.

**D5 — Land on top of the in-flight dead-code follow-up.** `feeds_content/mod.rs` and `shell`-adjacent files carry uncommitted edits from the `remove-production-dead-code` task 7.x work. This change starts only after that work is committed, to keep the two cleanups reviewable separately.

## Risks / Trade-offs

- [Boxing changes `Msg`'s memory profile (heap allocation per large-arm construction)] → Negligible at key-event rates and explicitly accepted: the user chose "Box now" over the recorded deferral. Clone/PartialEq semantics are unchanged (`Box` derefs transparently).
- [The moved `wide_hero.rs` block could collide with concurrent edits] → Same mitigation as D5: sequence after the in-flight commits; the move is a pure reorder verified by `cargo check` + unchanged test outcomes.
- [Clippy names an unexpected variant as the large one (not `Shell`)] → The diagnostic is authoritative; D3 boxes whatever it reports, so the plan does not depend on guessing the arm.

## Migration Plan

Single-commit internal refactor; no rollout or rollback concerns beyond `git revert`. Verification gates: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv`, `cargo fmt`, and `make check-code-file-lines` before push.

## Open Questions

None. The only material decision (box now vs defer) was resolved by the user: box now.
