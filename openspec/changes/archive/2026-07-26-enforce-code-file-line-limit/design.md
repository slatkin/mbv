## Context

The repository documents 200–400 lines as typical and 800 lines as the maximum, but the rule is currently advisory. Issue #365 intentionally reduced four `src/app` hubs; those hubs are now compliant, while a broader tracked-source audit finds 13 remaining files over the limit across `crates/mbv-core`, `src/app`, and `scripts`.

The change must preserve behavior while splitting Rust modules, Rust test modules, and the large Lua script. Rust module declarations are shared coordination points, and test fixtures currently depend on stable module paths, so each refactor needs an explicit boundary and verification pass.

## Goals / Non-Goals

**Goals:**

- Make the 800-line maximum a deterministic, locally runnable check for tracked code files.
- Run that check in CI on changes that can modify the repository.
- Reduce every current violation to 800 lines or fewer, including tests and scripts.
- Prefer cohesive, domain-oriented modules and preserve public behavior, test names, and fixture access.
- Make the file classifier and remediation policy discoverable to contributors.

**Non-Goals:**

- Applying the limit to Markdown, TOML/YAML configuration, generated output, fonts, images, or other binary assets.
- Imposing a new function-length limit or requiring every file to have a particular target size below 800 lines.
- Introducing a parser, formatter, or third-party dependency solely for line counting.
- Rewriting behavior, changing APIs, or adding new product functionality as part of the splits.

## Decisions

### 1. Use one repository-local checker as the source of truth

Add a shell checker under `scripts/` that enumerates tracked files with `git ls-files`, selects the documented code-file patterns, counts physical lines with `wc -l`, and exits nonzero for any file strictly greater than 800 lines. It must report every violation, not stop at the first one, and handle paths safely. `make check-code-file-lines` is the canonical local command and SHALL invoke this checker rather than duplicate its logic.

The classifier will cover tracked files matching the repository's source extensions: `.rs`, `.lua`, `.sh`, `.py`, `.js`, `.ts`, `.tsx`, `.c`, `.h`, `.cpp`, and `.hpp`. It will also explicitly include `Makefile`, `PKGBUILD`, `PKGBUILD-git`, and `.githooks/*` as build, packaging, or hook code. This includes `build.rs`, tests, and files under `scripts/`; `docs/**`, `.github/**`, `openspec/**`, `dist/**`, `assets/**`, `fonts/**`, `contrib/*.service`, TOML/JSON/YAML files, lockfiles, generated output, and other binary paths remain excluded. Adding a new source language or extensionless code path requires updating the classifier and its documentation in the same change that introduces it.

No violating file will receive a permanent per-file exception or grandfathered status; every governed violation must be resolved by restructuring the file or changing its governed classification through a documented repository-wide policy decision.

Alternatives considered:

- **Ad hoc `wc` commands in CI:** rejected because local and CI scope can drift and violations are difficult to reproduce.
- **A Rust test:** rejected because the policy should run before compilation and would couple repository hygiene to the application test binary.
- **A new dependency:** rejected because `git`, a shell, and `wc` are already available in development and CI environments.

### 2. Enforce the checker in a dedicated lightweight CI workflow

Add a separate workflow triggered on pull requests and pushes to `main` that checks out the repository and runs the checker. Separate workflows have no guaranteed execution order, so this workflow SHALL be an independent required status for merge rather than claiming to run before the release/build workflow. Repository settings must mark its check as required once deployed.

### 3. Split by cohesive responsibility, not arbitrary line ranges

Refactor the 13 current violations in reviewed lanes. Test-only files may be moved mechanically, preserving test inventory and fixture paths; oversized inline core test blocks must be extracted and split along with their production modules where needed. Production Rust files should be split around existing domain boundaries, with visibility changes limited to what cross-module calls require. The Lua script should be split only where its existing command/event responsibilities form stable module boundaries, and its release/package assembly must ship every resulting module.

New Rust modules must be declared through the existing module tree, and lanes that touch a shared declaration file must be serialized or reconciled deliberately. No lane may claim completion solely because a source file became smaller; the checker and behavior-preservation tests are the acceptance gates.

### 4. Establish a baseline and verify after every lane

Before each refactor lane, capture the relevant test inventory and file-size inventory. Afterward, run the checker, formatting, compilation, and applicable tests; for pure moves, compare normalized test identities before and after, ignoring module-path changes caused solely by extraction. A final repository-wide run must show zero violations and preserve the full test suite.

## Risks / Trade-offs

- **[Risk] Splitting Rust modules changes privacy or module paths.** → Survey callers before moving items, use the narrowest required visibility, preserve stable fixture paths, and require `cargo check --all-targets` plus targeted tests.
- **[Risk] Mechanical test moves silently drop or rename tests.** → Compare sorted pre/post test inventories and require the full suite to pass.
- **[Risk] The checker omits a new code-file type.** → Keep the classifier explicit and documented, include extensionless known code files, and require classifier updates when adding a language.
- **[Risk] Parallel lanes conflict in module declarations.** → Isolate lanes in worktrees and serialize shared-parent edits or perform a reviewed integration step.
- **[Risk] The Lua split changes mpv runtime behavior without Rust tests detecting it.** → Keep moves behavior-preserving, validate Lua syntax when the tool is available, and retain a focused manual/runtime verification step.
- **[Risk] Splitting `mbv.lua` leaves release archives or packages incomplete.** → Update the tarball, `Cargo.toml`, and both PKGBUILD packaging paths together, then verify every resulting Lua module is shipped and loadable.
- **[Risk] A strict cap encourages meaningless fragmentation.** → Require cohesive domain boundaries and allow review to reject splits that preserve the line count only through arbitrary extraction.
