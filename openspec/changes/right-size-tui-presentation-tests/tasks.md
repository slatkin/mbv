## 1. Establish the Bounded Audit Set

- [ ] 1.1 Inventory the eleven `#[ignore]` presentation tests, whole-frame equality assertions, frame-glyph locators, and the row-sensitive tests named in #707; record for each assertion its owning layer and intended keep/rewrite/narrow/delete disposition, and verify every candidate belongs to the bounded families in `design.md` rather than an unrelated test family.
- [ ] 1.2 If the inventory leaves row-budget coupling unclear, temporarily add or remove one spacer row in the shared hero/list path, run the affected `mbv` test families to collect failures, then restore the production change completely and verify `git diff` contains no production rendering change before continuing.

## 2. Remove Inert Test Debt

- [ ] 2.1 Delete the ten ignored legacy render characterizations under `src/app/render/`, remove only imports and local helpers made unused by those deletions, and verify `rg -n '#\[ignore.*(obsolete legacy-render characterization)' src/app/render` returns no matches and the affected render test modules pass.
- [ ] 2.2 Delete the ignored obsolete boundary test in `src/app/tests_tick_integration_mouse_panels.rs`, remove only scaffolding made unused by that test, and verify the remaining mouse-panel tick integration tests pass through `Application::tick()`.

## 3. Right-size Owner-level Presentation Tests

- [ ] 3.1 Refine arrangement tests in the bounded hero/list families so each owned placement or breakpoint fact is asserted once and relationally, delete duplicate absolute row/coordinate claims, and verify the affected arrangement tests pass at the relevant Narrow and Wide boundaries.
- [ ] 3.2 Refine Render Component tests in the bounded hero/list families to assert only semantic content, style role, or paint-local geometry; remove frame-glyph locators and whole-frame equality snapshots, retain glyph checks only where the glyph is the painter's own semantic output, and verify the affected buffer tests pass.
- [ ] 3.3 Replace unexplained terminal dimensions in the row-sensitive owner tests named by #707 with dimensions derived from named fit requirements or production breakpoint constants, retain literal dimensions where size is the explicit subject, and verify each test still exercises its named presentation or fallback branch directly.

## 4. Narrow Interaction and Integration Tests

- [ ] 4.1 Narrow affected Interactive Component tests to local state transitions, semantic requests, viewport behavior, and retained hit resolution; remove assertions that repeat parent spacing or painter decoration, and verify the affected component test modules pass.
- [ ] 4.2 Narrow affected mounted tick and mouse tests to mounting, focus, subscriptions, latest-frame delivery, routing, stable-target resolution, and cross-boundary effects; remove repeated glyph, spacing, row-budget, and breakpoint-proxy assertions while preserving real `Application::tick()` coverage, and verify the targeted tick integration modules pass in their relevant Narrow and Wide presentations.
- [ ] 4.3 Remove shared test helpers and fixtures left with no callers after groups 2-4, keep `buffer_to_string` where focused semantic-content assertions still use it, and verify `cargo check -p mbv --tests` reports no dead imports or compilation failures.

## 5. Prevent Regression and Verify

- [ ] 5.1 Update the Tests section and completion checklist in `.agents/skills/mbv-frontend/SKILL.md` with the four-layer ownership matrix, terminal-size intent rule, owner-only semantic glyph rule, whole-frame equality prohibition, and mounted-test integration boundary; verify the guidance does not require a checker, snapshot system, live fixture, or one-test-per-deletion replacement.
- [ ] 5.2 Run `cargo fmt`, `cargo nextest run -p mbv`, `cargo clippy -p mbv --all-targets -- -D warnings`, and `openspec validate right-size-tui-presentation-tests`; verify all pass with no production behavior changes and no uncommitted temporary perturbation.
- [ ] 5.3 Summarize the deleted, rewritten, narrowed, and retained assertions by owning layer for #707, record any informational coverage remeasurement separately on #697 if one was run, and verify no shell/effect/run-loop coverage work was folded into this change.
