## Why

Issue #369 is the remaining test-only cleanup from the completed #365-#368 `src/app` decomposition: four sibling test modules still exceed the repository's 800-line guideline and mix concerns that now have clear production and domain boundaries. The prerequisite production splits have landed, so these tests can now be reorganized as a standalone, low-risk move without coupling the work to another refactor.

## What Changes

- Split library-position model/persistence, runtime restore orchestration, and panel-focus tests into focused sibling modules.
- Split the mis-clustered home-video feed group and podcast tests, using separate feed navigation and feed loading/reconciliation modules so no replacement remains over 800 lines.
- Split queue population/removal tests from queue reorder, undo, slot-identity, and remote-reconciliation tests.
- Split local-daemon bootstrap, Sessions-panel connection, auto-reconnect, and library-route tests along their existing production and domain seams.
- Move the remote-position extrapolation outlier into the existing lifecycle test module rather than leaving it under session connection.
- Preserve every existing test name, body, assertion, local helper, and shared fixture path; change only test-module placement and `src/app/mod.rs` test declarations.
- Keep production behavior, visibility, APIs, dependencies, and unrelated oversized test files unchanged.

## Capabilities

### New Capabilities
- `app-test-module-organization`: Defines the structural and preservation requirements for decomposing the four oversized `src/app` test modules into focused, sub-800-line siblings.

### Modified Capabilities

None.

## Impact

- Affected code is limited to the test declarations in `src/app/mod.rs`, the four issue #369 source test files, their focused replacement siblings, and `src/app/tests_lifecycle.rs` for one runtime-state outlier.
- `src/app/tests.rs` remains the shared fixture module and retains its current import path.
- No production code paths, public APIs, persisted data, dependencies, ADRs, or domain behavior change.
- The broader module-naming decision in #375 and other cleanup issues such as #374 and #378 remain independent.
