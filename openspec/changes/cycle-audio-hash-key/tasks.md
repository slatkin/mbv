# Tasks

## 1. Replace the action (design D1)

- [x] 1.1 In `crates/mbv-keybinds/src/registry.rs`, rename the `toggle_mute_or_cycle_audio` declaration to `cycle_audio` and set `default_chords: &["#"]`, keeping its section, gate, policy and flags. In `crates/mbv-keybinds/src/tests.rs`, update the two places that list the action: the identifier list and the default-chord table (`("cycle_audio", &["#"])`). Verify with `cargo nextest run -p mbv-keybinds`.
- [x] 1.2 In `src/app/input/key_policy.rs`, rename the policy entry name, `KeyPolicyBinding::ToggleMuteOrCycleAudio` and its `command_for_policy` arm to `cycle_audio` / `CycleAudio` / `Command::CycleAudio`. In `src/app/dispatch/action.rs`, rename `Command::ToggleMuteOrCycleAudio` to `Command::CycleAudio`, make its arm call only `self.cycle_audio()`, and rewrite its doc comment. Also drop the reference to the old variant from `ToggleMute`'s doc comment. In `src/app/shell/playback.rs`, make `PlaybackRequest::CycleAudio` dispatch `Command::CycleAudio`. In `crates/mbv-render/src/components/help.rs`, rename the label key to `"cycle_audio"` (label stays "Cycle audio track"). Verify with `cargo check -p mbv`.

## 2. Delete the orphaned mute half (design D2)

- [x] 2.1 Delete `App::toggle_mute` (`src/app/dispatch/audio_subtitle.rs`) and `App::is_audio_item` with its doc comment (`src/app/dispatch/music.rs`). Delete `PlaybackTarget::is_audio_item` and `PlaybackTarget::toggle_soft_mute` (`src/app/state/playback_target.rs`), along with `is_audio_item` and `toggle_soft_mute` on `LocalPlaybackTarget`, `RemotePlaybackTarget` and `CastPlaybackTarget`. Also delete the cast doc comment that explains routing `a` to mute. Delete the two tests in `src/app/dispatch/actions/route_tests/audio_session.rs` that call them, plus `make_remote_session` if nothing else uses it. Keep `pre_mute_volume`. Verify with `cargo clippy --workspace --all-targets -- -D warnings`.

## 3. Routing contract (spec: "`a` reaches the focused leaf during playback")

- [x] 3.1 In `src/app/tests/routing_matrix/playback.rs` `playback_policy_preserves_per_key_eligibility`, change the `a` assertion so that `#` with an active player resolves to `Command(Command::CycleAudio)`. Change the `Ctrl+a` assertion into a bare `a` with an active player resolving to `FallThrough`. In `src/app/input/key_policy/resolution_tests.rs`, rename the `"toggle_mute_or_cycle_audio"` match case to `"cycle_audio"`. Add no other tests: the registry default test and the routing matrix cover the remaining scenarios. Verify with `cargo nextest run -p mbv`.

## 4. Integration

- [x] 4.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run --workspace`. All must pass.
- [x] 4.2 The user tests live: `#` cycles audio on a video, `m` mutes, and `a` in the Music tree enqueues during playback. Mark this done only on the user's confirmation.

## Workflow follow-up

- Archive the change (sync the specs) after the user signs off. It must land before `playlist-panel-play-shuffle-enqueue` is applied.
