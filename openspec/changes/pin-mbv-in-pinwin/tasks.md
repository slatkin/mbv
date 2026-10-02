# Tasks

Prerequisite: slatkin/pinwin#1 (`add-pinwin-control`) has landed and been archived in the pinwin
repo. Record its commit SHA for 1.1.

## 1. Import pinwin

- [x] 1.1 Copy the tracked files of `~/Dev/pinwin/pinwin/` (from `git -C ~/Dev/pinwin ls-files pinwin/`) into `pinwin/` here, excluding `zig-out/`, `zig-pkg/` and `.zig-cache/`. Add their ignores to `pinwin/.gitignore`. Verify: `cd pinwin && zig build && zig build check` prints `check_options: all passed`, and `cargo check --workspace` is unaffected.
- [x] 1.2 Copy `~/Dev/pinwin/openspec/specs/{pinwin-panel,pinwin-tray-options,pinwin-control}/spec.md` into `openspec/specs/`, deleting the "Install from the checkout" and "Coexists with pinwin" requirements from `pinwin-panel` (design D1). Verify: `openspec validate --specs --strict` passes for the three specs.
- [x] 1.3 Add `pinwin/AGENTS.md` (Zig/C conventions, `zig build` / `zig build check`, manual checks under niri, no Rust rules apply) and one line for `pinwin/` in the root `AGENTS.md` repository map. Verify that both files name `zig build check` as the check command.
- [x] 1.4 Add `*.zig` to the governed extensions in `scripts/check-code-file-lines.sh` and add the matching case to `scripts/check-code-file-lines-test.sh`. Verify: `make test-check-code-file-lines` and `make check-code-file-lines` pass. Commit 1.1–1.4 as one import commit whose message names `slatkin/pinwin@<sha>`.

## 2. Packaging and CI

- [x] 2.1 `build.yml`: add `zig gtk4 gtk4-layer-shell libdbusmenu-glib` to the pacman install, run `zig build -Doptimize=ReleaseSafe && zig build check` in `pinwin/`, and copy `pinwin/zig-out/bin/pinwin` into the release tarball. Verify by reading the workflow diff: the tarball step lists `pinwin`, and no Rust step changed.
- [x] 2.2 `PKGBUILD`: convert to `pkgbase=mbv`, `pkgname=(mbv pinwin)`, `package_mbv()` (today's contents plus `optdepends` for `pinwin` and `xdg-terminal-exec`) and `package_pinwin()` (`/usr/bin/pinwin`, license; `depends=(gtk4 gtk4-layer-shell libdbusmenu-glib pango)`). Verify: `makepkg --printsrcinfo` lists both packages, and `mbv`'s `depends` is unchanged.
- [x] 2.3 `PKGBUILD-git`: same split; add `zig gtk4 gtk4-layer-shell libdbusmenu-glib` to `makedepends`; build pinwin with `zig build -Doptimize=ReleaseSafe` in `build()`. Verify: `makepkg --printsrcinfo` lists both packages, and `aur.yml`'s `pkgver`/`sha256sums` rewrite still matches the file's lines.

## 3. Ctrl declaration

- [x] 3.1 `crates/mbv-ctrl`: add the `pinned-panel` capability constant to `CtrlHello::current()`, `supports_pinned_panel()`, the matching `CtrlCompatibility` field, and `CtrlCmd::DeclarePinned { socket: PathBuf }`, with its `requires_owner` classification as for other Local-only commands. Extend `tests_handshake.rs::current_hello_validates` to assert the capability is advertised. Verify: `cargo nextest run -p mbv-ctrl`.
- [x] 3.2 `crates/mbv-remote-player`: copy `supports_pinned_panel` into compatibility in `connect.rs`, and add `RemotePlayer::declare_pinned(&self, socket: PathBuf)`, which sends the command only when supported. Verify: `cargo check -p mbv-remote-player`.
- [x] 3.3 `src/`: add a pure `pinned_socket(Option<OsString>) -> Option<PathBuf>` (non-empty absolute path only), with a two-case `#[case]` test (absolute → `Some`, relative → `None`). Call it in `attach_owner_process` and `declare_pinned` once after connecting. No other attach path calls it. Verify: `cargo nextest run -p mbv` for the new test.
- [x] 3.4 `crates/mbv-daemon`: `CtrlClient` gains `pinned: Option<(u64, PathBuf)>`; `ClientRegistry` gains `set_pinned(id, path)` (local clients only, monotonically increasing sequence) and `latest_pinned()`; `remove` drops it with the client. Handle `DeclarePinned` in `control.rs`: log and ignore from non-local clients. Tests (contract: the pin target follows attached local declarations): one test that a TCP declaration records nothing, one test that `latest_pinned()` falls back to the earlier pinned client after the latest one is removed. Verify: `cargo nextest run -p mbv-daemon`.

## 4. Tray

- [x] 4.1 `crates/mbv-daemon`: add `pub trait TrayPort: Send { fn set_pin_target(&self, Option<PathBuf>); }` and change the `on_tray_ready` hook to return `Option<Box<dyn TrayPort>>`. Make `start_tray` keep the hook when stay-alive is off, invoke it on the first accepted `DeclarePinned` (at most once per daemon), and push `latest_pinned()` to the port after each accepted declaration, each client removal and right after a lazy start. `mbvd`'s no-op hook keeps compiling. Test (contract: tray starts lazily once when pinned without stay-alive): with stay-alive off, two declarations invoke a spy hook exactly once. Verify: `cargo nextest run -p mbv-daemon` and `cargo check -p mbvd`.
- [x] 4.2 `crates/mbv-desktop/src/tray.rs`: add an owned `pin_target: Option<PathBuf>` to `MbvTray`; implement `TrayPort` on the spawned handle via `Handle::update`; add a `Pin options...` menu item shown only while `pin_target` is `Some`, whose activation connects to the socket with 1-second read/write timeouts, sends `options\n`, reads one line and logs anything but `ok` under target `tray`. Extend the existing menu test so it asserts the item appears only with a target. Update `src/local_daemon.rs`'s hook to the new return type. Verify: `cargo nextest run -p mbv-desktop -p mbv`.
- [x] 4.3 Update `docs/` or `README.md` where the tray is described: the tray now also appears for pinned mbv without stay-alive, and it has the `Pin options...` item. Verify: `rg -n "tray" README.md docs/` shows no statement that the tray needs stay-alive.

## 5. Launcher

- [x] 5.1 Add `src/desktop_launch.rs`: a pure `desktop_command(on_path)` (`pinwin --no-tray mbv`, then `xdg-terminal-exec mbv`, else `None`), a std-only `PATH` lookup, and `exec`. Add a `#[case]` table with three cases (pinwin present; only xdg-terminal-exec; neither). Recognise `--desktop` in `pre_config_startup`, and handle it in `main` after `applog::init` and before `load_config`; on `None`, log the reason and exit 1. Add `--desktop` to `-h` output. Verify: `cargo nextest run -p mbv` and `mbv -h` lists `--desktop`.
- [x] 5.2 `contrib/mbv.desktop`: `Exec=mbv --desktop`, `Terminal=false`. Document the launcher (pinwin when installed, terminal otherwise, Wayland layer-shell only) in `README.md`. Verify: `desktop-file-validate contrib/mbv.desktop` passes.

## 6. Integration check

- [x] 6.1 Run `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` and `make check-code-file-lines`, and verify they all pass.
- [ ] 6.2 Manual check under niri with pinwin installed:
  - From the launcher, with stay-alive **off**: mbv opens pinned; exactly one tray icon (mbv's) appears; `Pin options...` opens the panel's options window; quitting mbv removes the panel and the tray.
  - From the launcher, with stay-alive **on**: the tray persists after quitting mbv, and `Pin options...` is gone once no pinned Client is attached.
  - With `show_systray_icon = false`: no tray appears.
  - With pinwin uninstalled: the launcher opens mbv in the terminal through `xdg-terminal-exec`.
