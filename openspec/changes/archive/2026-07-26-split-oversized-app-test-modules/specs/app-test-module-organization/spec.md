## ADDED Requirements

### Requirement: Focused app test ownership
The test suite SHALL organize the tests currently held by the four issue #369 source modules into direct `crate::app` sibling modules whose names and contents represent one coherent concern. The exact 108-test assignment and destination counts in `design.md` under "Exact move manifest" are normative requirements of this capability.

#### Scenario: Library state concerns are separated
- **WHEN** the library-position tests are reorganized
- **THEN** 12 tests reside in `tests_library_position`, 10 in `tests_library_position_restore`, 6 in `tests_panel_focus`, and the queue-restore outlier contributes to the 9 tests in `tests_daemon_bootstrap`, exactly as assigned by the normative manifest

#### Scenario: Feed and podcast concerns are separated
- **WHEN** the combined feed/podcast module is reorganized
- **THEN** 10 tests reside in `tests_feed_group_nav`, 4 in `tests_feed_group_loading`, and 7 in `tests_podcast`, exactly as assigned by the normative manifest

#### Scenario: Queue concerns are separated
- **WHEN** queue-mutation tests are reorganized
- **THEN** 12 tests reside in `tests_queue_mutation` and 16 in `tests_queue_reorder`, exactly as assigned by the normative manifest

#### Scenario: Connection concerns are separated
- **WHEN** session-connect tests are reorganized
- **THEN** 9 tests reside in `tests_daemon_bootstrap`, 7 in `tests_session_connect`, 6 in `tests_auto_reconnect`, 8 in `tests_library_route`, and the remote-position runtime outlier moves to existing `tests_lifecycle`, exactly as assigned by the normative manifest

### Requirement: Test preservation
The reorganization MUST preserve every affected test's registration, function name, attributes, comments, assertions, setup, nested helpers, synchronization guards, override lifecycle, and observable result.

#### Scenario: Test inventory is unchanged
- **WHEN** affected module-name segments are normalized in sorted `cargo test --bin mbv -- --list` output captured before and after the reorganization
- **THEN** the inventories are identical with no missing, duplicated, or renamed tests

#### Scenario: Override-based tests retain isolation
- **WHEN** a moved test uses a global override or test lock
- **THEN** its lock acquisition, override installation, and override reset remain within the unchanged test item

### Requirement: Module size ceiling
Every destination test module created or retained by this change SHALL contain fewer than 800 lines after formatting.

#### Scenario: Destination sizes are measured
- **WHEN** formatting is complete
- **THEN** `wc -l` reports fewer than 800 lines for each of the twelve decomposed modules and the existing lifecycle destination named by the design

### Requirement: Existing test access model
The destination modules SHALL remain direct children of `crate::app`, SHALL use the established `tests_<concern>.rs` naming pattern, and SHALL continue to access shared fixtures through `crate::app::tests::*` without production visibility changes.

#### Scenario: New modules are declared
- **WHEN** the test module declaration block in `src/app/mod.rs` is updated
- **THEN** every destination is declared through the existing `#[cfg(test)]` and `#[path = "..."]` sibling pattern and the obsolete combined feed/podcast declaration is absent

#### Scenario: Shared fixtures remain stable
- **WHEN** the reorganized tests compile
- **THEN** `src/app/tests.rs` and its fixture import path are unchanged

### Requirement: Production behavior remains unchanged
The change MUST be limited to test placement, test-only module declarations, and import headers required by the new test files; it MUST NOT alter production bodies, signatures, APIs, dependencies, persisted data, or runtime behavior.

#### Scenario: Structural diff is reviewed
- **WHEN** the completed implementation diff is inspected
- **THEN** the only change in `src/app/mod.rs` is its test declaration block, all moved test items are content-equivalent, and no production or shared-fixture edits are present

#### Scenario: Repository verification passes
- **WHEN** the repository's formatting, workspace check, Clippy, targeted test, and full workspace test commands run after the move
- **THEN** every command exits successfully without warnings promoted to errors
