## ADDED Requirements

### Requirement: Module naming convention for src/app/
The project SHALL document a naming convention for modules in `src/app/` in `AGENTS.md` (or an ADR under `docs/adr/`).

#### Scenario: Convention is documented
- **WHEN** a developer reads the project conventions
- **THEN** they find a clear rule for naming modules in `src/app/`

### Requirement: Convention supports type-definition modules
The convention SHALL support a `types_` prefix for modules that primarily define type aliases, enums, or structs used across the app.

#### Scenario: Type-definition module naming
- **WHEN** a new module primarily defines shared types
- **THEN** it SHALL be named with a `types_` prefix (e.g., `types_browse.rs`)

### Requirement: Convention supports action-handler modules
The convention SHALL support an `_actions` suffix for modules that implement action handlers or event dispatch logic.

#### Scenario: Action-handler module naming
- **WHEN** a new module implements action handlers or event dispatch
- **THEN** it SHALL be named with an `_actions` suffix (e.g., `queue_actions.rs`)

### Requirement: Convention covers all other modules
The convention SHALL specify that all other modules use bare nouns without prefixes or suffixes.

#### Scenario: Other module naming
- **WHEN** a new module does not fit the type-definition or action-handler categories
- **THEN** it SHALL be named with a bare noun (e.g., `construct.rs`, `resize.rs`)

### Requirement: No file renames required
The implementation SHALL NOT require renaming existing files. The documented rule shall grandfather in existing files.

#### Scenario: Existing files unchanged
- **WHEN** the convention is documented
- **THEN** no existing files in `src/app/` are renamed
