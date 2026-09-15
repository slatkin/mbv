## 1. Preflight: the submission boundary the guard sits on

- [x] 1.1 Verify that a single-item library play while attached to a remote
      daemon submits the selection to that owner. `App::play_item`'s
      `direct_remote` branch skips `replace_playback_queue` and then submits the
      scope's existing queue (`src/app/actions.rs:283-300`); no test covers this.
      Add a unit test on a remote-player stub with a command receiver asserting
      the submitted queue contains the selected item. If it does not, fix the
      boundary minimally (submit the selection, not the stale queue) and prove
      it with the same test. Verification: the new test fails on the current
      code and passes after the fix (or passes unchanged, recorded in the task
      notes).

## 2. ctrl audio-only capability (core)

- [x] 2.1 Add the audio-only capability constant, a `CtrlHello` reader for it,
      the matching `CtrlCompatibility` flag, and its assignment during the
      handshake on the client side
      (`crates/mbv-core/src/ctrl.rs`, `remote_player/connect.rs`). Verification:
      unit tests assert the flag is set when the peer advertises the capability,
      clear when it does not, and that an unrecognized advertised capability
      still connects.
- [x] 2.2 Construct the daemon's hello from its audio-only configuration and
      send it from every ctrl listener (local Unix and TCP)
      (`crates/mbv-core/src/daemon_run.rs` / `daemon_core*`). Verification: a
      test asserts a client of an audio-only daemon receives the capability and
      a client of an audio-capable daemon does not.
- [x] 2.3 Expose the fact to the client: `RemotePlayer` accessor plus
      `PlayerProxy::owner_is_audio_only()`, false for an in-process player,
      modeled on `can_admit_audiobookshelf`
      (`crates/mbv-core/src/player/proxy.rs:82`). Verification: unit tests for
      the local and remote cases, including a remote peer that did not advertise
      it.

## 3. Emby session playable-media fact

- [ ] 3.1 Parse the session's advertised playable media types into
      `SessionInfo` alongside `supported_commands`
      (`crates/mbv-core/src/api_types.rs`, `api_client_sessions.rs:98`).
      Verification: parsing tests for present, absent, and empty values.
- [ ] 3.2 Add the `App`-level query that reads the attached session's fact
      (unknown or empty means able to play) and treats the attached owner as
      audio-only only when the session advertises audio media types only.
      Verification: unit tests over an attached session state advertising audio
      only, advertising audio and video, and advertising nothing.

## 4. Eligibility and the play guard

- [ ] 4.1 Add the eligibility + selection classification helper: attached,
      non-Library-route owner known to be audio-only, and the selection
      classified wholly unplayable, mixed, or wholly playable. Verification:
      table-driven `#[case]` tests covering ctrl-attached, Emby-session,
      Library-route, unknown-capability, wholly unplayable, mixed, and wholly
      playable inputs.
- [ ] 4.2 Wire the guard into `App::play_items_routed` and `App::play_item`
      ahead of any queue replacement, scope change, focus change, or status
      flash (`src/app/actions.rs:221`, `:257`). Verification: unit tests assert
      a wholly unplayable selection on an eligible owner raises the prompt and
      sends no command, while a wholly playable selection submits exactly what
      it submits today.
- [ ] 4.3 Report the mixed-selection count in a Neutral toast and submit that
      selection unchanged. Verification: a unit test asserts one toast naming
      the count and the same submission as before the change.
- [ ] 4.4 Pin the enqueue behavior the change deliberately keeps: an explicit
      enqueue of an unplayable selection on an eligible owner raises no prompt
      and does not append to the client's own queue. Verification: a unit test
      asserts no prompt state and the existing append submission.

## 5. Prompt and confirmed fall-through

- [ ] 5.1 Add `ConfirmAction::PlayLocallyInstead` (unit variant) and
      `App::pending_local_play: Option<PendingQueueAction>`, and raise the modal
      with the owner and selection named, following the
      `DiscardOrSaveDirtyPlaylist` shape (`src/app/types_confirm.rs`,
      `src/app/queue_actions.rs:247`). Verification: a unit test asserts the
      deferred play is stored and a confirmation modal is requested.
- [ ] 5.2 Bind the prompt's keys: `y`/`Y`/Enter accept, `n`/`N`/Esc decline,
      other keys act on nothing and leave the modal open
      (`src/app/components/confirm.rs:123`,
      `src/app/shell_modal_actions.rs:146`). Verification: component tests for
      each key class.
- [ ] 5.3 Handle decline in `apply_confirm_action`: clear the pending play and
      change nothing else (`src/app/input_confirm_keys.rs:15`). Verification: a
      unit test asserts no command is sent, the attachment is intact, queue
      scope is unchanged, and no toast is raised.
- [ ] 5.4 Separate local-player preparation from attachment teardown: extract
      the shared tail of `App::restore_local_mode`
      (`src/app/session_switch.rs:280`) and add a preparation step that restores
      a suspended local player or constructs one through the ordinary local
      construction path (`src/app/construct.rs:259`), returning a result.
      Verification: unit tests assert preparation succeeds for a client with a
      suspended local player and for a client with none, and that the failure
      path leaves the attachment, its queue, and playback untouched. Use a test
      seam through the existing override pattern
      (`src/app/session_connect.rs:71`) so no live mpv handle is constructed.
- [ ] 5.5 Run the confirmed effect in order: prepare, stop the owner, end the
      attachment, rebind MPRIS, then run the deferred local play. Verification:
      a unit test asserts the stop command precedes the local submission, the
      attachment state is cleared, the media-key target is rebound, and no
      transport or queue command reaches the former owner afterward.
- [ ] 5.6 Cover the Emby-session owner variant: stop that session, end the
      control relationship, then play locally instead of issuing the session
      play request. Verification: a unit test asserts the session stop command
      is issued, the session state is cleared, and no session play request is
      sent.

## 6. Composition

- [ ] 6.1 Add a shell tick-integration test for the prompt through
      `Application::tick()`: pressing play on a video while attached to an
      audio-only owner mounts the confirmation modal, and accepting it routes to
      local playback with the attachment gone
      (`src/app/tests_tick_integration*.rs`). Verification: the test drives the
      real sync/mount/dispatch pass rather than calling `Component::on`
      directly, and asserts the modal is mounted and focused while the play
      stayed unsubmitted.

## 7. Documentation and decisions

- [ ] 7.1 Update the "Audio-only owner" entry in `CONTEXT.md` to describe the
      shipped behavior and reference this change instead of the parked plan
      (`CONTEXT.md:115-121`). Verification: the entry names the prompt, the
      owner stop, and the ended attachment, and points at this change.
- [ ] 7.2 Add a dated correction to ADR 0017 recording that the parked-owner
      fall-through is superseded by the prompt plus ended attachment, keeping
      the original decision text intact (`docs/adr/0017-*.md:55-66,124-131`).
      Verification: the annotation carries a date, names the superseding
      change, and states why parking was dropped.
- [ ] 7.3 Confirm the `non-audio-fall-through` spec reads consistently: the
      Purpose already reflects the new behavior and the delta's REMOVED
      requirements are gone from the capability after archive. Verification:
      `openspec validate play-locally-when-owner-cannot --strict` passes and, at
      archive time, `openspec show non-audio-fall-through --type spec` contains
      only the three modified and three added requirements.

## 8. Gates

- [ ] 8.1 Run the repo gates on the touched packages: `cargo fmt --all`, then
      `cargo check -p mbv-core --all-targets`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo nextest run -p mbv-core` and
      `cargo nextest run -p mbv`. Verification: every command exits zero with no
      warnings.
- [ ] 8.2 If any new `debug_assert!` lands in this change, also run
      `cargo test --release -p <pkg>` (CI runs the release profile, where
      debug assertions are compiled out). Verification: the release-profile run
      of the affected package exits zero.
- [ ] 8.3 Manual check against a real audio-only owner: attach to a
      `mbvd --audio-only`, play a video, confirm the prompt appears, accept it,
      and confirm the video plays locally while the owner stops; repeat with a
      decline and confirm nothing changes. Verification: recorded as a manual
      check in the task notes — this is deliberately not a unit test, since it
      needs a live daemon and mpv.
