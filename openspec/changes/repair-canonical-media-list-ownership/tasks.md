# Repair Canonical Media-List Ownership

## 1. Establish the Component-View Seam

- [x] 1.1 Confirm landed PR #683 (`6b58a608`) painted-owner arbitration, one-row wheel, throttle, routing, and real-`Application::tick()` tests pass before changing shared list code.
- [x] 1.2 Reopen after Grouped Music recon: implement plain TuiRealm `Component` bounds for `WideMediaList` and `InlineMediaBrowser`, semantic-only paint policy, parent-configured claim and row-flow rectangles, and retained read-only results; verify compile-time bounds and focused result lifecycle tests before, during, after, and after empty/zero-area view.
- [x] 1.3 Reopen after Grouped Music recon: route existing row primitives through each control's one component view, preserve parent panel framing, claim, and row-flow placement, retain current claim/content/selected/detail facts, and resolve a later point without caller-supplied geometry; retain compatibility behavior for untouched destinations and verify representative Wide and Inline buffer tests.

## 2. Prove the Wide Seam in Queue

- [x] 2.1 Convert Queue to invoke its persistent `WideMediaList<QueueSlotId>` view once in every panel mode and consume its retained result; remove Queue row-rectangle/selectable-map reconstruction while preserving scope-pill and drag gesture authority; verify Queue component, drag, and buffer tests.
- [x] 2.2 Verify Queue click, context, drag, and scroll resolve current painted `QueueSlotId` rows, and retain PR #683 one-row wheel, throttle, painted-owner arbitration, and live-`Application::tick()` evidence.

## 3. Prove Wide and Inline Seams in Grouped Music

- [x] 3.1 Convert Grouped Music's framed Wide album rail to invoke its persistent `WideMediaList` view once and consume only the retained result; remove its uniform selectable-map reconstruction while preserving grouped content, images, Inline Search, parent framing, and scroll behavior; verify existing Wide buffer and component tests.
- [x] 3.2 Convert Grouped Music's framed Wide track table to invoke its persistent `WideMediaList` view once and resolve track rows from the retained result; remove the uniform track hit map while preserving provider-owned track and gesture authority; verify focused track, mouse, and buffer tests.
- [x] 3.3 Convert Grouped Music's Inline album presentation to invoke its persistent `InlineMediaBrowser` view once and paint provider detail only in the retained admitted-detail rectangle; remove selectable-map reconstruction while preserving admission/fallback, responsive anchor handoff, grouping, images, and Inline Search; verify existing Inline buffer and component tests.
- [x] 3.4 Verify Grouped Music clicks, double clicks, right clicks, wheel, and breakpoint anchor behavior use current retained geometry across Wide and Inline presentations; add relevant real-`Application::tick()` mouse tests.

## 4. Verify and Accept the Seam

- [x] 4.1 Run focused media-list, Queue, Grouped Music, render-characterization, and real-`Application::tick()` mouse tests; confirm PR #683 painted-owner arbitration, one-row wheel, throttle, routing, and Playlists regression evidence remains passing.
- [x] 4.2 Run `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `ast-grep scan`, and `make check-code-file-lines`; fix all failures before acceptance.
- [ ] 4.3 Have a human verify Queue fixed rows and Grouped Music Wide album rail, Wide track table, and Inline behavior, plus focused-sidebar wheel arbitration; record the result or an explicit human waiver before acceptance.
- [ ] 4.4 Defer responsive active-control ownership, Browser/Home identity and grid work, TV/Feeds/Audiobookshelf migration, universal ratchets, documentation sweep, and multi-select to follow-on changes under issue #681; sync and archive only after implementation, acceptance, and final OpenSpec validation.
