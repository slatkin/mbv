## 1. Panel State Transitions

- [x] 1.1 Reverse the wide `x` panel-mode cycle to `Both -> QueueOnly -> LibraryOnly -> Both`, preserving mode-following focus behavior and queue cursor initialization.
- [x] 1.2 Initialize ephemeral mini-view focus to `Queue` without adding preference persistence.
- [x] 1.3 Detect the rendered-width transition from 80+ columns to fewer than 80 columns and reset mini-view focus to `Queue` only on entry, leaving wide mode and focus untouched.
- [x] 1.4 Verify all panel/input/render reads continue to use effective mini-view state while persistence reads and writes remain limited to wide-mode state.

## 2. Behavior Coverage

- [x] 2.1 Update wide panel-mode tests to assert the complete `Both -> QueueOnly -> LibraryOnly -> Both` cycle and focus changes for queue-only and library-only states.
- [x] 2.2 Update the narrow startup render test to assert queue-only mini-view with a full-width queue panel.
- [x] 2.3 Add narrow-entry tests for startup and wide-to-narrow transitions from each wide mode, asserting queue-only entry without mutating stored wide mode or focus.
- [x] 2.4 Assert that narrow `x` toggles between queue-only and library-only without being reset on subsequent narrow renders, and that widening restores the prior wide mode and focus.

## 3. Documentation And Verification

- [x] 3.1 Update user-facing shortcut/help descriptions and applicable panel-mode documentation to describe the reversed queue-first cycle and mini-view default.
- [x] 3.2 Run focused panel-mode input/render tests and the application package check.
- [x] 3.3 Run workspace Clippy, the code-file line check, and strict OpenSpec validation.
