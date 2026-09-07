## 1. Establish the shared wheel boundary

- [ ] 1.1 Characterize the existing signed one-notch gesture behavior and expose the minimum canonical-list claim seam needed by parent components; verify the focused gesture/list tests prove one-step direction and painted-region rejection.
- [ ] 1.2 Convert Browser (including narrow TV), Home, Queue, and wide TV to local one-step wheel movement; remove `HomeScroll`, `BrowserScroll`, `TvScroll`, the shell Queue wheel mover, and their dead tests; verify focused component tests plus a real `Application::tick()` test prove the correct mounted owner changes locally without a shell relay.

## 2. Convert destination workspaces

- [ ] 2.1 Convert Music and Feeds to direct one-step local movement through their painted canonical-list claims; verify their existing mouse tests cover both directions and a boundary without page/column multiplication.
- [ ] 2.2 Convert Audiobookshelf podcast and book surfaces to one-step movement using their painter-published irregular-row geometry; verify focused component tests cover their show/book/chapter focus behavior at the applicable narrow and wide arrangements.
- [ ] 2.3 Make Inline Search move its own result viewport by one step after it takes first refusal from its host; verify a focused search-host test proves the host list remains unchanged.

## 3. Convert sidebar and viewport surfaces

- [ ] 3.1 Convert Settings and Help to one-line local viewport movement gated by their painted content regions; update or replace only the existing focused tests that would otherwise preserve the three-line behavior.
- [ ] 3.2 Convert Sessions and Playlists to one-step local list movement gated by their painted list regions; verify the existing focused mouse tests cover list selection, boundaries, and outside-region rejection.

## 4. Record and verify the contract

- [ ] 4.1 Update `docs/architecture/interactive-surface-ledger.md` and the mouse-input delta to identify each scrollable surface's one-step behavior, painted claim region, and breakpoint evidence; verify every affected ledger row is coherent with the implemented owner.
- [ ] 4.2 Run `cargo fmt`, focused `cargo nextest run -p mbv` filters for the changed mouse/component suites, `cargo check -p mbv`, `ast-grep scan`, and `make check-code-file-lines`; manually exercise one narrow and one wide list plus a sidebar to confirm one-step direction, boundaries, and no wheel action outside the painted scroll region.
