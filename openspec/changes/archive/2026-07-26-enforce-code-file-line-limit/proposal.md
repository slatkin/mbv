## Why

The 800-line guideline was applied to a small set of `src/app` hub files, while a repository-wide audit still finds 13 tracked code files above the limit. Without an enforceable check and a complete cleanup scope, future refactors can leave oversized files behind or recreate them unnoticed.

## What Changes

- Establish an 800-line maximum for every governed tracked source file in the repository.
- Include tests, `build.rs`, release/build scripts, and tracked source files using the repository's Rust, Lua, shell, Python, JavaScript/TypeScript, and C-family patterns; explicitly classify extensionless build hooks and packaging scripts, while excluding documentation, configuration/lockfiles, systemd units, generated output, and binary assets.
- Refactor all currently violating files, including files outside `src/app` and files created or left over by earlier splits.
- Add a repeatable repository check and CI enforcement so a new over-limit code file fails verification.
- Document the file classification, strict threshold, and approved remediation process.

## Capabilities

### New Capabilities

- `code-file-size-governance`: Defines and verifies the repository-wide maximum line count for tracked code files.

### Modified Capabilities

None.

## Impact

- Affects oversized Rust, Lua, and shell source files across `crates/`, `src/`, and `scripts/`, plus their test modules and packaging references where Lua modules are added.
- Requires structural refactors of the current 13 violating files without changing behavior or test coverage.
- Adds the canonical `make check-code-file-lines` verification command and a CI step; no new runtime dependencies are expected.
- May update contributor and refactoring documentation to make the rule an explicit acceptance criterion.
