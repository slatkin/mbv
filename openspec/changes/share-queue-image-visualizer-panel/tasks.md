## 1. Shared Queue Visual Slot

- [x] 1.1 Update the visualizer boolean and `v` input path to represent session-local artwork/visualizer selection in all playback contexts. The selection never persists: every launch starts on artwork and the on-disk preference key is no longer written or read.
- [x] 1.2 Make queue-card rendering use its existing reserved artwork rectangle for the selected visualizer, including an empty visualizer when capture has no samples and the existing loading reservation while artwork is pending. When terminal images are disabled, keep a blank artwork reservation and render the selected visualizer in the same fallback geometry without fetching artwork.
- [x] 1.3 Keep PipeWire worker start/stop synchronized with visualizer selection and the existing local, active-playback, and audio-pipe eligibility guards.

## 2. Layout Simplification

- [x] 2.1 Remove the bottom-of-queue visualizer reservation, renderer, and separator so those rows return to the queue list.
- [x] 2.2 Remove the wide queue-only playback-panel visualizer branch and leave unused playback-panel rows on their existing background.
- [x] 2.3 Remove layout-only visualizer constants/imports made unused by the shared queue-card placement.

## 3. Behavior Checks

- [x] 3.1 Update the existing visualizer input/lifecycle tests to cover session-local selection (no persistence across launches), attached-session toggling without local capture, and artwork selection stopping capture.
- [x] 3.2 Extend the existing queue-card tests with the smallest focused cases that distinguish selected visualization, confirmed missing artwork, pending artwork, and images-off selection without adding full-screen snapshots. Assert that images-off artwork and visualizer selections return identical geometry and do not fetch artwork.
- [x] 3.3 Add focused layout assertions that the queue retains rows formerly reserved below it and wide queue-only playback leftovers remain `DARK_BG` without a duplicate visualizer.
- [x] 3.4 Manually verify Both, Queue-only narrow, Queue-only wide, Mini Queue, idle, local playback, and remote playback presentations at representative terminal sizes. With images off, verify a blank artwork reservation and an in-place visualizer after pressing `v`.

## 4. Documentation And Verification

- [x] 4.1 Amend ADR 0009 and user-facing help text to describe `v` as queue artwork/visualizer selection and remove descriptions of the old separate placement.
- [x] 4.2 Run the focused app tests, package check, workspace Clippy, code-file line check, and strict OpenSpec validation for this change.
