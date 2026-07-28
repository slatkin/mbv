## ADDED Requirements

### Requirement: Cosmetic render tests removed

Render tests whose primary assertions check visual appearance details (glyph characters, pixel colors, indentation strings, font modifiers, art rect dimensions) SHALL be removed. These tests assert on design choices that are not yet stabilized and break on every UI tweak.

#### Scenario: Glyph assertion tests removed

- **WHEN** a test's primary assertion checks for specific Unicode glyph characters at specific buffer positions (e.g., `▌`, `▕`, `▁`, `▔`, `♥`, `🖧`, `🖭`)
- **THEN** that test SHALL be removed from the test suite

#### Scenario: Pixel color assertion tests removed

- **WHEN** a test's primary assertion checks exact color palette values at specific pixel coordinates (e.g., `assert_eq!(buffer[(x, y)].fg, palette::AQUA)`)
- **THEN** that test SHALL be removed from the test suite

#### Scenario: Hardcoded visual string tests removed

- **WHEN** a test asserts on exact rendered strings that encode visual layout choices (e.g., `"        ▌2. Focused Track"`, `"|| X >> Title"`, `"RES 1080p  AUD en  SUB off"`)
- **THEN** that test SHALL be removed from the test suite

### Requirement: Behavioral render tests preserved

Render tests whose primary assertions verify logic, state, geometry, or math independent of visual styling SHALL be preserved unchanged.

#### Scenario: Data logic tests preserved

- **WHEN** a test verifies data resolution logic (e.g., which item is selected, which host label is used, which session name appears)
- **THEN** that test SHALL remain in the test suite with identical assertions

#### Scenario: State transition tests preserved

- **WHEN** a test verifies state transitions (e.g., toast expiry clears state, session state after mode switching)
- **THEN** that test SHALL remain in the test suite with identical assertions

#### Scenario: Geometry and layout math tests preserved

- **WHEN** a test verifies hitbox geometry, scroll behavior, layout area calculations, or `content_rows()` math
- **THEN** that test SHALL remain in the test suite with identical assertions

### Requirement: Shared test helpers consolidated

Helper functions duplicated across multiple render test files SHALL be extracted into a single shared module (`test_helpers.rs`) and included before all test files.

#### Scenario: Common helpers extracted

- **WHEN** a helper function (e.g., `buffer_to_string`, `render_power_library_to_terminal`, `make_power_movie_app`) is duplicated across 3 or more test files
- **THEN** that helper SHALL be defined once in `test_helpers.rs` and removed from all individual test files

#### Scenario: File-specific helpers remain local

- **WHEN** a helper function is used by only one test file (e.g., `render_power_list_to_string` in `list_tests.rs`)
- **THEN** that helper SHALL remain in its original file

#### Scenario: All retained tests pass after consolidation

- **WHEN** `cargo test` runs after helper extraction and cosmetic test removal
- **THEN** all remaining behavioral tests SHALL pass with identical results

### Requirement: No production code changes

The audit SHALL NOT modify any production rendering code. All changes are test-only.

#### Scenario: Production code untouched

- **WHEN** the change is applied
- **THEN** no files outside of `src/app/render/tests*.rs`, `src/app/render/home_tests.rs`, `src/app/render/detail_tests.rs`, `src/app/render/list_tests.rs`, and the new `src/app/render/test_helpers.rs` SHALL be modified
