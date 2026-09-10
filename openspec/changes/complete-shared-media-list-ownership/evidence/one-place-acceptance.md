# One-place acceptance probe

Change: `complete-shared-media-list-ownership`, task 9.1 (design.md D7-5)
Baseline: `0fbfc697` (`refactor/finish-canonical-media-list-ownership`)
Worktree: `/home/slatkin/Dev/mbv/.worktrees/finish-canonical-media-list-ownership`

## Verdict

**PASS.** A single test-only row-local transition plus one decoration, added
solely inside `src/app/components/media_list/`, is exercised through four
existing destination routing paths (Wide, Inline, Grid, provider workspace)
with **no destination production edit of any kind** — not even one line in a
destination Interactive Component or its render painter. The probe was reverted
before commit; only this evidence file remains.

## Probe contents (test-only, `#[cfg(test)]`)

All probe code lived in `src/app/components/media_list/mod.rs` and
`src/app/components/media_list/tests.rs`. No file outside
`src/app/components/media_list/` was modified, and the existing presentation
**render painters were not touched either** — the decoration travels in the
shared row's own `primary` text, which the existing `wide_media_row` painter
already paints for Wide, Inline, and Grid.

- **One row-local state transition** (`MediaList::delegate`): every delegated
  movement input advances a test-only `probe_flash_count` and records the row
  the movement left behind. Wired through the existing
  `delegate` / `RowLocalOutcome` path; no new outcome variant, so no destination
  translation changed.
- **One decoration derived from it** (`MediaList::advance_probe_flash` and
  `reapply_probe_flash`): the flashed row's `primary` text is prefixed with the
  test-only `PROBE_FLASH_PREFIX` (`*`). `set_content` re-derives the decoration
  after a destination content refresh so the list-local state survives
  reprojection. Painted unchanged by the existing presentation painters.

No destination production component, shell file, or render painter was edited
to surface either the transition or the decoration.

## Commands and results (probe applied)

| Command | Result |
|---|---|
| `df -h /tmp` | `/tmp` 16G, 9.9G available (sufficient) |
| `cargo fmt --all -- --check` | PASS |
| `cargo check -p mbv` | PASS |
| `cargo check -p mbv --all-targets` | PASS |
| `cargo nextest run -p mbv acceptance_probe` | **4 passed**, 1528 skipped |
| Probe-disabled mutation (`PROBE_FLASH_PREFIX = ""`), `cargo nextest run -p mbv acceptance_probe` | **0 passed, 4 failed** — the tests genuinely depend on the shared transition/decoration |
| Probe restored, `cargo nextest run -p mbv acceptance_probe` | **4 passed** |

Mutation detail: temporarily emptying only the shared decoration constant made
all four destination-routed tests fail (`expected shared-subsystem decoration
"*<row>"`), then restoring it made them pass again. The tests therefore assert
the shared-subsystem behaviour, not the fixture.

## Flows exercised (all through the real shell `Application::tick()` harness)

`cargo nextest run -p mbv acceptance_probe` — one test per flow, each driving
the mounted destination through `TickHarness` (inject event -> `step()` ->
`draw_frame`) and reading the painted `TestBackend` buffer.

| Flow | Destination / presentation | Delegated input | Buffer assertion | Result |
|---|---|---|---|---|
| Wide | Home destination, `WideMediaList` rail (160-col) | `Down` -> `HomeComponent::delegate_row_local_input(Move(1))` -> `MediaListCarrier::delegate` | painted frame contains `*Home Item 0` (the row left by the move); absent before the transition | PASS |
| Inline | Feeds destination, `InlineMediaBrowser` (81-col, below the 82 breakpoint) | `Down` -> `FeedsComponent::delegate_row_local_input(Move(1))` -> `MediaListCarrier::delegate` | painted frame contains `*One` (the row left by the move) | PASS |
| Grid | Generic Emby two-column Browser (`collection_type = "other"`, LibraryOnly, 100-col), `GridMediaList` | `Right` -> `BrowserComponent::move_cursor_delta(1)` -> `MediaListCarrier::delegate(Move(1))` | painted cell frame contains `*Focused Movie` (the row left by the move) | PASS |
| Provider workspace | TV workspace destination, series `WideMediaList` rail (160-col) | `Down` -> `TvWorkspaceComponent::move_rows(1)` -> `WideMediaList::delegate(Move(1))` | painted frame contains `*Focused Movie` (the series row left by the move), after the destination content refresh | PASS |

## Destination files I was tempted to edit (and why I did not)

1. **`src/app/components/browser/navigation.rs` / `browser/keyboard.rs`** —
   Browser `Up`/`Down`/`PageUp`/`PageDown` route through
   `MediaListCarrier::move_item_rows`, which bypasses the delegation seam, so
   the delegated transition does not fire for those chords. I considered
   rewriting the Browser's row movement to call `carrier.delegate(...)` to make
   a `Down`-key Grid probe work, but that is exactly the destination production
   edit the acceptance criterion forbids. Instead the Grid flow uses the
   Browser's existing delegated column chord (`Left`/`Right` ->
   `move_cursor_delta` -> `carrier.delegate`), which the destination already
   routes. The Browser's `move_item_rows` routing is pre-existing destination
   behaviour and out of scope for a disposable probe.
2. **`src/app/components/home.rs` / `feeds.rs` / `tv_workspace/`** — all were
   driven only from the new tests via their already-public row-local delegation
   paths; no destination accessor or hook was added.
3. **`src/app/render/components/media_list/wide_row.rs` / `wide.rs`** — the
   probe could have added a decoration parameter to `wide_media_row`, but that
   painter edit was unnecessary: decorating the shared row's `primary` text is
   painted by the existing painter unchanged.

## Revert

The probe was reverted with:

```sh
git checkout -- src/app/components/media_list/mod.rs src/app/components/media_list/tests.rs
```

Post-revert verification:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check -p mbv` | PASS |
| `cargo nextest run -p mbv` | **1528 passed** (back to the baseline count; the four probe tests are gone) |
| `git status --short` | clean except `openspec/changes/complete-shared-media-list-ownership/evidence/one-place-acceptance.md` |
| `git diff --stat` on `src/` | empty — no probe code survives |
