## Context

`mbvd` controls embedded mpv, but mpv produces the audio. On the deployed daemon host, mpv writes raw PCM to `/tmp/mbv-pipe`, Snapserver reads that pipe, and a primary Snapclient later renders the synchronized stream to the hardware DAC. The released daemon visualizer instead starts CAVA with `method = pulse`; that assumes an accessible user audio server and fails when launched by the root system service.

CAVA supports both FIFO and ALSA input. Reading mpv's existing FIFO directly is invalid because POSIX FIFOs distribute bytes among readers rather than broadcasting them: CAVA and Snapserver would each receive incomplete PCM. The capture point must be on the Snapcast playout side of that latency boundary.

The existing spectrum control protocol and renderer are suitable. This change corrects only how daemon-side CAVA obtains audio and how support is advertised and supervised.

## Goals / Non-Goals

**Goals:**

- Capture the same Snapcast stream on the synchronized playout timeline used by audible playback.
- Avoid any dependency on a PulseAudio or PipeWire user session in `mbvd.service`.
- Keep visualization entirely outside the primary playback path.
- Make missing CAVA or Snapclient support visible before the client enables the feature.
- Prevent immediate child failure from causing repeated start/fail cycles.
- Preserve the existing `StartSpectrum`, `StopSpectrum`, `Spectrum`, and `SpectrumFailed` wire protocol.

**Non-Goals:**

- Managing the primary Snapclient or changing its hardware device.
- Automatically changing Snapserver groups or stream assignments through its control API.
- Supporting daemon spectrum streaming when playback does not use Snapcast.
- Providing sample-accurate synchronization between sound and terminal rendering; CAVA analysis, network delivery, and terminal rendering still add a small post-playout delay.
- Changing local mbv visualization, which continues to use its existing system-audio input.

## Decisions

### Capture a dedicated synchronized Snapclient with FIFO output

On `StartSpectrum`, mbvd starts a second Snapclient connected to the configured Snapserver. That client uses a stable identity (`--hostID puffin-balls` by default, configurable) and a distinct instance number (`--instance 2`). It outputs raw PCM to a FIFO file (`/tmp/mbv-spectrum.fifo` by default) using Snapclient's file output mode (`-o file`). CAVA uses `method = fifo` and reads from that same FIFO.

```text
mpv --ao=pcm
      │
      ▼
Snapserver
      │ synchronized encoded stream
      ├──────────────────────▶ primary Snapclient ─▶ hw:0 ─▶ audible output
      │
      └──────────────────────▶ spectrum Snapclient ─▶ /tmp/mbv-spectrum.fifo
                                                        │
                                                        ▼
                                                   CAVA (fifo input)
                                                        │
                                                        ▼
                                                   spectrum protocol
```

Snapcast performs clock synchronization for both clients, so the FIFO receives samples at the intended playout time rather than when mpv originally wrote them. CAVA and UI processing occur after that point and may introduce modest visual lag, but the visualizer no longer leads by the Snapcast buffer.

### Why FIFO instead of ALSA loopback?

The original design considered using ALSA's `multi` plugin to duplicate the primary Snapclient's output to both the DAC and a loopback device. This approach was rejected because:

1. **No isolation**: The `multi` plugin uses `snd_pcm_link()` to synchronize all slaves at the kernel level. Any XRUN on the loopback propagates to the DAC.
2. **CAVA failure risks primary playback**: If CAVA is slow, absent, or crashes, the loopback buffer fills, causing an XRUN that glitches the primary DAC output.
3. **Complexity**: Requires `snd_aloop` kernel module, ALSA configuration, and careful buffer management.

The second Snapclient with FIFO output approach:
- Completely isolated from primary playback
- No kernel modules or ALSA configuration required
- Simple FIFO-based data flow
- Independent failure domains

### Why a second Snapclient instead of tapping the pipe?

The second Snapclient:
- Connects to Snapserver independently
- Receives the same synchronized audio stream
- Outputs to a FIFO file instead of an ALSA device
- CAVA reads from that FIFO
- Completely isolated from primary playback

Alternatives rejected:

- **CAVA reads `/tmp/mbv-pipe` with Snapserver:** invalid because two FIFO readers split bytes.
- **PCM relay before Snapserver:** safe with careful fan-out, but spectrum leads audible playback by Snapcast latency unless a separate delay model is maintained.
- **PulseAudio/PipeWire monitor:** does not work reliably from the root system service and is unrelated to the actual mpv-to-Snapcast data path.
- **Change the primary Snapclient to an ALSA multi-output route:** rejected because ALSA `multi` plugin links all slaves with `snd_pcm_link()`, causing XRUNs on the loopback to propagate to the DAC.

### Give the visualizer Snapclient a stable identity

The child uses a distinct instance number and stable host ID. Snapserver stores group and stream assignment by client identity, allowing the operator to assign the visualizer client to the audible stream once and have that assignment restored on later starts. The daemon does not infer or mutate Snapserver grouping, because doing so would require a new control-plane integration and ambiguous policy on multi-stream servers.

Configuration covers:

- Snapserver host and stream port;
- Snapclient instance and stable host ID (default: `puffin-balls`);
- FIFO output path (default: `/tmp/mbv-spectrum.fifo`).

The exact key placement should follow the repository's existing configuration conventions. Defaults target a local Snapserver.

### Complete independence guarantees

The two Snapclients are completely independent:
- **Separate processes**: Each is its own process with its own PID, memory space, and lifecycle
- **Separate Snapserver connections**: Each maintains its own TCP connection to Snapserver on port 1704
- **Separate output targets**: Primary writes to ALSA device (DAC), second writes to FIFO file
- **Separate synchronization**: Each independently syncs to Snapserver's clock
- **No shared resources**: No shared buffers, configuration, or state
- **Independent failure domains**: If second Snapclient crashes, primary continues unaffected

### Supervise Snapclient and CAVA as one spectrum session

`SpectrumState` owns both child guards, the CAVA frame reader, stop signaling, and temporary resources. Startup has a bounded readiness period and captures child stderr so immediate failures identify which command and device failed. A partial startup is rolled back before reporting failure.

Stopping remains idempotent and is triggered by the existing lifecycle events: `StopSpectrum`, playback stop, controlling-client disconnect, and daemon shutdown. Neither child is daemonized; both remain direct children that mbvd can terminate and reap.

### Advertise capability from a prerequisite probe

The released implementation advertises spectrum support unconditionally and waits for `SpectrumFailed`. The corrected implementation advertises it only if static prerequisites are present at handshake time. Startup still repeats validation because devices and services can disappear after negotiation.

The probe must not claim that stream assignment is correct; that can only be established by receiving meaningful PCM. A running but misassigned visualizer Snapclient can therefore produce silence. Documentation and diagnostics must make the stable Snapclient identity discoverable for assignment in Snapserver.

### Latch a remote spectrum failure per activation

The client currently records `visualizer_failed`, but the remote start branch does not consult it, allowing `StartSpectrum` to be sent repeatedly after `SpectrumFailed`. Remote startup uses the same failure latch as local startup. An explicit user toggle or a new playback activation clears the latch; periodic synchronization does not.

## Risks / Trade-offs

- [Dedicated Snapclient is assigned to the wrong stream] → Use a stable visible identity (`puffin-balls` by default), document one-time assignment, and report sustained silence separately from process failure where practical.
- [Terminal bars lag audible playback] → Capture occurs at synchronized playout, eliminating the large pre-Snapcast lead; keep CAVA polling and event delivery bounded and measure residual lag during verification.
- [Second Snapclient consumes CPU and network bandwidth] → Run it only while visualization is active and stop it promptly with the spectrum session.
- [Snapserver is remote or unavailable] → Make host and port configurable; isolate connection failure from the primary Snapclient and playback.
- [Capability probe becomes stale] → Revalidate at `StartSpectrum` and send one actionable `SpectrumFailed` response.
- [Non-Snapcast daemon playback receives no support] → Omit the capability when required daemon audio-pipe/Snapcast configuration is not enabled.

## Migration Plan

1. Add daemon spectrum configuration with defaults for local Snapserver, FIFO path, and Snapclient identity.
2. Extend CAVA worker configuration so local mode retains its current input while daemon mode selects FIFO input.
3. Add the supervised spectrum Snapclient child and combine its lifecycle with CAVA in `SpectrumState`.
4. Replace unconditional capability advertisement with prerequisite-aware advertisement and repeat validation on start.
5. Fix the client failure latch so a failed activation does not retry automatically.
6. Document and manually verify stable Snapclient assignment, FIFO data flow, audible continuity, synchronization, and cleanup on the target daemon host.

Rollback restores the previous daemon CAVA input behavior without touching the primary Snapclient or mpv pipe.

## Open Questions

- What residual spectrum lag is acceptable after CAVA analysis and terminal rendering, and should verification establish a numerical bound?
- Which existing mbv configuration section should own the Snapserver endpoint and FIFO path keys?
- Can the installed Snapclient expose a reliable readiness signal, or should readiness be defined as surviving a bounded grace period while CAVA begins receiving non-silent samples?
