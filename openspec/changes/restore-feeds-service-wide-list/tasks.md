## 1. Establish scope and baseline

- [x] 1.1 Confirm implementation starts from accepted HEAD `9005b80` and targets the Feeds Service/tab Wide panel only; record that #634/#637's Emby homevideos feed-view fix is separate.
- [x] 1.2 Capture current threshold behavior with metadata/state-bearing fixtures at width 82 and a larger Wide size, proving W1/W2/W3 rather than relying on blank or metadata-free output.

## 2. Correct Wide rendering

- [x] 2.1 Change the Feeds Wide arrangement/render call so it always uses one column; leave non-hero catalog column policy untouched.
- [x] 2.2 Restore the existing semantic surface/backdrop and border treatment for the Feeds Wide right rail using current arrangement/theme conventions.
- [x] 2.3 Correct `feed_row.rs` selected/active/played geometry: one title, contiguous full-row background, and aligned markers without multi-column drift or hero double-title.
- [x] 2.4 Preserve Narrow behavior; add or retain a Narrow regression fixture and change Narrow code only if a failing test proves it necessary.

## 3. Verification and delivery

- [x] 3.1 Add focused buffer/geometry tests for W1, W2, and W3 at width 82 and a larger Wide width with metadata-bearing selected, played, and active entries.
- [x] 3.2 Run the relevant Feeds tests and `rtk make check-code-file-lines`; split any touched source file before it exceeds 800 lines.
- [x] 3.3 Run `rtk openspec validate restore-feeds-service-wide-list --strict` and attach the output to the implementation review.
- [x] 3.4 Land this as an independent docs/implementation slice before `migrate-home-feeds-to-canonical-lists`; do not fold #634/#637, #640, or PR #606 sequencing.

> Acceptance note (2026-09-01): user visually confirmed the Feeds Service Wide panel. The repository size gate still reports only the pre-existing `src/app/shell_home.rs` 804-line violation. A two-space list-row indent follow-up is deferred to the canonical source-of-truth slice.
