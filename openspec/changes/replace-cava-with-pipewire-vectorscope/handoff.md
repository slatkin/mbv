# Handoff

## Change

`replace-cava-with-pipewire-vectorscope`

Implementation is complete at `22/22` tasks.

## Implemented

- Added `pipewire = "0.10.1"` to the workspace and `mbv-core`, plus PipeWire package dependencies in Debian metadata, `PKGBUILD`, and CI.
- Replaced the Cava child/FIFO parser with `PipeWireWorker`, a PipeWire main-loop capture stream, interleaved stereo F32LE parsing, non-blocking bounded sample publication, failure reporting, explicit disconnect, bounded join, and cleanup.
- Added bounded-buffer tests for newest complete stereo pairs and worker lifecycle behavior.
- Replaced app Cava state with PipeWire state while preserving local, same-host Local daemon, remote, and audio-pipe eligibility rules.
- Added persisted `[display].visualizer_glyph` validation with default `●`, fallback for empty/control/wide values, and config tests.
- Replaced spectrum bars with latest-window stereo vectorscope rendering, left-horizontal/right-vertical orientation, clamping, deduplicated cells, silence suppression, configured glyphs, palette preservation, and 16 ms cadence.
- Added a fixed internal 4x display gain before coordinate mapping so typical PCM levels produce a useful visual spread without changing captured samples or adding UI configuration.
- Added stable aqua, foam, yellow, and red point-color bands from center outward.
- Permitted Direct remote Player-owner playback to visualize audio forwarded into the local default output, while retaining attached Session and audio-pipe exclusions.
- Added coordinate-mapping and stale-cell clearing render tests.
- Removed Cava source, metadata, README, CI, current ADR/spec references, and the obsolete current daemon-spectrum spec. Historical archives and this change's planning artifacts intentionally retain historical Cava references.
- Preserved raw finite PCM amplitudes, validated negotiated F32LE stereo capture formats and SPA chunk layout/corruption, and sized the sample window from the negotiated rate.
- Added explicit CI `clang` provisioning and complete PipeWire Debian runtime dependencies.

## Verification

Fresh final checks passed:

- `rtk cargo nextest run -p mbv-core`: 530 passed.
- `rtk cargo nextest run -p mbv -E 'not test(audiobookshelf_progress_via_daemon_route_updates_queue_and_browse)'`: 906 passed, 1 excluded after that unrelated test reported `ok` and then aborted with allocator corruption in the full run.
- `rtk cargo clippy --workspace --all-targets`: 0 errors, 24 existing warnings.
- `rtk cargo fmt --all -- --check`: passed.
- `rtk make check-code-file-lines`: passed.
- `rtk openspec validate --changes "replace-cava-with-pipewire-vectorscope"`: passed.
- `rtk cargo metadata --no-deps --format-version 1`: passed.
- `rtk makepkg --printsrcinfo`: passed and shows `pipewire` as a runtime dependency.
- `rtk git diff --check`: passed.
- `pw-dump --no-colors` confirmed a live PipeWire 1.6.8 session with stereo default-output monitor ports.
- The independent review's capture and packaging findings were addressed; focused `mbv-core` clippy reports no issues.
- A live PipeWire 1.6.8 probe captured 1,584-pair windows with nonzero unrelated audio for 10 seconds on the 48 kHz default graph.
- `pw-link -l` showed the probe connected only to the default FiiO sink monitor and showed both links removed after stop.
- A real unavailable-server probe reported `failed to connect to PipeWire: Creation failed` without hanging worker shutdown.

`cargo deb` and `dpkg-deb` are unavailable in this environment. CI now inspects both generated packages and requires their standalone `pipewire` and `libpipewire-0.3-0` dependencies.

## Live Acceptance

- Real PipeWire links confirmed capture from and release of the default sink monitor, including unrelated local audio and bounded unavailable-server failure.
- Same-host Local daemon playback and Direct remote mbvd playback forwarded locally through Snapcast were verified in the running client.
- Attached Session and audio-pipe exclusions are covered by focused gate tests.
- The user directly accepted the live terminal cadence and appearance as sufficient for task 6.3; no numeric terminal-boundary measurement was recorded.

## Next Entry Point

1. Re-read `proposal.md`, `specs/system-audio-visualizer/spec.md`, `design.md`, and `tasks.md`.
2. Run `rtk openspec instructions apply --change "replace-cava-with-pipewire-vectorscope" --json` and confirm the `19/22` state.
3. Archive the completed change after the PR is accepted.

## Worktree Note

The worktree also contains an untracked `.serena/memories/` path. Do not remove or alter it without confirming ownership.
