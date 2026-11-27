# Tasks

## 1. Release mode in the Owner

- [x] 1.1 Add `StopScope::Release` in `crates/mbv-daemon/src/cast.rs` and handle it in
  `stop_active_output`: Cast arm keeps the stop report, the Feed `Stopped`, and forgetting the
  loaded item but sends no media stop; Emby Session arm forgets the loaded item and sends no
  device stop; Local arm still calls `player.stop()`. Verify: unit tests with a recording mock
  assert no stop job is queued for Cast or Emby Session under `Release`, and the local run stops.
- [x] 1.2 Give `stop_old_output_for_handoff` (`output/handoff.rs`) a `SelectionIntent` parameter:
  `Continue` keeps `LoadedItemOnly` and the receiver app teardown; `SwitchOnly` uses `Release` and
  queues no teardown. Update the three callers (`hand_remote_output_back_to_local`,
  `cast/connect.rs`, `emby_session/connect.rs`). Verify: `make checks PKG=mbv-daemon` compiles.
- [x] 1.3 Check the old Cast connection worker's drop path and remove any stop or app teardown it
  sends on shutdown after a `Release`. Verify: a mock-worker test asserts a released output's
  drop sends no stop.

## 2. Owner tests

- [x] 2.1 Cover the contract "a switch leaves a released remote device playing" for a Cast
  receiver to Emby Session, an Emby Session to Cast, and a remote to Local switch, in the
  existing hand-off test files of `mbv-daemon`; update the tests of #1072 that assert the old
  output stops on a switch. Verify: `make test PKG=mbv-daemon`.
- [x] 2.2 Guard that a `Continue` selection still stops the old output, and that a status report
  from a released device advances nothing. Verify: `make test PKG=mbv-daemon`.

## 3. Docs

- [x] 3.1 Update `docs/invariants/31-severing-a-target-stops-no-playback.md` (the switch send
  stops no remote playback), `CONTEXT.md` *Output switch*, and the owner-spec prompt text that
  says the old output stops on a switch. Verify: `rg "old output stops" docs CONTEXT.md` shows no
  switch-answer claim.
- [ ] 3.2 Run `make format`, `make checks`, and `make test`; fix every red gate. Verify: all
  three exit 0.

## Workflow follow-up

- Land after PR #1072 merges. Sync and archive only on the user's merge approval.
