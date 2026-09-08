## 1. Boundary Ownership

- [x] 1.1 Add full-height trailing-edge boundary geometry to the root Queue-side column, exclude that column from root backdrop painting, and verify relational arrangement coverage plus a sentinel buffer test proving root chrome leaves the boundary untouched.
- [x] 1.2 Add the boundary Interactive Component as sole painter and gesture owner, preserve the exact focused and unfocused edge appearance, add semantic live-width and drag-end messages, and include it in Both-mode synchronization and mouse eligibility; verify focused component and buffer tests cover appearance, exact-edge arming, one-column resolution, clamping, click-only behavior, and eligibility cancellation.

## 2. Width Application

- [x] 2.1 Separate exact in-memory queue-column width updates from preference persistence while retaining the existing keyboard resize behavior; verify focused state tests show mouse movement does not save and drag end saves only a changed final width.
- [x] 2.2 Dispatch boundary messages exhaustively, recompute the frame for each distinct live width, and cancel the component when Panel mode or overlay arbitration removes eligibility; verify Queue title/list geometry does not overlap the boundary and queue-slot dragging plus panel clicks outside it retain their existing behavior.

## 3. Integration and Documentation

- [ ] 3.1 Extend the existing live-tick mouse integration coverage with a press-drag-release sequence and an overlay-suppression case, verifying exact live width, final persistence, only the boundary owner's message is delivered, and Queue/Library destinations do not handle the gesture through `Application::tick()`.
- [ ] 3.2 Update `docs/architecture/interactive-surface-ledger.md` with the boundary owner, one-column drag gesture, Both-only availability, and focused/live-tick/manual verification evidence.
- [ ] 3.3 Manually verify the boundary is visually unchanged and draggable at representative Normal and Wide terminal sizes, then run `cargo fmt`, targeted `cargo nextest` tests, `cargo check -p mbv`, `ast-grep scan`, and `make check-code-file-lines`.
