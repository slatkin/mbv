# Proposal

## Why

The workspace has no integration test target (#889). Every test lives in `src/`, so tests that
only use a crate's public API are compiled as unit tests. Nothing would notice if a public
contract changed shape. `M-INTEGRATION-TESTS` (`docs/standards/rules/M-INTEGRATION-TESTS.md`)
has been flagged in past audits too. It keeps coming back because the repo gives agents nothing
to copy (no `tests/` directory exists) and gives contradictory guidance: invariant 14 says
"narrowest layer", which agents read as "unit test in `src/`".

## What Changes

- Add one integration test binary per crate for `mbv-queue`, `mbv-ctrl`, `mbv-emby`,
  `mbv-audiobookshelf` and `mbv-config` (`crates/<crate>/tests/<name>.rs` plus a `<name>/`
  submodule directory).
- Move the existing tests that compile against the public API into those binaries. The compiler
  is the classifier: a test that needs a private or `cfg(test)`-only item stays in `src/`.
- Make `AudiobookshelfClient::with_test_agent` `pub` (no `cfg(test)`). This matches
  `EmbyClient::with_test_agent`, which is already `pub`, and lets the ABS client tests move.
- Delete `#[cfg(test)]` re-exports and imports that only existed for the moved tests.
- Stop the drift: state the placement rule in invariant 14 and in the `AGENTS.md` Tooling
  section. The rule says a library crate's public contract is owned at the crate's `tests/`
  layer. It also forbids widening visibility (`#[cfg(test)] pub(crate) use`) so that a
  public-contract test can stay in `src/`.

## Capabilities

### New Capabilities

None. This is test layout and contributor guidance. It changes no runtime behavior
(`skip_specs: true`).

### Modified Capabilities

None.

## Impact

- New `tests/` targets in five crates. Test files move out of `src/tests/`. Test count does not
  change.
- `mbv-audiobookshelf` public API: `with_test_agent` becomes `pub`.
- `mbv-daemon` is out of scope. Its tests drive private loop internals (`DaemonLoop`,
  `CtrlClients`, `apply_*`), so they are real unit tests. A public-contract test for the daemon
  would be a new test through `run_with_options` or the ctrl socket, and it gets its own issue.
- Docs: `docs/invariants/14-test-ownership.md`, `AGENTS.md`.
