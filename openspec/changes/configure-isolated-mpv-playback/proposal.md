## Why

The default `use_mpv_config = false` path isolates embedded mpv safely, but it also leaves video playback on mpv's `hwdec=no` default and gives users no way to tune mbv's deliberately large video cache for their machine and network. Enabling the complete user mpv configuration is too broad a workaround because it also imports unrelated profiles, scripts, and behavior.

## What Changes

- Set `hwdec=auto-safe` for video-window playback when user mpv configuration is disabled, allowing mpv to select a safe hardware decoder and fall back to software decoding.
- Continue letting the user's `mpv.conf` control hardware decoding when `use_mpv_config = true`.
- Add mbv configuration for the video forward and retained/back demuxer-cache budgets.
- Preserve the current video defaults of 50 MiB forward and 100 MiB retained/back.
- Keep the headless audio cache fixed at 10 MiB forward and 10 MiB retained/back; video-cache settings do not affect headless playback.

## Capabilities

### New Capabilities
- `mpv-playback-policy`: Defines isolated embedded-mpv hardware-decoding behavior, user-config precedence, and playback-class ownership of mpv settings.

### Modified Capabilities
- `video-feed-playback-buffering`: Makes the existing video-window cache budgets user-configurable while preserving their current defaults and feed source behavior.

## Impact

- `crates/mbv-core/src/config_types_paths.rs`, `config_parse.rs`, and `config_save.rs` gain two persisted video-cache settings and defaults.
- `crates/mbv-core/src/player_runtime.rs` applies isolated-path hardware decoding and configured video cache values while retaining fixed headless values.
- `src/app/construct.rs`, player construction/runtime configuration, and packaged configuration examples carry the new settings to each Player owner.
- Existing configuration files remain compatible; omitted settings resolve to the current video budgets.
- No queue, protocol, source-resolution, or dependency changes.
