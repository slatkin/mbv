## 1. Tab hover ownership and painting

- [x] 1.1 Add `TabPanel` component tests for pointer movement over an unselected retained tab target, movement into a gap, and movement over the selected tab; verify hover changes no selection/focus state and returns no `Msg` with `cargo nextest run -p mbv tab_panel`.
- [x] 1.2 Add the private hovered-tab identity to `TabPanel`, resolve `MouseEventKind::Moved` through its existing latest-frame tab hits, and pass the identity into the tab paint model; verify the task 1.1 tests pass.
- [x] 1.3 Add resting/hovered/selected precedence to the central tab painter using existing semantic theme roles, and add buffer assertions that unselected hover strengthens the label while selected hover preserves selected styling; verify with `cargo nextest run -p mbv tab_panel`.

## 2. Main Library Selector-row hover

- [x] 2.1 Add focused `LibraryPanel` tests for movement over an unselected main Selector-row pill and movement into a gap, proving the component changes only its private main-selector hover identity, returns no `Msg`, and does not alter selection; verify with the narrowest `cargo nextest run -p mbv` filters covering `library_panel`.
- [x] 2.2 Add the private main-selector hover identity to `LibraryPanel`, replace it from the existing `hits.selector` registry on every `MouseEventKind::Moved`, and leave List controls, Workspace selector, hero, and media-list mouse paths untouched; verify the task 2.1 tests pass.
- [x] 2.3 Extend the shared `PillBar` paint model with an explicit optional hovered identity, thread it only through the Library panel's main `paint_selector_row`, and implement selected-over-hover-over-resting style precedence from existing semantic theme roles; verify focused pill-bar buffer tests pass.
- [x] 2.4 Add buffer coverage proving main Selector-row hover paints in both Narrow and Wide Library panel presentations while Workspace-selector and selection-modal pills remain unchanged when no hover identity is supplied; verify with the narrowest matching `cargo nextest run -p mbv` filters.

## 3. Live delivery and architecture records

- [x] 3.1 Add a real `Application::tick()` integration test through the shell synchronization pass that moves the pointer between a tab target, a main Selector-row pill, and a gap, proving only the pointed in-scope surface repaints hovered and no shell action occurs; verify with the matching `tests_tick_integration_mouse` nextest filter.
- [x] 3.2 Extend the integration coverage to both Narrow and Wide Panel modes using each component's retained role geometry rather than fixed coordinates; verify both cases pass under the same nextest filter.
- [x] 3.3 Update `docs/architecture/interactive-surface-ledger.md` mouse ownership/verification entries for the Tab panel and Library panel main Selector row, and verify the entries name local hover ownership plus their focused and live-tick tests.

## 4. Visual evaluation and tuning

- [x] 4.1 Run mbv in a terminal that reports passive pointer movement and evaluate the initial tab and main Selector-row pill hover styles in Narrow and Wide Panel modes: move across adjacent targets, gaps, selected targets, and out to another visible surface; verify resting < hover < selected remains legible without flicker or accidental actions.
- [x] 4.2 Record the evaluation result in this task: if the initial composition of existing semantic roles has the desired feel, mark it accepted; otherwise tune only the central tab/pill style policies, adding a dedicated semantic hover role only if the existing vocabulary cannot express the chosen appearance, then repeat task 4.1 before proceeding.

## 5. Whole-change verification

- [x] 5.1 Run `cargo fmt` and verify `cargo fmt --all -- --check` plus `cargo check -p mbv` pass.
- [x] 5.2 Run `cargo clippy --workspace --all-targets -- -D warnings` and fix findings introduced by this change.
- [x] 5.3 Run `cargo nextest run -p mbv` and confirm the hover, exclusion, breakpoint, and existing mouse behavior remain green.
- [x] 5.4 Sync the applied `mouse-input` delta into `openspec/specs/mouse-input/spec.md`, validate the change strictly, and verify the change is ready to archive without implementing hover on any excluded target.
