# Design

## Context

See proposal.md (Why). The current code has this shape:

- `toggle_mute_or_cycle_audio` (`crates/mbv-keybinds/src/registry.rs`) is declared with the default chord `a` and `KeyGate::Playback`. Its policy entry (`src/app/input/key_policy.rs`) uses `KeyPolicyGate::Playback`, which `playback_allowed` opens whenever a player is active or a remote target exists. The gate does not check which leaf has focus. While anything plays, the router returns `Command(ToggleMuteOrCycleAudio)` for `a` and drops the focused leaf's message (`router.rs`). That is why the Music tree's `a` (Enqueue) is dead during playback.
- `Command::ToggleMuteOrCycleAudio` (`src/app/dispatch/action.rs`) calls `toggle_mute()` (soft mute: `ui_volume` → 0, saved in `pre_mute_volume`) when `is_audio_item()` is true. Otherwise it calls `cycle_audio()`. Each playback target (`state/playback_target/{local,remote,cast}.rs`) implements `is_audio_item`, `toggle_soft_mute` and `cycle_audio`. Remote `toggle_soft_mute` already delegates to `cycle_audio`. Cast `toggle_soft_mute` delegates to command mute, and cast `cycle_audio` flashes "not supported".
- Local `cycle_audio` already includes mpv's audio-off step (track id 0), and it uses `pre_mute_volume` to restore the volume.
- `m` → `toggle_mute` → `Command::ToggleMute` is a separate mute path (`mute_on` / `SetMute`). This change does not touch it.
- `PlaybackRequest::CycleAudio`, sent by the playback panel leaf, dispatches `Command::ToggleMuteOrCycleAudio` (`src/app/shell/playback.rs`).
- The user's own config does not name `toggle_mute_or_cycle_audio`.

## Goals / Non-Goals

**Goals:**
- One action per effect. `#` cycles audio, matching mpv's default key. `m` mutes.
- `a` is free of router bindings, so leaf `a` actions work during playback.

**Non-Goals:**
- A config migration or alias for the old identifier. This is a single-user app, and the user's config does not use it.
- Unifying the two mute mechanisms. `m` keeps `SetMute`, and `cycle_audio`'s audio-off step keeps the volume-zero path.
- The playback panel leaf's hardcoded `<` → `CycleAudio` and `>` → `CycleSubtitle` keys (`crates/mbv-components/src/library_playback_panel.rs`). They fire only when the router leaves `<` and `>` alone. Aligning them with mpv is a separate decision.
- A broader mpv-parity audit of the other playback defaults.

## Decisions

### D1. Rename in place: `cycle_audio` / `Command::CycleAudio` / `KeyPolicyBinding::CycleAudio`

Every layer keeps its position and gate (`KeyGate::Playback`, `KeyPolicyGate::Playback`, `global: false`). Only the identifier, the default chord (`a` → `#`) and the dispatch arm change. The arm calls `self.cycle_audio()` and nothing else. `#` is parsed as `Key::Char('#')`, the same way as the existing symbol defaults `<` and `>`, so routing matches it the same way.

*Alternative:* keep the identifier and change only the default chord. Rejected, because the name would still describe a mute half that no longer exists.

### D2. Delete the orphaned mute half

When the dispatch arm stops calling them, `App::toggle_mute` (`audio_subtitle.rs`), `App::is_audio_item` (`music.rs`) and `PlaybackTarget::{is_audio_item, toggle_soft_mute}` plus their three per-target impls have no callers left. `-D warnings` forces their removal. The two tests in `src/app/dispatch/actions/route_tests/audio_session.rs` that exercise them (`is_audio_item_falls_back_…` and `toggle_mute_falls_back_to_cycle_audio_…`) are deleted with them. `fetch_album_tracks_is_a_no_op_when_already_loading` in the same file stays. `pre_mute_volume` and its persistence stay, because `cycle_audio` and `sync_volume_from_player` still use them.

### D3. User-visible consequences, accepted

- On a cast target, `#` flashes the existing "not supported" message. The old `a` muted on cast. `m` still mutes there.
- On a local audio item, `#` cycles the audio track. With a single track this alternates between that track and audio-off, which is mpv's behaviour.

## Risks / Trade-offs

- [Muscle memory: `a` no longer mutes or cycles] → This is intended. `m` mutes and `#` cycles.
- [`#` needs Shift on most layouts] → This is intended, because it makes an accidental press less likely.
