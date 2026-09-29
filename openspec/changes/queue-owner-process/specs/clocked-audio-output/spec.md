## MODIFIED Requirements

### Requirement: Packaged mbvd defaults to clocked ALSA output
Packaged `mbvd` SHALL use clocked ALSA device output when `audio_pipe_enabled` is absent or false. Owner-local configuration SHALL accept an `audio_device` value equal to `alsa` or beginning with `alsa/`; an absent value SHALL resolve to `alsa`. Local-daemon output defaults SHALL remain unchanged.

#### Scenario: Packaged daemon uses inherited output
- **WHEN** packaged `mbvd` starts a Playback run without an explicit pipe selection or ALSA device
- **THEN** the run uses the default ALSA device
- **THEN** it does not configure mpv as a PCM file writer or create a FIFO for that run

#### Scenario: Packaged daemon uses a selected ALSA endpoint
- **WHEN** packaged `mbvd` starts a Playback run with `audio_pipe_enabled = false` and `audio_device = "alsa/hw:Loopback,0,0"`
- **THEN** that run selects exactly `alsa/hw:Loopback,0,0`

#### Scenario: Another Player owner starts playback
- **WHEN** the Local daemon starts a Playback run without an explicit audio-device setting
- **THEN** its existing audio-output selection remains unchanged
