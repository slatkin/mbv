# Proposal

## Why

The workspace still has 2,887 tests after the September prune, and 2,096 of them are in the `mbv` TUI binary. That is where the design churns most and where the maintainer tests end-to-end by hand anyway. The weight comes from duplication, not rigor:

- The same shell contract is re-asserted once per screen family (tv, music, podcast, library_panel, emby_library, home, …).
- `#[case]` tables expand one contract across wide/narrow/mini (289 expanded cases in the binary).
- State and dispatch logic is exercised through the full tick loop (`…_through_tick`) instead of at its owning layer.

Each redundant test locks the UI in place and turns every intentional refactor into bulk test repair. In an app that is still changing, tests should build up where the code has settled and where real bugs occurred, not be front-loaded.

## What Changes

- Aggressively reduce the `mbv` binary test suite from about 2,096 tests to **at most 800**, with per-family budgets set in design.md.
- Each cross-boundary contract (mount, focus, subscription, routing, latest-frame delivery, projection, cross-boundary effect) keeps **one** shell tick-integration test. Per-screen copies are deleted unless the screen's behaviour genuinely differs.
- Breakpoint `#[case]` tables collapse to the case where behaviour changes. Tables whose cases all assert the same claim collapse to one case or are deleted.
- `…_through_tick` and similar full-loop tests of state or dispatch logic are deleted when an owning-layer unit test already covers the claim. Otherwise they are rewritten at the owning layer (state/dispatch), not ported.
- Delete tautological tests (M-TAUTOLOGICAL-TESTS), tests that mirror implementation branches, and tests of trivial accessors, constructors or `Default`s.
- Remove test helpers and fixtures left unused by the deletions.
- Update test-name references in `docs/architecture/interactive-surface-ledger.md` so each points at a surviving test.
- Add an "earn its place" rule to the `mbv-frontend` skill's test section: a new TUI test must own a contract no existing test owns, or reproduce a real bug.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. Tests only; no product behaviour changes (`skip_specs: true`).

## Impact

- Test code under `src/app/**` (tests, components, dispatch, shell, state, infra, input), plus `src/mpris.rs`, `src/tray*` and `src/local_daemon*` test modules.
- `mbv-core` and `mbvd` are **out of scope**. They are mostly stable-boundary tests (queue, config, persistence, API parsing) and stay.
- `docs/architecture/interactive-surface-ledger.md` and `.agents/skills/mbv-frontend/SKILL.md`.
- No production code, dependency, API or behaviour change. Production code is touched only to delete `#[cfg(test)]`-only helpers that become unused.
