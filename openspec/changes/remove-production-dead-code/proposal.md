# Proposal

## Why

`#[allow(dead_code)]` and `#[cfg_attr(..., allow(dead_code))]` suppressions currently hide production-unreachable declarations across the application, while tests that call those declarations make speculative code look maintained. This issue is to make dead-code linting an effective maintenance signal again without changing runtime behavior or adding another blanket suppression.

## What Changes

- Inventory every production dead-code suppression identified by issue #789 and classify each affected declaration by whether production code can reach it.
- Delete declarations that have no production caller, including obsolete fields, helpers, test-support seams, and the tests whose only purpose is to exercise those declarations.
- Keep declarations that are reachable from production and remove their now-unnecessary `dead_code` attributes; clean up imports and test scaffolding made obsolete by the removals.
- Remove the speculative “ships its whole contract ahead of its second adopter” convention from the shared list seam and the matching session-connect rationale when the affected primitive is unused.
- Preserve all user-visible, protocol, queue, playback, and persistence behavior; add no new lint suppression and add no replacement API solely to keep tests alive.
- Verify the cleanup with workspace compilation, the hermetic test suite, formatting, and the strict workspace clippy command.

## Capabilities

### New Capabilities

(none — this is a production-dead-code and test-scaffolding removal, not a new feature)

### Modified Capabilities

(none — no externally observable requirement changes; the change is an internal cleanup)

## Impact

- Primary production scope: shared list and media-list seams, feed/TV/music destination content, library-panel content, render theme and tree-browser modules, shell/input/queue/component helpers, and `src/mpris.rs` where the issue inventory identifies suppressions.
- Related test modules and helpers may be removed when they only call deleted code; retained tests continue to cover production behavior.
- Documentation changes are limited to removing obsolete dead-code rationale/comments.
- No dependency, wire-protocol, public API, runtime, or user-visible behavior is intended to change.
