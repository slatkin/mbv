## Why

Small, intentional TUI presentation changes currently break tests across arrangement, painter, Interactive Component, and shell-integration layers because those tests repeatedly encode the same row budget, glyph, or whole-frame output. This makes low-risk visual work disproportionately expensive while the higher-risk shell and effect paths identified by #697 remain comparatively under-covered.

## What Changes

- Classify TUI presentation tests by the layer that owns the asserted contract: arrangement, Render Component, Interactive Component, or shell integration.
- Keep one owner-level presentation characterization for each relevant surface and presentation; remove parallel assertions of the same spacing, glyph, or row arithmetic from other layers.
- Remove obsolete ignored presentation tests and test helpers left unused by the audit.
- Replace whole-frame snapshots and glyph-as-location assertions with focused semantic-content assertions or role-rect containment where those tests still carry an owned contract.
- Replace terminal dimensions that accidentally proxy for fit or presentation selection with dimensions derived from the named requirement; retain explicit boundary dimensions when terminal size is itself the subject.
- Restrict mounted tick and mouse tests to composition, routing, gesture, focus, subscription, and cross-boundary behavior rather than repeating painter appearance.
- Extend the `mbv-frontend` testing guidance and completion checklist so the ownership rule remains visible during review.
- Use a temporary row perturbation only as a discovery aid, restore it before test changes, and classify every resulting failure rather than treating failure as automatic grounds for deletion.
- Do not add tests solely to preserve a coverage percentage, and do not add scripts, snapshot infrastructure, smoke tests, or live external fixtures.

## Capabilities

### New Capabilities

None. This change right-sizes test coverage and review guidance without changing product behavior.

### Modified Capabilities

None.

## Impact

- Test code under `src/app/**`, concentrated in render characterization and mounted tick/mouse families exposed by #707.
- Shared TUI test helpers whose remaining uses disappear with deleted tests.
- `.agents/skills/mbv-frontend/SKILL.md` testing guidance and completion checklist.
- No production behavior, public API, dependency, protocol, or user-visible presentation change.
- #697 remains separate: this change may provide a fresh coverage baseline but does not add shell, effect, run-loop, process, or external-boundary tests.
