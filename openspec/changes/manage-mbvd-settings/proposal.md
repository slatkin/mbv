## Why

Packaged `mbvd` is headless, so changing playback-runtime options currently requires editing host-side configuration and restarting the process even when the option could naturally take effect at the next playback boundary. After #441 removes client-owned playback preferences from daemon behavior, mbv can provide a focused management surface for genuinely daemon-owned runtime settings without remotely rewriting bootstrap configuration.

## What Changes

- Add a `DAEMON` scope to the existing F2 settings panel, selected through a `LOCAL` / `DAEMON` pill bar. `LOCAL` retains the existing client-side settings behavior.
- Manage packaged `mbvd` only. The hidden `mbv --__local-daemon` stay-alive implementation does not expose or consume managed overrides.
- Initially manage exactly `use_mpv_config`, `no_scripts`, `audio_pipe_enabled`, `audio_pipe_path`, `audio_pipe_samplerate`, `audio_pipe_bitdepth`, `audio_pipe_playout_delay_ms`, and `progress_interval_secs`.
- Exclude bootstrap, networking, security, and every option that requires restarting `mbvd`.
- Store daemon-wide overrides as one typed, versioned document with optimistic revision checks, independently of per-user roaming documents and without rewriting `config.toml`.
- Resolve each value as either `override` or `inherited`. Resetting a setting removes its override and reveals the value inherited from the daemon's ordinary configuration/default resolution.
- Apply runtime overrides without restarting `mbvd`: playback-session settings take effect on the next playback session, while playout delay is captured when the next pipe playback intent is accepted. Issue #442 separately reevaluates whether playout-delay UX should continue to exist.
- Validate and durably commit each mutation before acknowledging it. Reject stale writes without mutation and return the current snapshot rather than silently overwriting another client's update.
- Serialize edits through a client-side mutation queue with one request in flight. Later queued actions continue from each acknowledged revision; unsent actions are discarded with feedback on disconnect.
- Reuse the existing shared-data connection and authentication boundary. Every authenticated shared-data user is intentionally trusted to manage the packaged daemon; this change adds no administrator roles or second authorization system.

## Capabilities

### New Capabilities

- `daemon-settings-management`: Discovery, display, editing, persistence, conflict handling, and next-playback application of allowlisted packaged-`mbvd` runtime overrides.

### Modified Capabilities

None.

## Impact

- Depends on #441 so client playback preferences are no longer consumed as daemon-host configuration.
- Extends the shared-data protocol with additive capability-negotiated daemon-settings requests, responses, subscriptions, and notifications.
- Adds a daemon-wide override record to the existing `redb` database and storage worker without adding it to the per-user roaming document set or changing shared-data export.
- Introduces a typed runtime-settings model distinct from the monolithic `Config` structure.
- Extends F2 settings state, rendering, keyboard handling, and mouse hit-testing with the `LOCAL` / `DAEMON` scope pill bar and typed daemon-setting editing.
- Adds runtime plumbing so the eight allowlisted options can change at their next playback boundary without daemon restart.
- Adds no new external dependency and changes neither the ctrl nor shared-data protocol version.
