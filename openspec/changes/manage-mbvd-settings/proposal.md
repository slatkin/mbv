## Why

Packaged `mbvd` is headless, so changing its operational behavior currently requires editing host-side configuration and restarting or otherwise administering the daemon outside mbv. Now that `mbvd` has an opt-in shared-data service, mbv can provide a small, explicit management surface without remotely rewriting the daemon's bootstrap configuration.

## What Changes

- Add a `DAEMON` scope to the existing F2 settings panel, selected through a `LOCAL` / `DAEMON` pill bar. `LOCAL` retains the existing client-side settings behavior.
- Expose an explicitly allowlisted daemon-settings snapshot containing effective values, their source (`override`, `config`, or `default`), and how each change takes effect.
- Initially manage `always_play_next`, `broadcast_ms`, and `audio_pipe_playout_delay_ms`; all other configuration remains outside the remote management surface.
- Store daemon-wide overrides as one typed, versioned document with optimistic revision checks, independently of per-user roaming documents and without rewriting `config.toml`.
- Resolve effective daemon settings in the order compiled default, local daemon config, then stored override. Resetting a setting removes its override and reveals the inherited value.
- Validate and durably commit an override document before acknowledging a change. Reject stale writes and return the current snapshot rather than silently overwriting another client's update.
- Report whether an accepted setting is active immediately, on the next playback, or after daemon restart; persisted values that are not active yet remain visibly pending.
- Reuse the existing shared-data connection and authentication boundary. This change does not introduce administrator roles or a second authorization system.

## Capabilities

### New Capabilities

- `daemon-settings-management`: Discovery, display, editing, persistence, conflict handling, and application status for allowlisted daemon-wide setting overrides.

### Modified Capabilities

None.

## Impact

- Extends the shared-data protocol with additive capability-negotiated daemon-settings requests, responses, and notifications.
- Adds a daemon-wide override record to the existing `redb` database and storage worker without adding it to the per-user roaming document set.
- Introduces a resolved daemon-settings model distinct from the monolithic `Config` structure.
- Extends F2 settings state, rendering, keyboard handling, and mouse hit-testing with the `LOCAL` / `DAEMON` scope pill bar and typed daemon-setting editing.
- Adds runtime plumbing for settings that can apply without restarting while preserving startup-captured behavior for restart-required settings.
- Adds no new external dependency and does not change the ctrl protocol version.
