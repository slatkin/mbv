## ADDED Requirements

### Requirement: Daemon spectrum capture occurs after Snapcast buffering
For audio-pipe daemon playback, mbvd SHALL derive spectrum input from a dedicated second Snapclient synchronized by the same Snapserver as the audible Snapclient. The daemon SHALL NOT derive remote spectrum input from mpv's pre-Snapserver PCM pipe or from a PulseAudio default source.

#### Scenario: Daemon visualizer starts during audio-pipe playback
- **WHEN** the client sends `StartSpectrum` while mbvd is playing through its configured Snapserver PCM pipe
- **THEN** mbvd starts a dedicated second Snapclient that outputs raw PCM to a FIFO file and starts CAVA to read from that FIFO using `method = fifo`

#### Scenario: Snapserver buffers playback
- **WHEN** Snapserver and the audible Snapclient apply their normal playback buffering
- **THEN** the spectrum source follows Snapcast's synchronized playout timeline instead of leading it from the pre-Snapserver PCM timeline

### Requirement: Visualization does not enter the primary playback path
The dedicated visualizer Snapclient and CAVA session SHALL be independent of the primary Snapclient, hardware ALSA output, mpv PCM writer, and Snapserver source FIFO.

#### Scenario: Visualizer starts and stops normally
- **WHEN** daemon visualization is toggled on or off
- **THEN** mbvd does not change mpv's audio output properties, Snapserver's input pipe, the primary Snapclient process, or the hardware playback device

#### Scenario: Visualizer process fails
- **WHEN** the dedicated Snapclient or CAVA exits unexpectedly
- **THEN** audible playback continues through the primary Snapclient without interruption

### Requirement: Daemon spectrum prerequisites are explicit and truthful
mbvd SHALL advertise `spectrum-streaming` only when `cava` and `snapclient` are executable. mbvd SHALL NOT load kernel modules or alter persistent system configuration at runtime.

#### Scenario: All prerequisites are available
- **WHEN** a client performs the daemon control handshake and the required executables are available
- **THEN** the daemon advertises `spectrum-streaming`

#### Scenario: Required executables are absent
- **WHEN** `cava` or `snapclient` is not found in PATH
- **THEN** the daemon omits `spectrum-streaming` and logs an actionable diagnostic identifying the missing prerequisite

#### Scenario: Runtime prerequisite disappears
- **WHEN** a prerequisite was available during the handshake but starting either child later fails
- **THEN** the daemon sends one `SpectrumFailed` event with the child and failure reason and leaves playback running

### Requirement: Visualizer Snapclient identity is stable
The dedicated Snapclient SHALL use a stable, distinct instance and host identity so Snapserver can preserve its group and stream assignment across visualizer sessions. The Snapserver endpoint, Snapclient identity, and FIFO output path SHALL be configurable.

#### Scenario: Visualizer reconnects
- **WHEN** the user stops and later restarts daemon visualization
- **THEN** the dedicated Snapclient reconnects with the same identity and Snapserver restores its prior stream assignment

#### Scenario: Multiple Snapcast streams exist
- **WHEN** the Snapserver exposes multiple streams
- **THEN** documentation identifies that the dedicated visualizer client must be assigned to the same stream as the audible client

### Requirement: Spectrum children have one supervised lifecycle
mbvd SHALL supervise the dedicated Snapclient, CAVA, frame reader, and private resources as one idempotently stoppable spectrum session.

#### Scenario: Session startup succeeds
- **WHEN** both children remain healthy through startup readiness and CAVA produces valid frames
- **THEN** mbvd streams those frames through the existing `CtrlEvent::Spectrum` path

#### Scenario: Session startup partially fails
- **WHEN** one child starts but the other child fails readiness
- **THEN** mbvd terminates and reaps the started child, removes private resources, and sends one `SpectrumFailed` event

#### Scenario: Session stops
- **WHEN** mbvd receives `StopSpectrum`, playback stops, the controlling client disconnects, or the daemon shuts down
- **THEN** mbvd terminates and reaps both children, joins reader threads, and removes private resources without changing primary playback

### Requirement: Spectrum failure does not cause an automatic retry loop
After a daemon spectrum session fails, mbv SHALL keep visualization failed for that activation and SHALL NOT resend `StartSpectrum` on each synchronization tick.

#### Scenario: CAVA exits immediately
- **WHEN** the client receives `SpectrumFailed` after enabling daemon visualization
- **THEN** it clears the visualizer frame and sends no further `StartSpectrum` command until the user explicitly toggles visualization again or a new playback session resets the failure state

### Requirement: Visualizer Snapclient setup is documented
Installation documentation SHALL describe configuring the dedicated visualizer Snapclient identity and assigning it to the audible Snapcast stream via Snapserver's web UI or configuration.

#### Scenario: Operator prepares a daemon host
- **WHEN** an operator follows the daemon visualizer setup documentation
- **THEN** the operator can verify the visualizer Snapclient appears in Snapserver and assign it to the correct stream before enabling visualization in mbv
