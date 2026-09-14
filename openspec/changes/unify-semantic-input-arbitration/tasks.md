## 1. Characterize Current Arbitration

- [x] 1.1 Extend the shared routing-matrix/live-tick coverage with representative immediate `Command`, `Swallow`, and `FallThrough` rows plus first/second/expired Space and Escape presses; verify with `cargo nextest run -p mbv` filtered to the routing matrix and tick integration tests.
- [x] 1.2 Add one live `Application::tick()` characterization showing a focused component that mutates local state without emitting an effect request while `UiRoot` observes the same key; verify the existing mutation and message order before changing the contract.

## 2. Introduce Explicit Leaf Disposition

- [x] 2.1 Add the internal `Unhandled`/`Consumed` leaf-key result and framework-local consumed marker in the component message boundary; verify unit coverage distinguishes consumed-without-request, consumed-with-request, and unhandled conversion.
- [x] 2.2 Convert Library panel and its destination key-handler families to return explicit leaf results without changing their typed requests; verify the relevant component tests and live Library tick tests pass.
- [ ] 2.3 Convert Queue, playback Panels, Status bar, Tab panel, and pane-boundary key handlers to explicit leaf results; verify their component tests and live panel-focus tick tests pass.
- [ ] 2.4 Convert sidebars, popups, modals, and remaining overlays to explicit leaf results; verify blocking-overlay, text-entry, focus-restoration, and overlay tick integration tests pass.
- [ ] 2.5 Remove compatibility inference inside converted key handlers and verify an exact source search finds no local-only recognized key path represented by an ambiguous bare `None`.

## 3. Centralize Semantic Arbitration

- [ ] 3.1 Introduce the pure central arbitration fold over captured focus, one router observation, and the focused leaf result; verify one table-driven unit test covers every row of design D3's truth table and malformed combinations trigger debug assertions.
- [ ] 3.2 Route the production tick and shared tick-test harness through the new fold while preserving immediate router outcomes; verify the routing matrix and all `Application::tick()` integration tests pass.
- [ ] 3.3 Add the compact diagnostic record for chord, focus, router result, leaf disposition, final disposition, and dispatch kind without payload data; verify focused assertions/log output identify a deliberately malformed arbitration input.
- [ ] 3.4 Remove the superseded router-outcome application path and framework-local marker fall-through; verify exhaustive message dispatch remains wildcard-free and `cargo check -p mbv` passes.

## 4. Defer Context-Sensitive Double Taps

- [ ] 4.1 Change Space and Escape policy resolution to return deferred candidates without mutating candidate clocks; verify existing immediate global commands and blocking-overlay swallows remain unchanged in the routing matrix.
- [ ] 4.2 Move candidate clock advance/fire/reset into final arbitration so consumed presses reset and never arm a candidate; verify live ticks cover Visual-like consumed Space/Escape, subsequent unhandled presses, timeout, and ordinary double taps.
- [ ] 4.3 Remove pre-arbitration Space/Escape double-tap facts and any component-local-state facts from `RouterSnapshot`; verify exact source search and router tests show no selection or Visual-state mirror feeds keyboard policy.

## 5. Normalize Canonical Media-List Operations

- [ ] 5.1 Add the target-resolved provider-neutral media-list operation vocabulary while retaining coordinate-bearing input only above presentation point resolution; verify compile-time/type-focused unit cases make invalid coordinate-plus-missing-target delegation unrepresentable.
- [ ] 5.2 Replace exclusive `RowLocalOutcome` with one orthogonal transition carrying disposition, optional selected-target change, optional selection-summary change, and optional external intent; verify one table-driven MediaList test covers single- and multi-fact transitions.
- [ ] 5.3 Convert Wide and Inline presentation adapters and `MediaListCarrier` to resolve pointer points before creating target-bearing operations; verify current-frame geometry and pointer-continuity MediaList tests pass unchanged in behavior.
- [ ] 5.4 Convert Home, generic Emby Browser, and Feeds destination translation to one delegation per operation; verify their component and Library panel integration tests pass and exact source search finds no synthetic Click before DoubleClick/Context handling.
- [ ] 5.5 Convert Music and TV list/workspace translation to one delegation per operation; verify Wide and Narrow component tests plus relevant tick integration tests pass.
- [ ] 5.6 Convert Audiobookshelf podcast/book and Queue translation to one delegation per operation; verify provider workspace, Queue slot-identity, pointer, and tick integration tests pass.
- [ ] 5.7 Remove `RowLocalInput`/`pointer_target` compatibility delegation and `RowLocalOutcome`; verify exact source search finds neither old API and `cargo check -p mbv` passes.

## 6. Add Selection Projection and Stable Origin

- [ ] 6.1 Define stable Library-destination/list and Queue selection origins plus count-only summaries, keeping target membership private to each MediaList; verify type/unit tests show summaries cannot reconstruct or reseed membership.
- [ ] 6.2 Handle generic Library transition disposition and focused-summary propagation once at the Library panel owner boundary; verify destination switches and Library/Queue panel-focus ticks select the correct summary without clearing retained list state.
- [ ] 6.3 Add the equivalent Queue transition/summary boundary and project only the focused list's summary to the Status bar panel; verify a live tick/render-content test covers simultaneous Library and Queue selections across focus changes without brittle coordinate assertions.
- [ ] 6.4 Carry selection origin and ordered resolved action values through context-menu open, overlay focus, action, and clear; verify one integration test opens from Queue while Library retains a selection, changes overlay focus, and clears Queue only.
- [ ] 6.5 Route the Status bar clear intent to the origin captured when invoked rather than current focus at later dispatch; verify focus-changing coverage leaves the other Panel's selection unchanged.
- [ ] 6.6 Remove router use of selection summaries and all repeated `SelectionChanged` routing plumbing; verify exact source search plus a stale-summary arbitration test show keyboard behavior depends only on the current leaf disposition.

## 7. Reconcile Dependent Planning and Documentation

- [ ] 7.1 Update `add-media-list-multi-select` proposal, specs, and design to depend on the landed semantic arbitration/media-list transition contracts, remove router-state mirrors and duplicate transition sequencing, and preserve the confirmed simultaneous Library/Queue selection behavior; verify `openspec validate add-media-list-multi-select --strict` passes.
- [ ] 7.2 Update ADR 0023 and the interactive architecture documentation to describe deferred candidates and the single central arbiter without creating a second Keyboard Router; verify terminology agrees with `CONTEXT.md`, the capability specs, and production ownership.

## 8. Final Verification

- [ ] 8.1 Run `cargo fmt`, `cargo nextest run -p mbv`, `cargo check -p mbv`, and `cargo clippy --workspace --all-targets -- -D warnings`; fix failures without weakening the arbitration, ownership, or live-tick assertions.
- [ ] 8.2 Run `openspec validate unify-semantic-input-arbitration --strict` and verify the change contains no project-code edits outside the planned implementation scope, no writable selection mirror, no second keyboard resolution site, and no global mouse router.
