#!/usr/bin/env bash

# Warn-only. Lists `#[cfg(test)]` re-exports that widen visibility so a test can
# stay in `src/` (docs/invariants/14-test-ownership.md, clause 2; #889).
# Always exits 0: a hit is a prompt to ask whether the test belongs in `tests/`.

set -u

hits=$(rg -nU --glob '*.rs' '#\[cfg\(test\)\]\s*\n\s*pub(\([a-z]+\))? use' crates/*/src) || true

if [[ -z "$hits" ]]; then
    printf 'test-visibility: no cfg(test) re-exports found\n'
    exit 0
fi

printf 'test-visibility: cfg(test) re-exports (check that no public-contract test needs them):\n'
printf '%s\n' "$hits"
exit 0
