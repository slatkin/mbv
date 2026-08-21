## 1. Shared Queue Visual Slot

- [ ] 1.1 Update the persisted visualizer boolean and `v` input path to represent artwork/visualizer selection in all playback contexts while retaining the existing on-disk preference key.
- [ ] 1.2 Make queue-card rendering use its existing reserved artwork rectangle for the selected visualizer, including an empty visualizer when capture has no samples and the existing loading reservation while artwork is pending.
- [ ] 1.3 Keep PipeWire worker start/stop synchronized with visualizer selection and the existing local, active-playback, and audio-pipe eligibility guards.

## 2. Layout Simplification

- [ ] 2.1 Remove the bottom-of-queue visualizer reservation, renderer, and separator so those rows return to the queue list.
- [ ] 2.2 Remove the wide queue-only playback-panel visualizer branch and leave unused playback-panel rows on their existing background.
- [ ] 2.3 Remove layout-only visualizer constants/imports made unused by the shared queue-card placement.

## 3. Behavior Checks

- [ ] 3.1 Update the existing visualizer input/lifecycle tests to cover persisted selection, attached-session toggling without local capture, and artwork selection stopping capture.
- [ ] 3.2 Extend the existing queue-card tests with the smallest focused cases that distinguish selected visualization, confirmed missing artwork, and pending artwork without adding full-screen snapshots.
- [ ] 3.3 Manually verify Both, Queue-only narrow, Queue-only wide, Mini Queue, images-off, idle, local playback, and remote playback presentations at representative terminal sizes.

## 4. Documentation And Verification

- [ ] 4.1 Amend ADR 0009 and user-facing help text to describe `v` as queue artwork/visualizer selection and remove descriptions of the old separate placement.
- [ ] 4.2 Run the focused app tests, package check, workspace Clippy, code-file line check, and strict OpenSpec validation for this change.
