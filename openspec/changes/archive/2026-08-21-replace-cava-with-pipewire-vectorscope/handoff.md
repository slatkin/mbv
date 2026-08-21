# Handoff

## Change

`replace-cava-with-pipewire-vectorscope`

Implementation is paused at `19/22` tasks. The three unchecked tasks are live-environment verification only: 6.1, 6.2, and 6.3.

## Implemented

- Added `pipewire = "0.10.1"` to the workspace and `mbv-core`, plus PipeWire package dependencies in Debian metadata, `PKGBUILD`, and CI.
- Replaced the Cava child/FIFO parser with `PipeWireWorker`, a PipeWire main-loop capture stream, interleaved stereo F32LE parsing, non-blocking bounded sample publication, generation tracking, failure reporting, explicit disconnect, join, and cleanup.
- Added bounded-buffer tests for newest complete stereo pairs and worker lifecycle behavior.
- Replaced app Cava state with PipeWire state while preserving local, same-host Local daemon, remote, and audio-pipe eligibility rules.
- Added persisted `[display].visualizer_glyph` validation with default `●`, fallback for empty/control/wide values, and config tests.
- Replaced spectrum bars with latest-window stereo vectorscope rendering, left-horizontal/right-vertical orientation, clamping, deduplicated cells, silence suppression, configured glyphs, palette preservation, and 16 ms cadence.
- Added a fixed internal 4x display gain before coordinate mapping so typical PCM levels produce a useful visual spread without changing captured samples or adding UI configuration.
- Added coordinate-mapping and stale-cell clearing render tests.
- Removed Cava source, metadata, README, CI, current ADR/spec references, and the obsolete current daemon-spectrum spec. Historical archives and this change's planning artifacts intentionally retain historical Cava references.

## Verification

Fresh final checks passed:

- `rtk cargo nextest run -p mbv-core`: 527 passed.
- `rtk cargo nextest run -p mbv`: 906 passed.
- `rtk cargo clippy --workspace --all-targets`: 0 errors, 24 existing warnings.
- `rtk cargo fmt --all -- --check`: passed.
- `rtk make check-code-file-lines`: passed.
- `rtk openspec validate --changes "replace-cava-with-pipewire-vectorscope"`: passed.
- `rtk cargo metadata --no-deps --format-version 1`: passed.
- `rtk makepkg --printsrcinfo`: passed and shows `pipewire` as a runtime dependency.
- `rtk git diff --check`: passed.
- `pw-dump --no-colors` confirmed a live PipeWire 1.6.8 session with stereo default-output monitor ports.
- An independent read-only code review was dispatched for the uncommitted diff; its result was not available before this pause.

`cargo deb` is unavailable in this environment. A `bash -n` check was not applied to the YAML workflow because YAML is not shell syntax.

## Remaining Tasks

### 6.1 Live PipeWire/bare playback

Use real mbv playback and PipeWire logs to prove default-sink capture, unrelated application audio contribution, toggle-off stream release, and non-fatal PipeWire failure. Do not mark complete from the worker smoke test alone.

### 6.2 Playback ownership paths

Verify bare playback, same-host Local daemon playback, remote daemon/attached-session playback, and audio-pipe playback. Confirm same-host Local daemon works without daemon or ctrl-protocol changes, while remote and audio-pipe paths do not start local capture.

### 6.3 Render-boundary freshness

On a terminal capable of the target cadence, measure at least 50 fresh vectorscope frames per second during steady audio and verify that a forced UI stall recovers to the newest sample generation rather than replaying queued frames.

## Next Entry Point

1. Re-read `proposal.md`, `specs/system-audio-visualizer/spec.md`, `design.md`, and `tasks.md`.
2. Run `rtk openspec instructions apply --change "replace-cava-with-pipewire-vectorscope" --json` and confirm the `18/21` state.
3. Perform tasks 6.1 through 6.3 with real playback and terminal-boundary instrumentation.
4. Mark each task only after its evidence is captured, then rerun the final checks above.
5. If all 21 tasks are complete, report implementation completion and offer `/opsx-archive`.

## Worktree Note

The worktree also contains an untracked `.serena/memories/` path. Do not remove or alter it without confirming ownership.
