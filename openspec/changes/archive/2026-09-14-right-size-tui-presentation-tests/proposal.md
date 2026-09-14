## Why

Small, intentional TUI presentation changes currently break tests across arrangement, painter, Interactive Component, and shell-integration layers because those tests repeatedly encode the same row budget, glyph, or whole-frame output. At the same time, tests that construct live external resources create flake and maintenance cost in a single-user application whose test policy requires hermetic mocks; both forms of misplaced test spend should be removed before adding coverage to the higher-risk shell and effect paths identified by #697.

## What Changes

- Classify TUI presentation tests by the layer that owns the asserted contract: arrangement, Render Component, Interactive Component, or shell integration.
- Keep one owner-level presentation characterization for each relevant surface and presentation; remove parallel assertions of the same spacing, glyph, or row arithmetic from other layers.
- Remove obsolete ignored presentation tests and test helpers left unused by the audit.
- Replace whole-frame snapshots and glyph-as-location assertions with focused semantic-content assertions or role-rect containment where those tests still carry an owned contract.
- Replace terminal dimensions that accidentally proxy for fit or presentation selection with dimensions derived from the named requirement; retain explicit boundary dimensions when terminal size is itself the subject.
- Restrict mounted tick and mouse tests to composition, routing, gesture, focus, subscription, and cross-boundary behavior rather than repeating painter appearance.
- Extend the `mbv-frontend` testing guidance and completion checklist so the ownership rule remains visible during review.
- Audit every workspace test target for live external construction: real mpv handles, sockets/listeners, spawned product processes or binaries, writable config/state directories, live servers or Services, and network-dependent smoke tests.
- Delete tests whose subject is the external itself; where a test protects mbv-owned request, classification, lifecycle, or bookkeeping logic, convert it to an existing in-memory/mock boundary instead of deleting that behavior claim.
- Preserve read-only static fixture access and ordinary in-memory/thread/channel synchronization; these are hermetic inputs, not live tests.
- Use a temporary row perturbation only as a discovery aid, restore it before test changes, and classify every resulting failure rather than treating failure as automatic grounds for deletion.
- Do not add tests solely to preserve a coverage percentage, and do not add scripts, snapshot infrastructure, smoke tests, or live external fixtures.

## Capabilities

### New Capabilities

None. This change right-sizes test coverage and review guidance without changing product behavior.

### Modified Capabilities

None.

## Impact

- Test code across the workspace for the live/smoke audit; TUI presentation changes remain concentrated in `src/app/**` families exposed by #707.
- Existing mock and in-memory test boundaries may be reused where mbv-owned logic must survive removal of a live harness.
- Shared TUI and external-boundary test helpers whose remaining uses disappear with deleted tests.
- `.agents/skills/mbv-frontend/SKILL.md` testing guidance and completion checklist; the repository-wide mocks-only policy remains authoritative.
- No product behavior, public API, dependency, protocol, or user-visible presentation change; test-only seams in production modules are allowed only when needed to inject an existing mock boundary.
- #697 remains separate: this change may provide a fresh coverage baseline but does not add shell, effect, run-loop, process, or external-boundary coverage merely to replace removed tests.
