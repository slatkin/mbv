## Why

Packaged `mbvd` currently uses mpv's untimed `ao=pcm` file writer when sending audio to a FIFO. Its blocking writes can delay startup and pause handling by roughly two seconds before downstream playout latency is added. A clocked ALSA device gives mpv a real-time audio-output boundary while preserving the FIFO path for installations that still require it.

## What Changes

- Make clocked ALSA device output the inherited output mode for packaged `mbvd`.
- Add owner-local ALSA device selection for routing packaged-daemon audio to a physical or virtual ALSA device such as `snd-aloop`.
- Keep `audio_pipe_enabled = true` as the explicit legacy selector and preserve the existing pipe path, format, startup guard, latency estimate, and diagnostics in that mode.
- Do not create, load, expose, or configure ALSA loopback devices, and do not configure or restart Snapserver or Snapclient; document those as operator-owned deployment prerequisites.
- Verify that accepted pause and resume commands reach mpv promptly in clocked-output mode and that clocked startup does not enter the pipe startup guard.
- **BREAKING**: packaged `mbvd`'s inherited non-pipe output is forced to ALSA instead of libmpv auto-selection; installations using the PCM FIFO must keep or set `audio_pipe_enabled = true`.

## Capabilities

### New Capabilities

- `clocked-audio-output`: Defines packaged-daemon ALSA output selection, its default behavior and control responsiveness, and isolation from operator-owned audio routing.

### Modified Capabilities

- `pipe-playout-latency`: Restricts pipe startup phases, buffering estimates, and guards to explicitly enabled legacy pipe output while preserving their existing behavior.

## Impact

- Affects packaged-daemon configuration and validation, player runtime construction in `mbv-core`, mpv property projection, startup/pause integration tests, example configuration, and operator documentation.
- Retains existing pipe configuration keys and their next-session application boundary, including the active `manage-mbvd-settings` change's `audio_pipe_enabled` control.
- Adds no dependency and no ctrl or shared-data protocol version change.
- Requires deployment coordination outside mbv when ALSA output feeds Snapcast: the host must provide the selected ALSA endpoint, and Snapserver must consume its corresponding capture endpoint.
