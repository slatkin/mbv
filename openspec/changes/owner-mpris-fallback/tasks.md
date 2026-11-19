# Tasks

## 1. Claim state machine in `mbv-desktop`

- [ ] 1.1 Add the pure claim-decision core to `crates/mbv-desktop/src/mpris.rs` (or
  `mpris/fallback.rs` if the file split is taken): states `Waiting`/`Holding`, inputs
  `AttachedClients` value and claim outcome, transitions per design D3 — claim only when no
  local Client is attached, never release while holding, `NameLost` returns to `Waiting`. Verify
  with `src` unit tests covering each transition (free-name claim, `Exists` stays `Waiting`,
  `NameLost` returns, attached Client suppresses claiming); no D-Bus in tests (issue #757 rule).
- [ ] 1.2 Add `mbv_desktop::mpris::start_owner_fallback` behind `#[cfg(not(test))]`: build the
  session connection serving `MediaPlayer2`/`MediaPlayer2Player` WITHOUT claiming the name, wire
  the status source to the passed `Arc<Mutex<PlayerStatus>>`, transport to the passed
  `mpsc::Sender<TransportCommand>`, art resolution to the passed cache-path resolver (design D4),
  and run the 500 ms poll loop that both publishes while `Holding` and drives the claim machine
  through `Connection::request_name_with_flags` with `AllowReplacement | DoNotQueue`. Failures
  log through `tracing` and park, never warn on the terminal (design D6). Verify with
  `cargo check -p mbv-desktop` and `cargo nextest run -p mbv-desktop`.

## 2. Local Owner spawn and packaging exclusion

- [ ] 2.1 Spawn the fallback in `src/local_daemon.rs`'s `on_player_ready` hook using the
  `DaemonPlayerHandle`'s status, transport sender, and attached-Client cell, with
  `crate::config::image_disk_cache_path` as the art resolver (design D5). Confirm the packaged
  entry (`crates/mbvd/src/main.rs`) never calls it. Verify with `cargo check --workspace` and by
  reading that no other `DaemonRuntimeHooks` caller gained MPRIS.
- [ ] 2.2 Confirm test builds stay clean: no test constructs the fallback, no test claims the real
  bus name, and `cargo clippy --workspace --all-targets -- -D warnings` passes with no lint
  suppression added.

## 3. Integration checks

- [ ] 3.1 `cargo fmt --all` and `make check-code-file-lines` pass with the fallback code in place;
  apply the design's file split if `mpris.rs` exceeds the limit.
- [ ] 3.2 Update the architecture note that MPRIS lives in the Client (design D4's shared
  machinery note): name the Owner fallback in the module doc of `crates/mbv-desktop/src/mpris.rs`
  and in `crates/mbv-daemon/src/lib.rs`'s tray/MPRIS ownership comment, so the docs match the new
  two-publisher split.
