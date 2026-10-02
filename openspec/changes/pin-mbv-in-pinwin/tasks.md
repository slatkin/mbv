# Tasks

Rewritten around the in-app model (proposal, design D1–D8). The pinwin library reshape is
specified and implemented upstream in `slatkin/pinwin` change `add-library-abi`; this change
imports the resulting revision (section 3) and wires it into mbv. Section 0 is done work that
is kept; section 2 removes done work that the rewrite supersedes.

## 0. Carried over (done in the first version)

- [x] 0.1 Import `~/Dev/pinwin/pinwin/` into `pinwin/` and the three pinwin specs into `openspec/specs/` (commit e20c10e6a, `slatkin/pinwin@eaefd7f`). This is the program form; task 3.1 re-imports the library form the upstream change produces.
- [x] 0.2 `*.zig` governed by `check-code-file-lines` (e20c10e6a).
- [x] 0.3 `pinwin/AGENTS.md` and the root `AGENTS.md` map line (e20c10e6a); reworded for the library in 3.1.

## 1. Probe

- [x] 1.1 Prove the D3 hand-over before the rest is built, using the program-form pinwin already in `pinwin/` (its `pinwin` executable runs a command on a pty; that is enough to exercise the slave side) or a scratch binary outside the repo: from foot or ghostty, and from a compositor keybind with no terminal, a process that `TIOCNOTTY`s (when it has a controlling terminal, `SIGHUP` ignored across the call) and `dup2`s a pty slave onto 0/1/2 gets crossterm's size from the slave, receives resize events when `SIGWINCH` is raised in-process after `TIOCSWINSZ`, and sees kitty keyboard, mouse and kitty graphics work. Also confirm `local_daemon::spawn_detached` and libmpv do not touch the launching terminal afterwards. Record the result under D3 in `design.md` ("Probe result"); if the route fails, switch D3 to its fork alternative there before section 4. Verify: `design.md` D3 has a "Probe result" paragraph naming the route that worked.

## 2. Remove the first version's external-program wiring

Use `git revert --no-edit`, one revert commit per group, newest first within a group. The
groups are not in global newest-first order: the launcher commits (2.3) postdate the ctrl
commits (2.2), and `951ca97a7` and `4bc2fdb6e` both touch `src/main.rs` — expect a textual
conflict there; resolve it keeping both reverts' intent.

- [x] 2.1 Revert the tray work: `89d27cf82`, `a36fef682`, `e37f6770a`, `ca03159c4` (the ADR 0004 amendment is `a36fef682`). Verify: `cargo nextest run -p mbv-daemon -p mbv-desktop` passes. (The diff-against-baseline check for these paths runs at 2.2: `83e476b7a` also touched `crates/mbv-daemon` and is reverted there.)
- [x] 2.2 Revert the ctrl capability and declaration: `83e476b7a`, `951ca97a7`, `89d58bf51`, `5690ab521`. Verify: `cargo nextest run -p mbv-ctrl -p mbv-daemon -p mbv-remote-player -p mbv` passes, `rg -n "DeclarePinned|pinned.panel|PINWIN_SOCKET" crates src` finds nothing, and `git diff 8fb44d79c -- crates/mbv-daemon crates/mbv-desktop docs/adr/0004-tray-observer-and-non-takeover-commands.md Cargo.lock` is empty.
- [x] 2.3 Revert the launcher: `79a49de20`, `4bc2fdb6e`. Verify: `git diff 8fb44d79c -- contrib/mbv.desktop` is empty, `rg -n "\-\-desktop|desktop_launch" src README.md contrib` finds nothing, and `cargo nextest run -p mbv` passes.
- [x] 2.4 Revert the packaging split: `91d68daf4`. Verify: `makepkg --printsrcinfo` lists the single `mbv` package for both PKGBUILDs and `.github/workflows/aur.yml`'s `pkgver`/`sha256sums` rewrite still matches the files' lines.

## 3. Import the pinwin library

Blocked until `slatkin/pinwin` change `add-library-abi` has landed, including the three D2
additions (SIGWINCH on resize, start-time geometry validation, no thread left after a failed
start), and is tagged `library-abi`.

- [x] 3.1 Import the library-form pinwin revision (the `library-abi` tag) over `pinwin/` (the tree 0.1 imported is the program form): sources and `build.zig`/`build.zig.zon` at the upstream revision — no `tray.c`/`control.c`, no control socket, no installed executable entry (a dev-only `pinwin-demo` may exist under `zig build demo`; it is never shipped). Re-sync `openspec/specs/pinwin-panel` from upstream and delete `openspec/specs/pinwin-tray-options` and `openspec/specs/pinwin-control`. Reword `pinwin/AGENTS.md` (no installed executable, `zig build check`) and the root `AGENTS.md` map line. Verify: `cd pinwin && zig build check` passes, `zig build -Doptimize=ReleaseSafe` produces `zig-out/lib/libpinwin.a` and `zig-out/lib/libghostty-vt.a`, `rg -n "PINWIN_SOCKET|no_tray|dbusmenu|getenv" pinwin/src pinwin/build.zig` finds nothing, `rg -n "SIGWINCH" pinwin/src` finds the resize raise, and `openspec validate --specs` passes.

## 4. mbv wiring

- [ ] 4.1 New leaf crate `crates/mbv-pinwin`: `build.rs` runs `zig build` for `pinwin/` and links `libpinwin.a` and `libghostty-vt.a` plus GTK4, gtk4-layer-shell and pango/cairo (design D2, D4); a safe wrapper over the C ABI (owned types, result enum, no raw pointers in the public API). The root `mbv` crate depends on it behind the cargo feature `pinning`. Verify: `cargo check -p mbv`, `cargo check -p mbv --features pinning`, and `cargo tree -p mbvd -i mbv-pinwin`, `cargo tree -p mbv-config -i mbv-pinwin` and `cargo tree -p mbv-core -i mbv-pinwin` each report that the package is not in the graph, and `LD_DEBUG=libs target/debug/mbv --pin 2>&1 | grep 'find library='` shows libgtk4-layer-shell loaded before libwayland-client (design Risks, link order).
- [ ] 4.2 `crates/mbv-config`: add the `[panel]` keys (`side`, `cols`, `gutter_top/bottom/left/right`) with Rust range validation (design D6), parse, save and `dist/config.toml` documentation. Contract test: out-of-range `[panel]` values fall back to their defaults and a valid section round-trips through save. Verify: `cargo nextest run -p mbv-config`.
- [ ] 4.3 F2 Panel page (design D6): a `Panel` destination on the main page with `Side` as a `Text` row and the five numeric rows as a new stepper row kind reusing the `Text` row's value keys (Shift steps by 10, bounded), in `mbv-ui-model/src/settings.rs` and `src/app/dispatch/settings.rs`. The page is present only when the root crate's `pinning` feature is built; the shell passes that in, `mbv-ui-model` gets no feature. While pinned a step calls `pinwin_apply_layout` (wired in 4.4) and saves only on `PINWIN_OK`; a rejection shows a Warning toast and keeps the previous value. No new tests (design D8). Verify: `cargo nextest run -p mbv-ui-model -p mbv` and `cargo check -p mbv --features pinning`.
- [ ] 4.4 Start-up in `src/main.rs` (design D3, D5): parse `--pin` (help text included); without the feature, reject it (exit 2). With it, after `load_config` and before any terminal output or setup, check `WAYLAND_DISPLAY`, set the terminal environment (design D3; restored on failure), open the pty pair, `pinwin_start` with the saved `[panel]` layout, then `TIOCNOTTY` (if a controlling terminal exists, `SIGHUP` ignored across it) and `dup2` the slave onto 0/1/2; `pinwin_stop` before normal exit. On any start failure, log, then warn and continue in the terminal when stdin is a tty, otherwise `notify-send` and exit 1; fatal start-up exits after the hand-over use the same log + notification report (design D5). Wire the F2 Panel steps to `pinwin_apply_layout`. Set `contrib/mbv.desktop` to `Exec=mbv --pin`, `Terminal=false`. Verify: `cargo nextest run -p mbv`, `cargo check -p mbv --features pinning`, `desktop-file-validate contrib/mbv.desktop`.
- [ ] 4.5 `CONTEXT.md`: add the pinned-panel terms (pinned panel, `--pin`, gutters) beside the in-TUI panel vocabulary so the two senses of "panel" stay distinct. Verify: `rg -n "pinned panel" CONTEXT.md` finds the entry.

## 5. Packaging, CI, docs

- [ ] 5.1 `build.yml`: install `zig gtk4 gtk4-layer-shell` in the pinned image (confirm the image's Zig is 0.16 or newer; if not, fetch the pinned Zig release instead), build the pinned `mbv` with `--features pinning` into a separate target directory (`--target-dir target-pinned`) for the tarball, and keep the existing unpinned `cargo build --release` for `mbvd` and the `.deb`. Remove the `contrib/mbv.desktop` entry from the root `Cargo.toml` `[package.metadata.deb]` assets. After `cargo deb`, assert the mbv `.deb` Depends name no `gtk4` or `gtk4-layer-shell` package, alongside the existing pipewire checks. `PKGBUILD` and `PKGBUILD-git`: single package; `makedepends` gain `zig gtk4 gtk4-layer-shell`, `depends` gain the GTK4 and gtk4-layer-shell runtime libraries, and the build uses `--features pinning`. Verify: the CI job passes on a branch push, and `makepkg --printsrcinfo` lists one package for each PKGBUILD.
- [ ] 5.2 Docs: document `--pin`, the desktop entry's behaviour (including that GNOME/X11 are unsupported) and the `[panel]` keys in `README.md` (the `pinwin/AGENTS.md` and root `AGENTS.md` rewording happens with the import in 3.1). Verify: `rg -n "pinwin" README.md` shows no mention of a user-run `pinwin` command.

## 6. Integration check

- [ ] 6.1 Run `cargo clippy --workspace --all-targets -- -D warnings` and again with `--features pinning` on the `mbv` package, `cargo fmt --all -- --check` and `make check-code-file-lines`, and verify they all pass.
- [ ] 6.2 Manual check under niri:
  - `mbv` in foot, ghostty and kitty: opens in that terminal as before; `mbv` over ssh with `WAYLAND_DISPLAY` set in a tmux session: opens in the terminal.
  - Desktop entry: opens docked in the panel with no terminal window; posters, mouse, kitty keyboard and resize work; quitting removes the panel.
  - `mbv --pin` in a terminal: panel opens, the terminal waits, the prompt returns when mbv quits.
  - `mbv --pin` from inside tmux: posters render in the panel (terminal environment normalised).
  - A `[panel]` layout that leaves no output width, then the desktop entry: notification, exit 1.
  - `mbv --pin --connect-daemon <endpoint>` against a running `mbvd`: remote client in the panel.
  - Step width, side and a gutter in F2's Panel page while pinned: the panel updates live and survives a relaunch; a step that leaves no output width shows a warning toast and changes nothing.
  - `mbv --pin` with `WAYLAND_DISPLAY` unset in a terminal: one warning line, terminal launch works. The desktop entry under a session without layer-shell (or with `WAYLAND_DISPLAY` removed from the keybind's environment): a notification and exit 1.
  - Stay-alive on or off: the Owner's tray behaves as it did before this change.
  - A build without `pinning`: `mbv --pin` exits 2 with the error; no Panel page in F2.
- [ ] 6.3 Archive this change and sync the `pinned-launch` delta into `openspec/specs/` (the pinwin specs are already synced by the 3.1 re-import; this covers only `pinned-launch`). Verify: `openspec validate --specs` passes and `openspec/changes/` no longer lists `pin-mbv-in-pinwin`.
