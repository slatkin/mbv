## 1. Red-before-green now-playing buffer test

- [x] 1.1 Add a failing buffer test for the queue now-playing row asserting the new geometry (title with no inline `%`, no elapsed, right-aligned `glyph pct` with the aqua throbber role) alongside an unchanged resume-progress row (inline `%`, duration intact), and verify it fails against the current painter before any production edit.
- [x] 1.2 Pin the browser resume path as the regression net: confirm existing tests covering `emby_semantic_state` / `Active` rendering pass untouched throughout, and verify with `cargo nextest run -p mbv browser`.

## 2. `NowPlaying` row vocabulary

- [ ] 2.1 Add `MediaSemanticState::NowPlaying { progress }` in `src/app/components/media_list/mod.rs` reusing the bounded `ActiveProgress`, leaving the `Active` variant and its constructor semantics unchanged, and update every exhaustive `match` on the enum (compiler-guided; resume/music/ABS/feeds/grouping sites keep current behavior). Verify with `cargo check -p mbv`.
- [ ] 2.2 Add the aqua throbber theme role over the AQUA primitive in `src/app/render/theme/mod.rs` (raw primitive stays private to `theme/`), and verify no other theme role value changes with `cargo check -p mbv`.

## 3. Painter: now-playing geometry with paint-time throbber

- [ ] 3.1 In `wide_media_row` (`src/app/render/components/media_list/wide_row.rs`), add the `NowPlaying` arm (no inline progress; right slot renders `glyph pct` as aqua-glyph + FOAM-pct spans replacing the duration, neither span using the green duration role; throbber alone when progress is `None`; `Collection` kind suppresses the slot) plus a trailing `throbber: Option<char>` parameter, leaving the `Active` arm byte-identical, and assert all eight braille glyphs are single-column with narrow/zero-reserve buffer cases (long titles, scrollbar, focused selection, throbber-only, `glyph 100%`). Verify the 1.1 test now passes.
- [ ] 3.2 Thread the throbber parameter through `render_wide_media_list` and the internal `wide_media_row` call inside `render_inline_media_browser` (queue never uses the inline path — it passes `None`), updating all non-queue callers to pass `None`, and verify with `cargo check -p mbv` plus the wide-row regression suite.

## 4. Revision authority in `mbv-core`

- [ ] 4.1 Enumerate every production `PlaybackQueue` mutator (`crates/mbv-core/src/playback_queue.rs`) and make `QueueRevision` authoritative: add missing bumps on every row-affecting path (`update_slot_item`, `apply_progress`, refresh merge, any other unbumped mutator found), invalidate on queue replacement, and pin it with a mutation-matrix test asserting each row-affecting mutator advances the generation. Verify with `cargo nextest run -p mbv-core playback_queue`.

## 5. Queue projection: no elapsed, `NowPlaying`, fingerprint gate

- [ ] 5.1 In `queue_media_rows` (`src/app/components/queue.rs`), project the active row as `NowPlaying` with live bucketed progress and `duration: None` (stop calling `queue_row_time_text` with `show_elapsed=true`; inactive rows unchanged; predicted-selection projects throbber-only with no progress), and verify with `cargo nextest run -p mbv queue`.
- [ ] 5.2 Split `QueueComponent::set_content` row delivery from cursor/chrome delivery: add separate row-replace/row-patch, cursor-command, and scope/chrome/title setters so the fingerprint gate touches only row projection and cursor pushes plus current chrome are delivered on every tick. Verify existing queue cursor/scope tests pass unchanged with `cargo nextest run -p mbv queue`.
- [ ] 5.3 Add the fingerprint gate in `sync_queue` (`src/app/shell_queue.rs`): compute `(viewed_scope, revision, playback.active, projected active target, progress-% bucket)` from `&PlayerTab` before cloning, skip slot clone/row projection/row push on the no-change path (cursor/chrome still delivered), and add a target-based in-place now-playing row patch on `ListCore` for the progress-bucket-only change (full rebuild only on fingerprint change beyond the bucket). Verify with tick-level tests asserting no clone/projection/push on an unchanged tick and a one-row patch on a bucket change, plus a two-scope same-revision/same-slot-ID stale-row test, plus `cargo nextest run -p mbv queue`.
- [ ] 5.4 Add local/remote/cast transition tests: paused freezes the glyph, active-to-idle clears `NowPlaying` and hides the glyph, empty queue and invalid active index never patch, predicted `ItemSelected` renders throbber-only, `Collection` rows suppress the slot. Verify with `cargo nextest run -p mbv queue`.
- [ ] 5.5 Resolve the paint-time glyph in the shell (existing 300 ms `now_playing_throbber_index`, already gated on active && !paused; `None` when nothing is playing), store it on `QueueComponent` via the dedicated setter from 5.2 (never part of the row fingerprint), and pass it through `view()` into `render_wide_media_list`. Verify with buffer tests.

## 6. Gates and docs

- [ ] 6.1 Run the full gates and verify green: `cargo check -p mbv`, `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `cargo fmt --all -- --check`, `ast-grep scan`, `make check-code-file-lines`.
- [ ] 6.2 Record the deferred follow-up (now-playing styling in non-queue lists) and the `render_plain_rows` non-goal in `CONTEXT.md` only if those terms are already defined there; otherwise leave docs untouched and note the follow-up in the change summary.
