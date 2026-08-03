## Context

See `proposal.md` and the delta specs for the behavioral contract. `Player` currently captures `always_play_next`, `always_skip_intro`, and `SubtitlePrefs` at construction. `daemon_run.rs` derives those fields from daemon-host configuration and later fetches daemon-host Emby user preferences. The interactive app already expands series playback in `actions.rs`, owns the skip-intro prompt in `player_event.rs`, and forwards live subtitle preference updates through `PlayerCommand::SetSubtitlePrefs`.

The direct control connection already serializes `WireCommand::SetSubtitlePrefs` and then playback commands, so preference synchronization can remain additive and ordered without changing framing. The ctrl version stays unchanged unless investigation finds an old same-version peer would parse a new payload with unsafe semantics; otherwise capability negotiation follows the repository ctrl protocol rule.

## Goals / Non-Goals

**Goals:**

- Make local and direct-daemon playback follow the controlling mbv client's policy and track-selection preferences.
- Keep `mbvd` an execution target: it accepts submitted queues, neutral playback events, and synchronized track-selection state without consulting client preferences from its host config.
- Preserve the current manual skip-intro flow when the client preference is disabled.
- Remove `always_play_next` from the in-progress daemon-settings-management design and task scope.

**Non-Goals:**

- Remove the client preference fields from the shared `Config` or change their existing interactive settings UI.
- Change behavior for a generic attached Emby client beyond the existing client-side request construction.
- Add a remote configuration-management API for client preferences.
- Change unrelated daemon operational settings, protocol framing, or authentication.

## Decisions

### Make the app the single policy decision point

Move all automatic episode expansion behind an app-level request-preparation path that runs before target dispatch. It must use the client config directly, not `PlayerProxy::always_play_next`, so reconnecting or switching between local and remote proxies cannot change policy ownership. The path prepares an ordered queue once and hands that queue unchanged to local player, direct daemon, or attached-session command code.

Keeping expansion in `Player` was rejected because the direct daemon has no access to the controlling client's policy and would otherwise continue to fetch episodes. Duplicating expansion in each target was rejected because target-specific branches would drift.

### Report intro boundaries neutrally and react in the app

Remove automatic seeking from the player runtime's intro handler. It always emits `IntroStarted` when the boundary is crossed and retains its existing `IntroEnded` cleanup. The app's `PlayerEvent::IntroStarted` branch reads the controlling client's `always_skip_intro`: enabled sends `SeekAbsolute` and `SkipIntroDismiss`; disabled preserves the prompt/notification path.

Letting the daemon decide was rejected because only the daemon-host config is available there. Adding a separate daemon-only auto-skip command was rejected because it duplicates the client's existing seek and dismissal mechanism.

### Synchronize only track-selection state over ctrl

Use the existing `SetSubtitlePrefs` wire command as the direct daemon's source of subtitle mode, subtitle language, and audio language. Initialize a newly connected `RemotePlayer` from the controlling client's current preferences and enqueue that command before any direct playback command. The existing preference-changing UI continues to send updates through `PlayerProxy`, making changes live for either local or remote targets.

The daemon initializes `SubtitlePrefs` to neutral defaults and removes its background daemon-host preference fetch. Extending every `PlayItems`/intent payload was rejected because selection state already has an ordered command and repeated payload fields create conflicting sources of truth.

### Remove daemon-policy fields from runtime construction

Replace daemon `Player::new` inputs for `always_play_next` and `always_skip_intro` with neutral execution values or remove the now-unused fields from the player construction API. Do the same for packaged daemon and hidden local-daemon startup paths. Retain host config only for genuine daemon operational settings.

### Keep ctrl compatibility additive

If existing `SetSubtitlePrefs` is sufficient, advertise no new mandatory capability and do not bump `CTRL_PROTOCOL_VERSION`. If a new synchronization signal is required, add it behind a capability string and preserve behavior for a peer that does not advertise it. A protocol bump is reserved only for a same-version peer that would act incorrectly.

## Risks / Trade-offs

- [An initial direct play can race preference synchronization] -> Enqueue synchronization before the first playback command on the same ordered ctrl sender and add a command-order regression test.
- [Moving episode expansion can alter queue replacement timing] -> Reuse the current series request expansion and queue-source update behavior, then cover local and direct target parity.
- [Neutral intro events can briefly display a prompt before an automatic seek] -> Handle the event synchronously in the app event loop and dismiss immediately after dispatching the seek.
- [The pending daemon-settings change still names `always_play_next`] -> Update its artifacts before implementation so its allowlist cannot reintroduce the ownership violation.

## Migration Plan

1. Remove daemon-host preference seeding and make intro events neutral behind focused core tests.
2. Establish the app-level playback-preparation path and direct-target preference synchronization with ordering tests.
3. Update daemon settings planning artifacts to exclude `always_play_next`.
4. Validate local and direct-daemon behavior, including a daemon host configured with conflicting client preferences.
5. Roll back by restoring the prior binary; no persisted data migration is required because client preferences remain in existing client config.
