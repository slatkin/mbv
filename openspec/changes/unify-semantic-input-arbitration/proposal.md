## Why

Component-local interactions currently report through `Option<Msg>`, which cannot distinguish an unhandled input from one consumed by local state mutation. Multi-selection exposes the resulting architectural fault: central keyboard precedence would otherwise need mirrored copies of list-local Visual state, repeated `SelectionChanged` plumbing, and destination-specific sequencing that is difficult to modify and debug safely.

## What Changes

- Introduce one semantic input-arbitration contract that combines the Keyboard Router's ordered policy result with an explicit focused-leaf disposition for the same terminal event.
- Treat context-sensitive double-tap actions as deferred router candidates: a consumed local action suppresses and does not arm the candidate, while an unhandled local action permits it.
- Replace mutually exclusive media-list outcomes with one transition that can independently report consumption, selected-target change, multi-selection summary change, and an external stable-target intent.
- Keep keyboard and mouse delivery modality-specific through keyboard precedence and pointer eligibility/gesture/hit resolution, then converge them on the same target-resolved media-list operations.
- Project multi-selection summaries outward for status presentation without making that projection router authority or a writable selection mirror.
- Permit Library and Queue to retain independent simultaneous multi-selections; panel focus selects which summary and keyboard interaction are active.
- Carry stable selection origin through context-menu lifetime so actions and clearing apply to the originating list even if focus changes.
- Preserve existing global-chord precedence, mouse eligibility, geometry ownership, and user-visible single-row behavior.

## Capabilities

### New Capabilities

- `semantic-input-arbitration`: Explicit leaf consumption, central candidate arbitration, presentation-only local-state projection, and stable interaction origin.

### Modified Capabilities

- `interactive-component-framework`: Focused Interactive Components explicitly distinguish consumed local input from unhandled input while the Keyboard Router remains the sole precedence authority.
- `canonical-media-lists`: Shared list delegation returns orthogonal transition facts and accepts target-resolved semantic operations from keyboard and pointer paths.

## Impact

- `src/app/router.rs`, `src/app/key_policy.rs`, `src/app/input_resolver.rs`, and `src/app/shell_run.rs` — central arbitration and double-tap candidate lifecycle.
- `src/app/components/msg/`, `src/app/components/media_list/`, destination content owners, Queue, Library panel, and status panel — explicit dispositions, transition translation, selection-summary projection, and origin-aware clearing.
- `src/app/context_menu_actions.rs` and context-menu types — stable originating-list identity across overlay focus changes.
- Keyboard routing matrix, real `Application::tick()` integration tests, media-list unit tests, and Library/Queue simultaneous-selection tests.
- `add-media-list-multi-select` depends on this change's contracts and should be reconciled before either implementation begins.
