# Fix Home Media-List Ownership Tasks

## 1. Characterize Home ownership

- [ ] 1.1 Add or update focused component/media-list tests proving stable-target ordinary refresh, local clamp, explicit re-anchor, and `ViewportAnchor` handoff between the Wide and Inline controls; verify the targeted tests pass.
- [ ] 1.2 Add Wide AND Normal/Narrow shell-tick integration coverage through `Application::tick()` and the shell sync pass proving navigation paints the same row, ordinary refresh preserves the stable target, a breakpoint flip preserves target plus offset via one anchor, row clicks resolve the painted target, and the one-painter counters assert exactly one list painter per frame; verify the targeted tests pass.

## 2. Transfer Home list authority

- [ ] 2.1 Refactor `HomeComponent` so only the active persistent control moves on keyboard, wheel, and pointer input, the Inline control is `&mut` with persisted narrow scroll, the `view()` breakpoint handoff transfers one `ViewportAnchor`, and the render `resolved_section` writeback is removed; verify component tests cover movement and handoff without inactive-control synchronization.
- [ ] 2.2 Refactor Home shell projection and Wide/Inline render paths so rendering only configures geometry/paint policy and paints through child `Component::view`, never reseeds content/position or writes painter-derived position back; delete the live `continue_cursor` mirror (CW effects and context menu resolve from the component's selected stable target) and the dead `latest[i].3` per-pill cursor storage with its `merge_home_sections` preservation sub-behavior; verify retained control geometry resolves row hits and Home-owned sections/pills/hero/images/effects/persistence remain unchanged.

## 3. Verify the bounded repair

- [ ] 3.1 Run relevant Home component, render, and shell-tick tests at Wide and Normal/Narrow breakpoints; verify the base frame does not underpaint the mounted Home surface and exactly one list painter runs per frame.
- [ ] 3.2 Run `cargo fmt --all -- --check`, `cargo check -p mbv`, relevant `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `ast-grep scan`, and `make check-code-file-lines`; record any unrelated failure without widening this change.
