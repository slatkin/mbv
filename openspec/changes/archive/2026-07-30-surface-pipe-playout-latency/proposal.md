## Why

When `mbvd` writes PCM to a pipe, downstream buffering can leave a substantial gap between mbv starting output and sound becoming audible. This niche deployment needs honest progress visibility and duplicate-send protection during that gap without making mbv responsible for Snapserver or any other downstream consumer.

## What Changes

- Expose playback startup phases mbv can directly observe for pipe output.
- Add an optional, generic expected downstream playout delay used only for estimation and startup guarding.
- Show output-buffering state, with approximate remaining delay when configured.
- Keep equivalent same-item Play intents guarded until estimated output buffering settles.
- Record phase timestamps for diagnosing startup latency.
- Document manual calibration and the estimate's limits.
- Explicitly exclude downstream discovery, API integration, configuration, control, and automatic tuning.

## Capabilities

### New Capabilities

- `pipe-playout-latency`: Read-only phase reporting and optional downstream playout-delay estimation for direct `mbvd` pipe output.

### Modified Capabilities

None.

## Impact

- Builds on `reliable-daemon-playback-intents`.
- Affects daemon playback phase reporting, direct-daemon client status, rendering, configuration, logging, and user documentation.
- Applies only to direct `mbvd` pipe output; other routes retain their existing presentation.
- Adds no Snapserver API dependency and no external dependency.
