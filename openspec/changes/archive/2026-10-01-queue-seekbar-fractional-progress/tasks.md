# Tasks

## 1. Queue-only fractional seekbar slice

Deliver this group as one bounded implementation slice with its owned tests. Production
scope is `crates/mbv-render/src/components/chrome_player/title/queue_band.rs`; test scope is
its new `queue_band/tests.rs` child and the affected existing Queue panel test file. No
Library production code, shared whole-cell helper, theme values, or dependencies change.

- [x] 1.1 Replace the active Queue seekbar's shade spans with the existing Unicode Gauge, using the raw clamped tick ratio, empty label, accent foreground, and `#272e33` track background inside the unchanged bar rectangle; preserve label formatting, spacing, inactive rendering, and the zero-width fallback. In the same slice, move affected glyph/colour/time/paint-geometry assertions from `crates/mbv-components/src/queue_playback_panel/tests.rs` to render-owned `title/queue_band/tests.rs`, remove orphaned helpers, and add a focused regression for 10 seconds of 45 minutes in a 24-cell bar. Verify with `cargo check -p mbv-render` and `cargo nextest run -p mbv-render -p mbv-components`: the early edge is one eighth filled, labels and bar geometry remain intact, and no Gauge percentage or border appears. Cite this change for the new regression and preserve `7fdb7fee8` provenance for migrated assertions.
- [x] 1.2 Complete the same slice's contract checks: verify repainting a later Queue position back to the early position clears obsolete accent fill; verify invalid runtime yields an empty bounded bar and completion fills without spilling; add a focused Render Component scope guard that the Library still paints its thin upper-line seekbar with whole-cell rounding. Reuse existing Interactive Component semantic seek tests, adapting the fixture to exercise a partial-cell click and label non-targets rather than reasserting glyphs there. Verify with `cargo nextest run -p mbv-render -p mbv-components`, `cargo fmt --all -- --check`, and `cargo clippy -p mbv-render -p mbv-components --all-targets -- -D warnings`; no sleeps, real player, live server, per-eighth Gauge test suite, snapshots, or lint suppressions.

## 2. Review and acceptance

Depends on the complete, green implementation slice in group 1. These are integration
and acceptance checks, not a deferred test-authoring phase.

- [x] 2.1 Review the diff against the delta spec and confirm Queue-only production scope, unchanged `seek_fill()` and Library painter, one Queue bar painter, unchanged pointer fractions, and no protocol, layout, reporting, configuration, or dependency changes; verify the review records no unresolved findings and `openspec validate queue-seekbar-fractional-progress --strict` passes.
- [x] 2.2 Obtain user-run or explicitly authorized manual acceptance in the user's terminal: inspect early progress on an episode and a movie, pause, seek backward, resize through short-bar and time-only widths, and switch to Library-only playback to confirm its thin bar is unchanged. Verify solid accent fill and muted track look correct, flanking labels remain readable, bar clicks still seek, and time labels do not. Record results in this change's tasks artifact; do not launch playback, manipulate the live desktop, or capture screenshots without separate permission.
Manual acceptance: user reported **“all good”** after the Queue track update to `#272e33`, confirming the row 2.2 checklist on the revised build. No agent-run live playback or desktop actions were performed.

- [ ] 2.3 After implementation review and manual acceptance pass, sync the delta into `openspec/specs/queue-playback-panel/spec.md` and archive the change through OpenSpec; verify the durable spec contains the accepted Queue-only requirements, archive validation succeeds, and no planning task remains unchecked. Do not push or open a PR without a separate user request.
