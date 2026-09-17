## 1. Unit A - owner-addressed slot jump (stops the panic)

- [ ] 1.1 Add one shared client slot-jump dispatch operation on `App` (remote owner -> owner-addressed slot request; this client owns playback -> sync snapshot, mint/accept the local transition, send the jump) and route `src/app/player_event.rs:18` through it; verify with an app-level test asserting a remote owner receives a jump request and no local-only command is constructed.
- [ ] 1.2 Route `src/app/player_event.rs:288` through the shared operation; verify its existing test still passes and that a remote owner receives a request rather than a local-only command.
- [ ] 1.3 Route the Next-Up accept event (`src/app/player_event.rs:371`) through the shared operation and drop the client cursor write on the remote branch; verify with an app-level test that an accept under a remote owner requests the owner's jump, leaves the local cursor on the owner snapshot, and keeps the client running.
- [ ] 1.4 Route `src/app/action.rs:527` through the shared operation so explicit play and the accept cannot diverge again; verify the existing action/queue tests pass (`cargo nextest run -p mbv`).
- [ ] 1.5 Audit every remaining client-side construction of a local jump command (repository-wide search for the dispatch shape) and confirm each is either converted or reachable only when this client is the owner; record the audit result in this change's notes.
- [ ] 1.6 Replace the local-only `unreachable!()` arm in `crates/mbv-core/src/ctrl.rs:456-465` with a fallible conversion and surface the refusal to the caller; verify with a ctrl-level test that the attempt is refused, no command is delivered, and the caller's process survives.
- [ ] 1.7 Present a refused remote jump with the message explicit play already uses; verify with an app-level test asserting the refusal is flashed and the client remains usable.

## 2. Unit B - one runtime source for the mpv overlay scripts

- [ ] 2.1 Make script resolution a pure function over injected candidate directories (checkout, package, legacy installer path) returning the chosen path plus any unused legacy copy found; verify with hermetic unit tests covering checkout present, package only, and legacy copy present-but-ignored (no real HOME/config/state directory is read).
- [ ] 2.2 Remove the user-data-directory branch from script resolution and correct the dead checkout fallback in `crates/mbv-core/src/config_paths.rs:53` to the checkout's real `scripts/` path; verify the 2.1 tests pass and a checkout run resolves the checkout's entry script.
- [ ] 2.3 Apply the same resolution rule to the overlay font directory (`crates/mbv-core/src/config_paths.rs:63`); verify a font-resolution test mirrors the script cases.
- [ ] 2.4 Log the resolved script path where the script is handed to mpv (`crates/mbv-core/src/player/runtime.rs:239`) and warn, naming the path, when an unused legacy copy exists; verify with a unit test over the log decision and by reading the startup log from one run on this machine.
- [ ] 2.5 Confirm packaging metadata still installs the whole fragment set (`Cargo.toml` script mapping, `PKGBUILD`) and that the entry script resolves its siblings from its own directory; verify the mapping lists every fragment and `openspec validate --all` passes.
- [ ] 2.6 Manual check: start daemon-owned playback and confirm the on-screen accept completes the jump; then remove the legacy `~/.local/share/mbv/scripts/mbv.lua` and repeat, confirming the accept still completes the jump after the copy is gone. (manual terminal check, not an automated test)

## 3. Gates and acceptance

- [ ] 3.1 `cargo fmt --all -- --check` reports no diff.
- [ ] 3.2 `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] 3.3 `cargo nextest run -p mbv -p mbv-core` passes (report any teardown flake with a clean rerun).
- [ ] 3.4 `openspec validate --all` passes.
- [ ] 3.5 Manual acceptance: reproduce the original report end-to-end (daemon owns playback, episode near its end, press the on-screen accept) and confirm the next episode starts immediately with the TUI still running, and that volume scaling now applies once (the proposal's breaking note).
