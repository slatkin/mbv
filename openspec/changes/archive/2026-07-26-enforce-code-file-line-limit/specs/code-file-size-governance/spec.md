## ADDED Requirements

### Requirement: Tracked code files have a strict maximum size

The repository SHALL define a deterministic classifier for governed tracked source files and SHALL require every classified file to contain no more than 800 physical lines. The classifier SHALL include tracked files using the repository's `.rs`, `.lua`, `.sh`, `.py`, `.js`, `.ts`, `.tsx`, `.c`, `.h`, `.cpp`, and `.hpp` extensions, plus `Makefile`, `PKGBUILD`, `PKGBUILD-git`, and `.githooks/*`. It SHALL include tests, `build.rs`, and files under `scripts/`, and SHALL exclude `docs/**`, `.github/**`, `openspec/**`, `dist/**`, `assets/**`, `fonts/**`, `contrib/*.service`, TOML/JSON/YAML files, lockfiles, generated output, and binary assets.

The policy SHALL NOT grant permanent per-file exceptions or grandfather existing violations.

#### Scenario: Current repository inventory has no violations

- **WHEN** the size check runs against the completed change
- **THEN** every classified tracked code file is reported at 800 lines or fewer
- **AND** the check reports zero violations

#### Scenario: Non-code artifacts are not governed by the limit

- **WHEN** the repository contains a documentation, configuration, generated, or binary file above 800 physical lines or records
- **THEN** the size check excludes that file from the governed inventory

#### Scenario: A supported source extension is added

- **WHEN** a change introduces a governed source language or extension
- **THEN** the classifier and its documentation are updated in the same change

### Requirement: Developers can run the size check locally

The repository SHALL provide the canonical command `make check-code-file-lines`, which enumerates all governed tracked source files, reports each file over 800 lines with its line count, and exits successfully only when no violation exists. The command SHALL report all violations in one run and SHALL use the same classifier in local and CI execution.

#### Scenario: A violation is present

- **WHEN** a governed tracked code file exceeds 800 lines
- **THEN** the command prints the file path and measured line count
- **AND** the command exits with a nonzero status

#### Scenario: No violation is present

- **WHEN** every governed tracked code file is at or below 800 lines
- **THEN** the command exits with status zero
- **AND** the command produces a clear success result

### Requirement: CI prevents new violations

Continuous integration SHALL run the repository size check for pull requests and pushes that can change governed code files. A failed size check SHALL fail the workflow before the change is considered verified.

#### Scenario: Pull request adds an oversized code file

- **WHEN** a pull request causes any governed tracked code file to exceed 800 lines
- **THEN** the size-check workflow fails
- **AND** the violation is visible in the workflow output

#### Scenario: Compliant pull request passes the size gate

- **WHEN** all governed tracked code files are at or below 800 lines
- **THEN** the size-check workflow passes

### Requirement: Existing oversized files are refactored without behavior loss

The implementation SHALL reduce each currently identified over-limit file to 800 lines or fewer by moving code into cohesive modules while preserving externally observable behavior, Rust test coverage, and stable shared-fixture access paths unless an intentional, reviewed API change is documented.

#### Scenario: Test-only code is split

- **WHEN** an oversized Rust test module is extracted into one or more focused files
- **THEN** the normalized test identity inventory before and after the move is identical, ignoring module-path changes caused solely by extraction
- **AND** the relevant tests pass

#### Scenario: Production code is split

- **WHEN** an oversized production module is divided by domain responsibility
- **THEN** all targets compile
- **AND** the full existing test suite passes
- **AND** the resulting files are each at or below 800 lines

### Requirement: The policy is documented for future changes

The repository SHALL document the 800-line threshold, governed file classification, exclusions, canonical `make check-code-file-lines` command, CI enforcement, and the expectation that new source-language or extensionless code patterns update the classifier.

#### Scenario: Contributor evaluates a new source file

- **WHEN** a contributor adds or splits a code file
- **THEN** the documentation identifies whether the file is governed
- **AND** it identifies the command used to verify compliance
