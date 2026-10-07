# Tasks

## 1. Bump pinwin

- [x] 1.1 In `Cargo.toml`, replace `pinwin = { git = "https://github.com/slatkin/pinwin", tag = "0.2.2" }` with `rev = "8127ff81079d91f326dc5b8427ab03a1dc17992c"` (tag 0.3.0, design D4). Run `cargo update -p pinwin` so `Cargo.lock` follows.
- [x] 1.2 In `src/pin.rs` `start_panel`, replace the `Startup { fd, layout, keyboard, accent }` struct literal with `Startup::new(fd, layout, keyboard, accent)`. Verify with `cargo check -p mbv`.

## 2. Instance socket and `--toggle` (`src/pin.rs`, `src/main.rs`)

- [x] 2.1 In `pin.rs`, add a private `instance_name()` that returns `pinwin::instance::InstanceName::parse("mbv")` (D1).
- [x] 2.2 Add `PinLaunch { Started(PinnedPanel), ShownExisting }`. Add `PinStartError::Instance(pinwin::instance::InstanceError)` and `PinStartError::ShowExisting(pinwin::instance::SendError)`. Extend `Display` and `source` exhaustively.
- [x] 2.3 Rework `pin::start` to return `Result<PinLaunch, PinStartError>` in the D2 order: Wayland check, then `InstanceSocket::bind`. On `Duplicate`, call `instance::send(.., Request::Show)` and return `ShownExisting` or `ShowExisting(err)`. Any other bind error returns `Instance(err)`. On `Ok(socket)`, run `PanelEnv::apply` and then `start_panel(config, socket)`, which uses `.with_instance(socket)`.
- [x] 2.4 In `main.rs` `main`, match the new result. `Ok(Started(panel))` gives `Some(panel)`. `Ok(ShownExisting)` returns from `main` (exit 0). `Err` goes to the unchanged `report_start_failure`. Update the comment above the block.
- [x] 2.5 Add `pin::toggle_running()` (D3). It sends `Request::Toggle` and returns on `Ok`. On `NotListening`, it reports "mbv: no pinned mbv is running". Any other `Err(error)` reports "mbv: cannot toggle the pinned panel: {error}". Both reports go through one helper: stderr when stdin is a TTY, otherwise `notify`, then exit 1. The helper reuses `failure_action`.
- [x] 2.6 In `pre_config_startup`, handle `--toggle` right after `-q`: call `pin::toggle_running()` and then `return None`. Add a `--toggle` line to the help text below `--pin`: "Show or hide the running pinned panel (bind this to a compositor key)."
- [ ] 2.7 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run -p mbv`. Existing `pin.rs` tests must pass unchanged. Add no new unit tests: the bind, show and toggle contract is owned and tested by pinwin, and mbv's outcome mapping is covered by the manual run in 5.1.

## 3. Packaging and CI

- [x] 3.1 Build `cargo build --release -p mbv`. Run `ldd target/release/mbv | rg -i 'gtk|wayland|xkbcommon|fontconfig'`, confirm no GTK, and note which of libwayland-client, libxkbcommon and libfontconfig are linked (D5).
- [x] 3.2 In the `Cargo.toml` `[package.metadata.deb] depends`, drop `libgtk-4-1, libgtk4-layer-shell0` and add the Debian packages for the libraries 3.1 found (`libxkbcommon0`, `libfontconfig1`, and `libwayland-client0` only if linked).
- [x] 3.3 In `PKGBUILD` and `PKGBUILD-git`, replace `gtk4 gtk4-layer-shell` with `libxkbcommon fontconfig` (plus `wayland` if 3.1 shows libwayland-client) in `depends` and in `makedepends`.
- [x] 3.4 In `.github/workflows/build.yml`, swap `gtk4 gtk4-layer-shell` in the pacman install line for the 3.3 packages. Point the two deb `Depends` greps (`libgtk-4-1`, `libgtk4-layer-shell0`) at the new Debian names from 3.2.

## 4. Docs

- [x] 4.1 In `src/pin.rs`, remove the stale GTK wording from the comments only. That covers `redirect_stderr` ("GTK/GLib/layer-shell") and the two `PanelEnv` comments ("pinwin GTK thread"), which become "the pinwin panel thread".
- [x] 4.2 In `CONTEXT.md` *Pinned panel*, change "GTK layer-shell window" to "Wayland layer-shell surface", and add that it can be hidden and shown with `mbv --toggle` while mbv keeps running.
- [x] 4.3 In `AGENTS.md`, change the pinwin entry from "GTK4 layer-shell panel" to "Wayland layer-shell panel". Leave its `rev` instruction, which now matches.

## 5. Manual verification (user-run)

- [ ] 5.1 Ask the user before running anything live, and give these exact commands:
  1. `cargo run --release -- --pin` from a terminal.
  2. With it running, `cargo run --release -- --toggle` twice. Check that the panel hides and then shows at the same width with no TUI reflow, playback continues, and focus returns on show and leaves on hide (niri).
  3. `cargo run --release -- --pin` again while hidden, and then while shown. Expect a show, then no change, with exit 0 both times.
  4. Quit mbv, then run `--toggle`. Expect "no pinned mbv is running" and exit 1.
  5. Repeat steps 1 to 4 with `[panel] cover = true` and check that no tiled window moves.

## Workflow follow-up

- On sync, update the `pinned-launch` Purpose line ("GTK layer-shell panel" becomes "Wayland layer-shell panel") by hand, because deltas cannot change a Purpose.
- Archive after 5.1 passes.
