## 1. The filter

- [ ] 1.1 Replace `audio_only_rejection` in
      `crates/mbv-core/src/daemon_core.rs:565` with an admission filter over
      `&[MediaItem]` returning the admitted items and a discard count. Keep it a
      pure function with no `Player`/`EmbyClient` argument, for the same
      testability reason the doc comment above the current function gives. Reuse
      `MediaItem::is_audio` (`api_types.rs:115`) via the existing `all_audio`
      helper (`daemon_ws.rs:172`) — do not write a second audio predicate.
- [ ] 1.2 Keep a wholly non-audio submission rejecting with today's `AudioOnly`
      reason string. Mixed becomes admissible; wholly non-audio does not. The
      rejection is a live path, not dead code.
- [ ] 1.3 Add a start-index remap helper: given the original index and which
      positions were admitted, return the index of the first admitted item at or
      after the original position, else the last admitted item. This is NOT the
      `start_idx.min(len - 1)` clamp already in `PlayItems` — that clamp stays
      for its own purpose but cannot substitute for the remap.
- [ ] 1.4 Verify: `cargo check -p mbv-core` clean.

## 2. The three call sites

- [ ] 2.1 Apply the filter and remap in `daemon_control.rs` `CtrlCmd::PlayItems`
      (currently rejecting at `:361`), before `*items` is assigned and before
      `play_queue`/`play` is called.
- [ ] 2.2 Apply the filter in the playback-intent path (`daemon_run.rs:559`),
      preserving the existing intent accept/reject/coalesce sequencing around it.
- [ ] 2.3 Apply the filter in the ws path (`daemon_ws.rs:35`) so Emby-started
      playback is admitted on the same terms.
- [ ] 2.4 Log every discard with its count on all three paths. Do NOT send a
      ctrl notification — reporting discards over ctrl is out of scope
      (design.md, Non-Goals).

## 3. Tests

- [ ] 3.1 Unit-test the admission filter: wholly audio, mixed, wholly non-audio,
      empty.
- [ ] 3.2 Unit-test the start-index remap: index on an admitted item, index on a
      discarded item with admitted items after, index on a discarded item with
      none after, all discarded.
- [ ] 3.3 Update the existing `daemon_tests.rs` cases that reference
      `audio_only_rejection` and `all_audio` to the new shape. The
      `audio_only_rejection(true, mixed)` case changes meaning — it now admits a
      subset rather than rejecting.
- [ ] 3.4 Verify: `cargo test -p mbv-core` passes.

## 4. Close out

- [ ] 4.1 Verify: `cargo clippy --workspace --all-targets` clean.
- [ ] 4.2 Verify: `make check-code-file-lines` passes. `daemon_core.rs` is 669
      lines and `daemon_control.rs` 482 — neither is near the 800 cap, so no
      split is expected. Check rather than assume.
