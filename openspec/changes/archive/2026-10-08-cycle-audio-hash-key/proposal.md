# Proposal

## Why

The `a` key runs `toggle_mute_or_cycle_audio`. On an audio item it mutes by setting the volume to zero. On a video it cycles the audio track. `a` is easy to press by accident, and the user then cannot tell what changed. `a` is also a router chord gated only on active playback, so while anything plays it takes `a` away from every focused leaf. The Music tree's `a` (Enqueue) is swallowed during playback today, and the planned Playlists panel `a` (Enqueue, change `playlist-panel-play-shuffle-enqueue`) would be swallowed too.

## What Changes

- **BREAKING (keybinding):** The `toggle_mute_or_cycle_audio` action is deleted. A config that names it fails to load as an unknown action.
- New `cycle_audio` action with the default chord `#`, which is mpv's own cycle-audio key. It only cycles the audio track, using the existing per-target `cycle_audio` behaviour. It never runs the mute half.
- `m` (`toggle_mute`) stays the only mute key. Its behaviour does not change.
- No router chord is bound to `a`. A focused leaf receives `a` in every playback state, so the Music tree's Enqueue works during playback.
- The playback panel's audio-indicator request (`PlaybackRequest::CycleAudio`) dispatches the new `cycle_audio` command. It no longer runs the mute-or-cycle branch.
- The mute half of the old action is deleted because nothing else calls it: the soft-mute (volume-to-zero) toggle and the audio-item check that chose between the two halves.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `configurable-keybinds`: Adds a requirement that audio-track cycling and mute are separate actions, sets the `cycle_audio` default to `#`, and keeps `a` free of router bindings so leaves receive it.

## Impact

- `crates/mbv-keybinds/src/registry.rs` and `tests.rs`: the action declaration and the default-chord table.
- `src/app/input/key_policy.rs`: the policy entry, the binding, and the command mapping.
- `src/app/dispatch/action.rs`: the `Command` variant and its dispatch arm.
- `src/app/shell/playback.rs`: the `PlaybackRequest::CycleAudio` arm.
- `crates/mbv-render/src/components/help.rs`: the help label key.
- Dead-code removal: `src/app/dispatch/audio_subtitle.rs` (`toggle_mute`), `src/app/dispatch/music.rs` (`is_audio_item`), and `src/app/state/playback_target.rs` plus `local.rs`, `remote.rs` and `cast.rs` (`is_audio_item`, `toggle_soft_mute`).
- Tests: `src/app/tests/routing_matrix/playback.rs`, `src/app/input/key_policy/resolution_tests.rs`, `src/app/dispatch/actions/route_tests/audio_session.rs`.
- No protocol, persistence, or config schema changes. `pre_mute_volume` stays, because `cycle_audio`'s audio-off step still uses it.
