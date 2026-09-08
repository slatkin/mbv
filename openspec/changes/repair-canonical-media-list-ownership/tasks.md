# Repair Canonical Media-List Ownership

## 1. Establish the Component-View Seam

- [ ] 1.1 Confirm landed PR #683 (`6b58a608`) painted-owner arbitration, one-row wheel, throttle, routing, and real-`Application::tick()` tests pass before changing shared list code.
- [ ] 1.2 Implement plain TuiRealm `Component` bounds for `WideMediaList` and `InlineMediaBrowser`, semantic-only paint policy, and retained read-only results; verify compile-time bounds and focused result lifecycle tests before, during, after, and after empty/zero-area view.
- [ ] 1.3 Route existing row primitives through each control's one component view; derive all list rectangles internally, retain current claim/content/selected/detail facts, and resolve a later point without caller-supplied geometry; verify representative Wide and Inline buffer tests.

## 2. Prove the Wide Seam in Queue

- [ ] 2.1 Convert Queue to invoke its persistent `WideMediaList<QueueSlotId>` view once in every panel mode and consume its retained result; remove Queue row-rectangle/selectable-map reconstruction while preserving scope-pill and drag gesture authority; verify Queue component, drag, and buffer tests.
- [ ] 2.2 Verify Queue click, context, drag, and scroll resolve current painted `QueueSlotId` rows, and retain PR #683 one-row wheel, throttle, painted-owner arbitration, and live-`Application::tick()` evidence.

## 3. Prove Wide and Inline Seams in Feeds

- [ ] 3.1 Convert Feeds Wide presentation to invoke its persistent `WideMediaList` view once, including framed inset behavior, and consume only the retained result; remove selectable-map reconstruction while preserving selectors/filter/feed detail; verify existing Wide buffer and component tests.
- [ ] 3.2 Convert Feeds Inline presentation to invoke its persistent `InlineMediaBrowser` view once and paint feed detail only in the retained admitted-detail rectangle; verify existing admission/fallback, point-resolution, and Inline buffer tests.
- [ ] 3.3 Verify Feeds row clicks, double clicks, and scroll use current retained geometry while its existing responsive lockstep movement remains unchanged; verify focused component and relevant real-`Application::tick()` mouse tests.

## 4. Verify and Accept the Seam

- [ ] 4.1 Run focused media-list, Queue, Feeds, render-characterization, and real-`Application::tick()` mouse tests; confirm PR #683 painted-owner arbitration, one-row wheel, throttle, routing, and Playlists regression evidence remains passing.
- [ ] 4.2 Run `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `ast-grep scan`, and `make check-code-file-lines`; fix all failures before acceptance.
- [ ] 4.3 Have a human verify Queue fixed rows and Feeds Wide/Inline behavior, plus focused-sidebar wheel arbitration; record the result or an explicit human waiver before acceptance.
- [ ] 4.4 Defer responsive active-control ownership, Browser/Home identity and grid work, TV/Music/Audiobookshelf migration, universal ratchets, documentation sweep, and multi-select to follow-on changes under issue #681; sync and archive only after implementation, acceptance, and final OpenSpec validation.
