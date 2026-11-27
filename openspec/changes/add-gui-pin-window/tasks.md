# Tasks

Every unit runs `make format`, `make checks PKG=<package>`, and `make test PKG=<package>` in the
foreground for the packages it changes, and waits for each to finish. Never run a gate in the
background, and never poll or sleep. Task groups 1 to 8 build against the `PanelHost` mock
(design D3) and need no pinwin code. Only task group 9 needs the pinwin `v0.5.0` tag (design D2).

## 1. Records, config, and core facts

- [ ] 1.1 Write `docs/adr/0045-the-gui-hosts-pinwin-in-its-own-process.md` (read
  `docs/adr/README.md` first). It amends ADR 0033 for the GUI only, records design D1 and its
  rejected options, and states the GPL-3.0-or-later combined-work consequence. Add an
  "Amended by ADR 0045 for the GUI Client" line to ADR 0033. Verify: both files exist and ADR 0033
  names 0045.
- [ ] 1.2 Add the `CONTEXT.md` terms *Pinned window* (the GUI window docked by its in-process
  panel host; Avoid: pinned panel, which stays the TUI's term), *Queue view*, *Full view*, and
  *Panel host*. Add a sentence to *Pinned panel* that says it is the TUI's surface. Verify:
  `rg -n '^Pinned window:|^Queue view:|^Full view:|^Panel host:' CONTEXT.md` finds four lines.
- [ ] 1.3 Add `pinned: bool` (serde default `false`) to `mbv_config::GuiLaunchState`, and
  `full_view: Option<u32>` (serde default) to `GuiPanelWidths` (design D6, D10). Extend the
  existing round-trip tests in `crates/mbv-config/src/tests/` with one case each that loads a
  file without the field. Verify: `make test PKG=mbv-config` passes.
- [ ] 1.4 In `crates/mbv-client-core`, add `user_play_epoch: u64` to `App`, increment it in
  `collapse_pinned_view_after_play` before the pinned-width check, and add the
  `pub fn user_play_epoch(&self) -> u64` read (design D7). Extend the existing
  `collapse_pinned_view_after_play` test with an assert that the counter moves on an unpinned
  play. Verify: `make test PKG=mbv-client-core` passes.
- [ ] 1.5 Remove `WAYLAND_SOCKET` from the Owner spawn in
  `crates/mbv-client-core/src/app/owner_link/start.rs`, next to `CLAIMED_ENV` (design D4).
  Verify: `make checks PKG=mbv-client-core` passes.
- [ ] 1.6 Change `SettingKey::scope` in `crates/mbv-ui-model/src/settings.rs`: `Panel`,
  `PanelSide`, `PanelReserve`, the four gutters, `PanelAccent`, `PanelAccentColor`, and
  `PanelAccentWidth` become `AllClients`, and `PanelWidth` and `PanelWidthExpanded` stay
  `TuiOnly` (design D9). Update the existing scope test cases. Verify:
  `make test PKG=mbv-ui-model` passes.

## 2. Spike: the live window re-create

- [ ] 2.1 In `crates/mbv-gui/src/window/launch.rs`, split out `pub fn create_event_loop()`.
  Make `launch` take the event loop, and update `main.rs` to create the loop before it spawns
  the core thread (design D4, steps 6 to 8). Verify: `make checks PKG=mbv-gui` passes.
- [ ] 2.2 Add the `RedockEvent` embedder event and its handler in `MbvApplication::proxy_wake_up`.
  The handler moves the doc and the renderer from the old `View` into a new one, re-provides the
  root contexts, and registers the new `WindowId` (design D5 steps 2 to 4). It has no panel host
  yet. Add the pure `redock_attributes(pinned, restore_size)` with a `#[case]` test for the
  pinned (no minimum size) and unpinned (minimum and restore size) attributes. Verify:
  `make test PKG=mbv-gui` passes.
- [ ] 2.3 Add `RedockGuard` (design D5): it is set when a re-dock starts and cleared by the new
  window's first presented frame. Forward `Focused(false)` from `window_event` as
  `WindowEvent::FocusLost` on the window channel. Add one test: a focus loss while the guard is
  set reports as guarded. Verify: `make test PKG=mbv-gui` passes.
- [ ] 2.4 Record the re-dock in `docs/architecture/gui-client.md` (doc hand-over, contexts,
  guard). Verify: `rg -n 'RedockEvent' docs/architecture/gui-client.md` finds the section.

## 3. The pin module

- [ ] 3.1 Create `crates/mbv-gui/src/window/pin.rs` with `pin/host.rs`: `trait PanelHost` with
  the design D2 control methods, the GUI-side `PanelOptions`, `Reserve`, `Edge`, and `Gutters`
  types, `PinAvailability`, and a recording test mock in `pin/tests.rs` that can answer a
  scripted `Err`. Verify: `make checks PKG=mbv-gui` passes.
- [ ] 3.2 Add `pin/state.rs`: `PinState`, `PinnedView`, and the pure `queue_view_width`,
  `full_view_width` (with the design D6 clamp), `pinned_reserve`, and `pinned_tween` functions.
  Add one `#[case]` table per function, with only cases that have different outcomes. Verify:
  `make test PKG=mbv-gui` passes.
- [ ] 3.3 Add `PanelWidthsStore::drag_full_view_width` and the `full_view` signal, and include it
  in `save` and `reset_panel_widths` (design D6, D8). Extend the existing reset test so that it
  covers `full_view`. Verify: `make test PKG=mbv-gui` passes.
- [ ] 3.4 Add the root effect that sends one `set_size` when the computed pinned width changes
  (design D6), with the reserve and tween functions from 3.2. Add one test against the mock: a
  collapse in the Queue view sends exactly one `set_size` with the rail width. Verify:
  `make test PKG=mbv-gui` passes.

## 4. Pin and Unpin

- [ ] 4.1 Provide `PinAvailability` as a root context from `launch`, with `Unavailable` from
  `main.rs` for now. Add the pin control to `LibraryTopBar` beside the Settings control
  (`HeaderIconButton` with the Lucide `pin` and `pin-off` glyphs added to the icon set). Paint it
  only while `Available` (design D7). Its press sends `RedockEvent` through the
  `BlitzShellProxy` context. Verify: a headless test finds no pin control with `Unavailable` and
  one with the mock `Available`.
- [ ] 4.2 Wire the host into the `RedockEvent` handler: `open_panel` with the Queue view width,
  the `[panel]` edge, gutters, accent, and reserve before a pinned re-create; `release_panel`
  before an unpinned one; and a refusal that sends `GuiPinCommand::Refused { reason }` and skips
  the re-create (design D5 step 1). Handle `GuiPinCommand` in `crates/mbv-gui/src/core_thread.rs`
  with `App::flash`. Add one test: a scripted `open_panel` refusal leaves `PinState::Unpinned`
  and sends one `Refused`. Verify: `make test PKG=mbv-gui` passes.
- [ ] 4.3 Remember the logical window size at a pin and pass it as the restore size at an Unpin
  (design D5 step 3). Verify: the 2.2 attributes test covers the restore size.

## 5. The views

- [ ] 5.1 Change `destination_pressed` in `crates/mbv-gui/src/window/app.rs` for the pinned rules
  (design D7 *Header*). Extend the existing `destination_pressed` tests with the three pinned
  cases from spec `gui-pin-window` "The header moves between the views". Verify:
  `make test PKG=mbv-gui` passes.
- [ ] 5.2 Project `user_play_epoch` and the `[panel]` config in the GUI frame's `panel` section
  (`crates/mbv-gui/src/bridge/frame.rs`), and set the Queue view when the counter changes in the
  pinned Full view (design D7 *Play*). Add one test: a counter change in the Full view sets the
  Queue view, and a change in the Queue view sends nothing. Verify: `make test PKG=mbv-gui`
  passes.
- [ ] 5.3 Set the Queue view on `WindowEvent::FocusLost` in the pinned Full view with the guard
  clear (design D7 *Drawer*). Add one test for the Full view case. Verify:
  `make test PKG=mbv-gui` passes.
- [ ] 5.4 In `crates/mbv-gui/src/window/components/panel.rs`, mask `collapsed` with
  `library_visible || pinned`, and give `QueueFooter` the same condition for its collapse
  control (issue #1066). A collapse in the Full view sets the Queue view first. Skip the
  `LibraryTopBar` mount while the window is pinned and collapsed (design D7 *Rail*). Update the
  existing "queue-only window does not collapse" test so that it names the unpinned case, and add
  the pinned case. Verify: `make test PKG=mbv-gui` passes.
- [ ] 5.5 Update `docs/architecture/gui-client.md` with the pin state, the views, and the
  transitions. Verify: `rg -n 'Full view' docs/architecture/gui-client.md` finds the section.

## 6. The inner edge handle

- [ ] 6.1 Add `crates/mbv-gui/src/window/pin/edge_handle.rs` (design D8): the handle at the edge
  away from the docked edge in the Queue view and the Full view, none in the rail, pointer capture
  through `PanelDivider`'s rules, the distance-from-docked-edge measure, and one `set_size` per
  presented frame. Save through `drag_queue_width` or `drag_full_view_width` at the release. Add
  the pure `drag_target_width(edge, pointer_x, window_width, press_offset)` with a `#[case]`
  table for the left and the right dock. Verify: `make test PKG=mbv-gui` passes.
- [ ] 6.2 Add one mock test: a drag in the Queue view saves the clamped Queue width at the
  release, and a refused `set_size` during the drag keeps the last accepted width. Verify:
  `make test PKG=mbv-gui` passes.

## 7. The Panel settings section

- [ ] 7.1 Add `GuiSettingsCommand::PanelStep { setting, delta }` and route it in
  `crates/mbv-gui/src/gui_core/gui_settings/command.rs` to `App::apply_panel_setting` (design D9).
  Verify: `make checks PKG=mbv-gui` passes.
- [ ] 7.2 Add the Panel section to the GUI Settings sidebar after Display, with the cycle,
  toggle, and stepper controls of design D9 (the Shift press steps by 10). Extend the existing
  sidebar-order test and hidden-settings test for spec `gui-settings-screen` "The sidebar lists
  the sections in a fixed order" and "Settings with no effect on the GUI are hidden". Verify:
  `make test PKG=mbv-gui` passes.
- [ ] 7.3 Add the pinned interception of design D9 steps 1 to 4 and `GuiPinCommand::AccentDeferred`.
  Add one mock test: a refused gutter step sends `Refused` and no `PanelStep`. Add one more: a
  Reserve space step in the Full view sends `PanelStep` and calls no host method. Verify:
  `make test PKG=mbv-gui` passes.

## 8. Pinned at start

- [ ] 8.1 Write `GuiLaunchState.pinned` at the orderly exit from the `PinState`, and open the
  first window pinned in `can_create_surfaces` when the saved value is `true` and the host is
  `Available`. A refusal logs a warning and opens a normal window (design D10). Add one test: a
  saved `true` with `Unavailable` creates unpinned attributes. Verify: `make test PKG=mbv-gui`
  passes.

## 9. The pinwin wiring

- [ ] 9.1 Add the dependency `pinwin = { git = "https://github.com/slatkin/pinwin", tag = "v0.5.0", default-features = false, features = ["embed"] }`
  to `crates/mbv-gui/Cargo.toml` with a comment that names design D1 and D2. Verify:
  `cargo tree -p mbv-gui -i pinwin` shows the tag, and `cargo tree -p mbv-gui -i ksni` shows no
  path through pinwin.
- [ ] 9.2 Add `PinwinHost` in `crates/mbv-gui/src/window/pin/pinwin_host.rs`: the `PanelHost`
  implementation over `pinwin::embed::Host` that maps the GUI-side types to pinwin's. This is the
  untested boundary. Verify: `make checks PKG=mbv-gui` passes.
- [ ] 9.3 In `main.rs`, add design D4 steps 3 to 5: start the host when `WAYLAND_DISPLAY` is set,
  log one warning on an error, set `WAYLAND_SOCKET` with its SAFETY note, and pass
  `PinAvailability::Available(PinwinHost)` to `launch`. Record the start order and the
  `WAYLAND_SOCKET` rule in `crates/mbv-gui/AGENTS.md`. Verify: `make checks PKG=mbv-gui` passes
  and `rg -n WAYLAND_SOCKET crates/mbv-gui/AGENTS.md` finds the rule.

## 10. Licence notices

- [ ] 10.1 Add `LICENSES/GPL-3.0.txt`, copied from `~/Dev/pinwin/LICENSE`. Add it to the `.deb`
  assets in `crates/mbv-tui/Cargo.toml`, the tarball step and the `dpkg-deb` check in
  `.github/workflows/build.yml`, `PKGBUILD` and `PKGBUILD-git` (with
  `license=('MIT' 'GPL-3.0-or-later')`), and the `Makefile` install. Add the `README.md` sentence
  (design D11). Verify: `rg -n 'GPL-3.0' PKGBUILD PKGBUILD-git Makefile crates/mbv-tui/Cargo.toml .github/workflows/build.yml README.md`
  finds each file.

## 11. Gates

- [ ] 11.1 Run `make format`, `make checks`, and `make test` in the foreground, and fix every red
  gate, whoever introduced it. Verify: all three exit 0.
- [ ] 11.2 Run `make check-code-file-lines` and split any file over 800 lines along its
  responsibility seam. Verify: exit 0.

## Workflow follow-up

- Prerequisite outside this repository: the pinwin `embed` library (design D2) in tag `v0.5.0`.
  It is a pinwin OpenSpec change that only the user authorizes in `~/Dev/pinwin`. Task group 9
  waits for it.
- Sync the delta specs into `openspec/specs/` and archive the change only when the user approves
  the PR for merge, as the last commit before the merge.
