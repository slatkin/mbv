# Tasks

## 1. Publishers in `mbv-desktop`

- [x] 1.1 Add the pure Client publishing rule to `crates/mbv-desktop/src/mpris.rs` (or
  `mpris/rule.rs` if the file split is taken): inputs "attached endpoint is Local" and the local
  Owner's `Option<PlayerStatus>`; publish only when the endpoint is not Local and the local status
  is not `Playing` (`active && !paused`); `None` counts as not `Playing` (design D3). Verify with
  one `src` `#[case]` table whose rows have distinct outcomes: Local endpoint → no name; remote with
  local `Playing` → no name; remote with local `Paused` → name; remote with no local status → name.
  No D-Bus in tests (issue #757 rule).
- [x] 1.2 Add `mbv_desktop::mpris::start_owner` behind `#[cfg(not(test))]`: serve
  `MediaPlayer2`/`MediaPlayer2Player` on a session connection, claim `org.mpris.MediaPlayer2.mbv`
  with `ReplaceExisting | DoNotQueue` and without `AllowReplacement`, take the status from the
  passed `Arc<Mutex<PlayerStatus>>`, send transport through the passed
  `mpsc::Sender<TransportCommand>`, resolve art through the passed resolver, and run the existing
  500 ms publisher loop (design D2). A failed connection logs and parks; a failed claim logs and
  retries on the next tick (design D6). Verify with `cargo check -p mbv-desktop`.
- [x] 1.3 Change the Client entry point `mbv_desktop::mpris::start` and `rebind` (design D4): the
  connection serves the interfaces without requesting a name; `MprisSource` gains the rule closure,
  set by `start` and `rebind`; the 500 ms loop requests `org.mpris.MediaPlayer2.mbv.instance<pid>`
  (`std::process::id()`) when the rule turns true, calls `Connection::release_name` when it turns
  false, and publishes no property signals while the rule is false. Claim and release errors log
  and retry on the next tick. Verify with `cargo check -p mbv-desktop` and
  `cargo nextest run -p mbv-desktop`.

## 2. Wiring

- [x] 2.1 Start the Owner publisher in `src/local_daemon.rs`'s `on_player_ready` hook with the
  `DaemonPlayerHandle`'s status and transport sender and `crate::config::image_disk_cache_path` as
  the art resolver (design D5), keeping the handle stored for the tray as today. Confirm the
  packaged entry (`crates/mbvd/src/main.rs`) still passes a no-op hook. Verify with
  `cargo check --workspace`.
- [x] 2.2 Pass the rule's inputs from the Client: `start_mpris`
  (`src/app/state/construct/remote.rs`) and `rebind_mpris_to_current_player`
  (`src/app/dispatch/session/switch.rs`) build the rule closure from whether `player_endpoint` is
  `DaemonEndpoint::Local` and from the suspended local link's `status_snapshot`, with a
  disconnected or absent link giving no local status (design D3). Verify with
  `cargo check --workspace`.
- [x] 2.3 Confirm test builds stay clean: no test constructs either publisher, no test claims a
  real bus name, and `cargo clippy --workspace --all-targets -- -D warnings` passes with no lint
  suppression added.

## 3. Finish

- [ ] 3.1 Run `cargo fmt --all`. Keep `mpris.rs` under 800 lines; apply the design's file split if
  the new code would take it past the limit.
- [ ] 3.2 Update the comments that place MPRIS only in the Client so they name both publishers and
  both bus names: the header comment of `crates/mbv-desktop/src/mpris.rs`, the module doc of
  `crates/mbv-desktop/src/lib.rs`, the ownership comment in `crates/mbv-daemon/src/lib.rs`, the
  `start_mpris` doc in `src/app/state/construct/remote.rs`, and the MPRIS paragraph in
  `src/owner_link.rs` (which says the Client is the only owner of the name).
