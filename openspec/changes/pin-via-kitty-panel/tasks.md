# Tasks

## 1. Probe kitty on niri (the user runs the commands)

- [ ] 1.1 Ask the user to run this on niri: `kitten panel --edge=left --columns=40 --margin-right=40 --focus-policy=on-demand -o allow_remote_control=socket-only --listen-on=unix:$XDG_RUNTIME_DIR/mbv-probe.sock -- sh -c 'sleep 600'`. Ask where tiled windows start: at the panel edge, or 40 px after it. Write the answer and the build or no-build choice into design D4. Verify: D4 states one result.
- [ ] 1.2 With the probe panel open, ask the user to run `kitten @ --to=unix:$XDG_RUNTIME_DIR/mbv-probe.sock resize-os-window --action=os-panel --incremental columns=80 margin-left=-10`. Write the result for the resize and for the negative margin into the design Risks entries. Verify: the negative-gutter and `allow_remote_control` entries state the result.
- [ ] 1.3 Read the kitty changelog with `ketch` and find the first version with `resize-os-window --action=os-panel`. Verify: the design Open Questions entry names the version.

## 2. Configuration: drop the accent

- [ ] 2.1 Remove `accent`, `accent_color` and `accent_width` from `PanelConfig`, from `parse_panel_section` and from the `[panel]` save path in `crates/mbv-config` (design D5). Delete the accent parse tests. Add one test that loads a `[panel]` with `accent_color = "#123456"`, saves it, and finds no accent key (scenario "Custom colour survives cycling"). Verify: `cargo nextest run -p mbv-config` passes.
- [ ] 2.2 Remove the three accent rows and the "applies on the next launch" toast. They live in `crates/mbv-components/src/settings.rs`, the `mbv-render` settings rows and `src/app/shell/settings.rs`. Delete their tests. Verify: `cargo check --workspace --all-targets` passes.
- [ ] 2.3 Delete the *Focus accent* entry in `CONTEXT.md`. Verify: `rg -n "accent_color" CONTEXT.md` returns nothing.

## 3. Launcher and remote-control client

- [ ] 3.1 Rewrite `src/pin.rs` as the launcher (design D1, D3). It runs the pre-start tests from design D1, builds the `kitten panel` argument list from `PanelConfig`, spawns kitty, waits, and exits with the kitty status. Keep `report_start_failure`, `fatal` and `notify`. Delete `open_pty`, `detach_controlling_terminal`, `hand_over_stdio`, `PanelEnv` and `HANDED_OVER`. Keep `redirect_stderr` for the pinned process (design D6). Make the argument builder a pure function. Replace the `cover` test with one `#[case]` table on the builder: cover off gives no `--exclusive-zone`, and cover on gives `--exclusive-zone=0 --override-exclusive-zone`. Verify: `cargo nextest run -p mbv pin` passes.
- [ ] 3.2 Add pinned-process detection. Read `MBV_PINNED` and `KITTY_LISTEN_ON` once at start, remove both from the environment before any thread starts, and build `PinnedPanel` from the address. Add one test for the parse of the two values into `Option<PinnedPanel>`. Verify: the test passes.
- [ ] 3.3 Implement `apply_layout` as one `kitten @ --to=<address> resize-os-window --action=os-panel --incremental ...` call (design D2). Map a spawn error or a non-zero exit to the error string that the existing toast path shows. Remove `ANIM_DEFAULT_MS`. If task 1.1 chose to build the pixel zone, build it and the apply after start. Make the argument list a pure function, and add cases to the 3.1 table only where the expected output differs. Verify: `cargo nextest run -p mbv` passes.
- [ ] 3.4 Rewire `src/main.rs`. With `--pin` and no `MBV_PINNED`, run the launcher and exit with its status before `run_configured_startup`. With `MBV_PINNED`, run the TUI with `Some(PinnedPanel)`. Verify: `cargo check -p mbv` passes and the `mbv --help` text is unchanged.
- [ ] 3.5 Update the comments that name pinwin in `src/app/state/app_struct.rs`, `src/app/state/panel_focus.rs`, `src/app/shell.rs`, `src/app/tests/`, `crates/mbv-config/src/panel.rs` and `crates/mbv-config/src/parse.rs`. Update `docs/invariants/17-resize-neither-blanks-nor-wipes-art.md` to name kitty resizes. Verify: `rg -n -i pinwin src crates docs/invariants` returns nothing.

## 4. Dependencies, build and docs

- [ ] 4.1 Remove `pinwin` from `Cargo.toml` and run `cargo update -p pinwin`. Verify: `cargo tree -i gtk4` finds no match and `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] 4.2 In `.github/workflows/build.yml`, drop `zig gtk4 gtk4-layer-shell` from the pacman line and the two `libgtk-4-1` and `libgtk4-layer-shell0` `Depends` assertions. Add kitty as an optional dependency in the `.deb` packaging (`Recommends`) and the Arch packaging (`optdepends`). Verify: `rg -n -i gtk .github/workflows/build.yml` returns nothing.
- [ ] 4.3 Update `AGENTS.md`: replace the pinwin repository-map entry and the pinwin example in the live-test rule. Update `CONTEXT.md` so that *Pinned panel* names `kitten panel`. Update `README.md`: `--pin` needs kitty at the version from task 1.3, and `kitty.conf` sets the panel look. Verify: `rg -n -i pinwin AGENTS.md CONTEXT.md README.md` returns nothing.

## 5. Integration

- [ ] 5.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run --workspace`. Verify: all three pass.
- [ ] 5.2 Ask the user to run `mbv --pin` on niri and walk the `pinned-launch` scenarios: a launch from a terminal and from the desktop entry, `Ctrl+e`, an F2 width step, a gutter step, `cover = true`, and quit. Verify: the user reports that each scenario works.
