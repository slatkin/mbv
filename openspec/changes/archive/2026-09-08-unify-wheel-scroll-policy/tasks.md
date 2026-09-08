## 1. Establish the shared wheel boundary

- [x] 1.1 Characterize the existing signed one-notch gesture behavior and expose the minimum canonical-list claim seam needed by parent components; verify the focused gesture/list tests prove one-step direction and painted-region rejection.
- [x] 1.2 Convert Browser (including narrow TV), Home, Queue, and wide TV to local one-step wheel movement; remove `HomeScroll`, `BrowserScroll`, `TvScroll`, the shell Queue wheel mover, and their dead tests; verify focused component tests plus a real `Application::tick()` test prove the correct mounted owner changes locally without a shell relay.

## 2. Convert destination workspaces

- [x] 2.1 Convert Music and Feeds to direct one-step local movement through their painted canonical-list claims; verify their existing mouse tests cover both directions and a boundary without page/column multiplication.
- [x] 2.2 Convert Audiobookshelf podcast and book surfaces to one-step movement using their painter-published irregular-row geometry; verify focused component tests cover their show/book/chapter focus behavior at the applicable narrow and wide arrangements.
- [x] 2.3 Make Inline Search move its own result viewport by one step after it takes first refusal from its host; verify a focused search-host test proves the host list remains unchanged.

## 3. Convert sidebar and viewport surfaces

- [x] 3.1 Convert Settings and Help to one-line local viewport movement as focus-owned sole overlays, independent of pointer position; update or replace only the existing focused tests that would otherwise preserve the three-line behavior.
- [x] 3.2 Convert Sessions and Playlists to one-step local list movement as focus-owned sole overlays, independent of pointer position; verify focused mouse tests cover selection, boundaries, and an off-panel pointer.

## 4. Record and verify the contract

- [x] 4.1 Update `docs/architecture/interactive-surface-ledger.md` and the mouse-input delta to distinguish pointer-gated competing surfaces from focus-owned sole overlays and record breakpoint evidence; verify every affected ledger row is coherent with the implemented owner.
- [x] 4.2 Run `cargo fmt`, focused `cargo nextest run -p mbv` filters for the changed mouse/component suites, `cargo check -p mbv`, `ast-grep scan`, and `make check-code-file-lines`; manually exercise one narrow and one wide list plus a focused sidebar to confirm one-step direction, boundaries, canonical-list outside-region rejection, and pointer-independent sidebar scrolling.
