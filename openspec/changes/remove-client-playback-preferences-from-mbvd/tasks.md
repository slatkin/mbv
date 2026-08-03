## 1. Establish Client-Owned Playback Policy

- [x] 1.1 Trace every `Player::new` caller and remove `always_play_next` / `always_skip_intro` as daemon execution inputs, preserving only the interactive client's local policy state.
- [x] 1.2 Refactor `src/app/actions.rs` so a single app-level request-preparation path reads the controlling client's `always_play_next`, resolves following episodes once, sets the series queue source, and dispatches the resulting sequence unchanged to local, direct-daemon, or attached-session targets.
- [x] 1.3 Update `crates/mbv-core/src/player_runtime.rs` so intro-boundary detection always emits `PlayerEvent::IntroStarted` and never seeks based on a player or daemon configuration value.
- [x] 1.4 Update `src/app/player_event.rs` so the client handles `IntroStarted`: when its `always_skip_intro` is enabled, send `SeekAbsolute(intro_end)` and `SkipIntroDismiss`; otherwise preserve the current prompt and notification behavior.
- [x] 1.5 Add focused tests proving a direct-daemon episode request receives the client-expanded sequence, a disabled preference submits no additional episodes, and local/direct targets share the same expansion result.
- [x] 1.6 Add core and app tests proving intro detection is neutral, client-enabled auto-skip sends seek plus dismissal, and client-disabled playback retains the manual prompt flow.

## 2. Remove Daemon-Host Preference Consumption

- [x] 2.1 In `crates/mbv-core/src/daemon_run.rs`, stop constructing daemon `SubtitlePrefs` from daemon-host `subtitle_mode`, `subtitle_lang`, or `audio_lang`; remove the daemon-host Emby preference fetch and start with neutral track-selection state.
- [x] 2.2 Remove any remaining daemon runtime reads of daemon-host `always_play_next` and `always_skip_intro`, including packaged `mbvd` and hidden local-daemon startup paths.
- [x] 2.3 Retain the five fields in shared `Config` for interactive mbv, but verify the daemon-specific bootstrap path does not consume them as daemon configuration.
- [x] 2.4 Add daemon startup tests using conflicting daemon-host client preferences to prove they neither append episodes, auto-seek intros, nor select audio/subtitle tracks.

## 3. Synchronize Direct-Daemon Track Preferences

- [x] 3.1 Reuse `WireCommand::SetSubtitlePrefs` as the ordered direct-daemon synchronization mechanism; do not add a ctrl version bump or a new payload unless implementation demonstrates the existing command cannot establish ordering safely.
- [x] 3.2 Initialize a new direct `RemotePlayer` from the controlling app's current subtitle mode, subtitle language, and audio language values, then enqueue preference synchronization before its first playback command on the same ctrl sender.
- [x] 3.3 Ensure `push_subtitle_prefs` and the existing subtitle/audio settings actions continue to update the active direct-daemon target immediately while preserving local-player behavior.
- [x] 3.4 Add ctrl/remote-player tests that assert preference synchronization precedes the first `PlayItems` or playback-intent command and that subsequent preference changes update the daemon's track-selection state.
- [x] 3.5 Add an integration-level regression test showing a direct daemon follows the controlling client's synchronized preferences rather than conflicting daemon-host values.

## 4. Reconcile Related Planning and Verify

- [x] 4.1 Update `openspec/changes/manage-mbvd-settings/{proposal,design,tasks}.md` to remove `always_play_next` from the daemon settings allowlist, model, runtime application table, and related tasks.
- [x] 4.2 Run `cargo fmt --check`, targeted core/player/ctrl/app tests, and the complete `cargo test` suite.
- [x] 4.3 Manually validate local and direct-daemon playback with differing client and daemon-host preferences: episode expansion, intro handling, initial track selection, and a live track-preference update.

  Evidence recorded 2026-08-03 in this worktree:
  - `cargo test -p mbv --bin mbv client_playback_sequence_ -- --nocapture` passed 2/2: client-enabled episode expansion is sent to direct playback, disabled expansion sends only the requested item, and the shared preparation path is target-independent for local/direct playback.
  - `cargo test -p mbv --bin mbv intro -- --nocapture` passed 3/3: client-enabled intro handling emits seek plus dismissal; disabled handling retains the manual prompt. The core intro-boundary test also passed via `cargo test -p mbv-core --lib intro -- --nocapture` (1/1).
  - `cargo test -p mbv-core --lib daemon_ -- --nocapture` passed 15/15, including conflicting daemon-host preference startup isolation and controlling-client track preferences applied by the daemon.
  - `cargo test -p mbv-core --lib subtitle_preferences_are_ordered -- --nocapture` passed 1/1: `SetSubtitlePrefs` precedes direct playback on the real `RemotePlayer` command queue.
  - `cargo test -p mbv --bin mbv track_preferences -- --nocapture` passed 2/2: initial direct-daemon synchronization and live preference updates are sent from the controlling client.
  - Limitation: no live Emby server, media fixture, or mpv session was available, so this validates the repository's app/ctrl/daemon execution paths and command observations rather than audible/video playback against a real server.
