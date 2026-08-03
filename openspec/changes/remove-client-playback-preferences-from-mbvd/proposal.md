## Why

`mbvd` currently consumes preferences that belong to the controlling mbv client, which makes daemon-targeted playback behave differently from local or generic Emby-targeted playback. Separating client policy from daemon execution is required before daemon settings can be safely managed as operational settings only.

## What Changes

- Stop daemon-host configuration and runtime paths from applying `always_play_next`, `always_skip_intro`, `subtitle_mode`, `subtitle_lang`, and `audio_lang`.
- Make mbv expand `always_play_next` requests before dispatching them to local, direct-daemon, or attached Emby playback targets.
- Make mbv react to neutral intro-boundary reports using its own `always_skip_intro` preference, retaining the existing prompt when skipping is disabled.
- Carry the controlling client's subtitle and audio-language preferences to direct `mbvd` playback rather than seeding selection from the daemon host.
- Preserve equivalent client-preference behavior for local and direct-daemon playback, without changing generic attached-Emby control semantics.
- Remove `always_play_next` from the pending daemon-settings-management allowlist because it is a client preference, not a daemon operational setting.

## Capabilities

### New Capabilities

- `client-playback-preference-routing`: Client-owned playback policies and media-selection preferences are applied by mbv and transmitted to execution targets when needed.

### Modified Capabilities

- `ctrl-protocol`: Direct-daemon control carries client playback-selection preferences without allowing daemon-host defaults to substitute for them.

## Impact

- Affects configuration loading and daemon bootstrap, playback request construction, intro event handling, direct ctrl protocol payloads and capability negotiation, and player session initialization.
- Affects the pending `manage-mbvd-settings` change by removing `always_play_next` from its daemon-owned allowlist.
- Requires focused local/direct-daemon parity tests and daemon-host configuration isolation tests; no new dependency is introduced.
