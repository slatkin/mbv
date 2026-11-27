# Design

## Context

The TUI pins through the external `pinwin` program (ADR 0033). The core's `app/pin.rs` builds
`pinwin` argv, and a worker thread runs one `pinwin` child per request. The core decides
"pinned" from the `PINWIN_PANEL` claim and `--in-panel`. The GUI never claims these, so for the
core the GUI is always unpinned.

These facts shape the design:

- winit-wayland `0.31.0-beta.3` (pinned by blitz-shell `0.3.0-beta.4`) has no layer-shell
  support. `EventLoop::new` connects through `Connection::connect_to_env()`, which reads
  `WAYLAND_SOCKET`, removes it, and sets `CLOEXEC` on the fd. It does one registry roundtrip
  during `EventLoop::new`.
- A `wl_surface` that has the `xdg_toplevel` role can never take the layer-surface role. A
  live re-dock therefore needs a new toplevel.
- pinwin `0.4.3` is binary-only and GPL-3.0-or-later. Its proxy (`src/proxy/`) turns an app's
  toplevel into a layer surface, and it docks the next toplevel when its panel holds no window.
  Its tween configures the app one time per change: a grow configures first and then shows a
  growing strip from the docked edge, and a shrink crops the old buffer from the docked edge and
  configures at the end.
- blitz-shell `View` has public `doc`, `renderer`, and `window` fields, `View::init` is public,
  `View` suspends the renderer in `Drop`, and `BlitzShellEvent::Embedder` carries any embedder
  event to the `ApplicationHandler`. mbv's `window/launch.rs` is a copy of
  `launch_cfg_with_props` that already handles the `Embedder` route.
- The GUI paints the Library column, the Queue divider, and the Queue column in that fixed
  order (`window/app.rs`). The arrangement comes from the Library column width
  (`window/arrangement.rs`): Mini below 480 px, Wide from 880 px. The Queue column is 400 px by
  default, 800 px at most, and 72 px as the rail.
- `main.rs` spawns the core thread before `launch` creates the event loop.

## Goals / Non-Goals

**Goals:**

- The live re-dock in the same process, with the VirtualDom kept.
- One Wayland connection path on Wayland, so that Pin is always one press away.
- A panel seam with a mock, so that every GUI unit except the last wiring lands before the
  pinwin `embed` release exists.

**Non-Goals:**

- No change to the TUI pin, Pin swap, the Tray, or `--toggle`.
- No hide or show of the GUI window from the Tray or a compositor key.
- No coordination between a GUI panel and a TUI panel. Two panels at the same edge overlap.
- No top or bottom dock: `[panel] side` has only `left` and `right`.
- No relaunch fallback. If the live re-dock fails in practice, a relaunch Pin is a new change.

## Decisions

### D1. The GUI links pinwin and hosts its proxy in-process

The GUI links pinwin's `embed` library and runs pinwin's proxy on one thread in the `mbv`
process. This amends ADR 0033 for the GUI only. A new ADR records it. The TUI keeps the external
program.

Alternatives:

- **The `pinwin` CLI, as the TUI does (rejected).** It needs one child process per request, so
  a live edge drag is not possible. It also needs `pinwin run` to start the app, so a pin of
  the running window is not possible.
- **A client of the pinwin daemon socket (rejected).** It keeps the process boundary and avoids
  the GPL, but a daemon can only proxy an app that it starts. The running GUI window cannot move
  behind it.
- **A winit fork with layer-shell, or a layer surface made with smithay-client-toolkit beside
  winit (rejected).** Blitz renders only into a winit window, and a fork of winit pins the whole
  window stack to mbv.

Consequence: the shipped `mbv` binary is a GPL-3.0-or-later combined work. The user accepted
this. The mbv source stays MIT. The packages ship the GPL text (D11).

### D2. The pinwin `embed` API that mbv needs

The API is a prerequisite pinwin change in `~/Dev/pinwin`, which only the user authorizes. mbv
needs this contract and nothing more:

```rust
pub mod embed {
    /// Owns the proxy thread. Dropping it ends the thread and closes the client end.
    pub struct Host { /* ... */ }

    impl Host {
        /// Connect upstream to `display` (a WAYLAND_DISPLAY value the caller read), check
        /// layer-shell, spawn the proxy thread, and return the client end of a socketpair.
        /// The thread reads no environment variable.
        pub fn start(display: &std::ffi::OsStr)
            -> Result<(Host, std::os::unix::net::UnixStream), StartError>;
        /// Arm one panel: the next toplevel the client opens docks with `options`.
        pub fn open_panel(&self, options: &PanelOptions) -> Result<(), PanelError>;
        /// End the panel when its window is destroyed; the next toplevel passes through.
        pub fn release_panel(&self) -> Result<(), PanelError>;
        pub fn set_size(&self, px: u32, reserve: Reserve, ms: Option<u32>)
            -> Result<(), PanelError>;
        pub fn set_edge(&self, edge: Edge) -> Result<(), PanelError>;
        pub fn set_gutters(&self, gutters: Gutters) -> Result<(), PanelError>;
        pub fn set_reserve(&self, reserve: Reserve) -> Result<(), PanelError>;
        /// The logical size of the panel's output, once known.
        pub fn output_size(&self) -> Option<(u32, u32)>;
    }

    pub struct PanelOptions {
        pub edge: Edge,              // Left | Right (Top | Bottom unused by mbv)
        pub size_px: u32,
        pub reserve: Reserve,        // Push | Cover | None
        pub focus_ring: Option<(u8, u8, u8, u32)>,
        pub gutters: Gutters,        // top, bottom, left, right: i32
        pub tween_ms: u32,
    }
    // Layer: overlay. Keyboard: on-demand. Fixed for the embed panel.
    // PanelError and StartError implement Display with pinwin's reason text.
}
```

The contract rules are these:

- Every control call returns after validation. It never waits for a compositor roundtrip, so the
  GUI can call it on the window thread.
- No call exits the process, and no panic crosses the API.
- A panic on the proxy thread ends the host and closes the client end.
- The embed build links no tray, no daemon socket, and no TOML or JSON code.

The dependency is this line:

```toml
pinwin = { git = "https://github.com/slatkin/pinwin", tag = "v0.5.0", default-features = false, features = ["embed"] }
```

pinwin is not on crates.io. The latest stable tag is `v0.4.3`, which has no library, so
`v0.5.0` is the first usable version. Through it, mbv also gets `wl-proxy 0.1.5` (the latest
release, MIT OR Apache-2.0) and `libc 0.2`. Both are already compatible with mbv's lockfile.
No other new dependency is added. The socketpair is `std::os::unix::net::UnixStream`.

### D3. The `PanelHost` seam

`crates/mbv-gui/src/window/pin/host.rs` declares `trait PanelHost` with the D2 control methods
(no `start`) and `enum PinAvailability { Available(Box<dyn PanelHost>), Unavailable }`. The
pin logic depends only on the trait. Tests use a recording mock that can answer a scripted
`Err`. The real `PinwinHost` adapter over `pinwin::embed::Host` lands in the last wiring unit
(task group 9). Until then `main.rs` passes `Unavailable`, which hides the Pin control.

### D4. Connection order in `main.rs`

The new start order is:

1. `claim_swap_token`.
2. `applog::init`.
3. Load `GuiLaunchState.pinned` and `GuiPanelWidths`.
4. If `WAYLAND_DISPLAY` is set, `Host::start(display)`. On an error, log one warning and use
   `Unavailable`.
5. On success, set `WAYLAND_SOCKET` to the client fd (`unsafe std::env::set_var` with a SAFETY
   note: no thread other than the Rust-only proxy and log threads exists).
6. Create the event loop. winit removes `WAYLAND_SOCKET`, and the proxy thread answers the
   roundtrip.
7. Spawn the core thread.
8. Run `launch` with that event loop.

`window/launch.rs` gets a `create_event_loop()` that `main.rs` calls in step 6, and `launch`
takes the loop. The core thread therefore spawns after `WAYLAND_SOCKET` is gone, so the Owner
and mpv never see it. `owner_link/start.rs` also removes `WAYLAND_SOCKET` from the Owner spawn,
as it removes the claimed panel variables.

Alternative: set the variable only for pinned launches (rejected). A live pin then needs a
process restart, which the user did not choose.

### D5. The live re-dock

A Pin or Unpin press sends `BlitzShellEvent::Embedder(Arc<RedockEvent>)` through the
`BlitzShellProxy` context. `MbvApplication::proxy_wake_up` handles it:

1. For a Pin, call `PanelHost::open_panel`. On an `Err`, send a warning toast through the
   core and stop (spec "A refused pin leaves the window as it was"). For an Unpin, call
   `release_panel`, and remember nothing else.
2. Take the old `View` out of `windows`. Move its `doc` out (swap in an empty `DioxusDocument`)
   and clone its `renderer`. Drop the old `View`, which suspends the renderer and destroys the
   toplevel.
3. Call `View::init` with `WindowConfig::with_attributes(doc, renderer, attributes)`. The
   attributes come from the pure `redock_attributes(pinned, restore_size)`: a pinned window has
   no minimum surface size, so the 72 px rail fits. An unpinned window has today's minimum and
   the logical size that it had before the pin.
4. Provide the shell provider, the renderer, and the new `Arc<dyn Window>` at the root scope
   again, insert the view under its new `WindowId`, and request a redraw.

The VirtualDom, every signal, and every component's local state live in the doc, so they
survive. `use_container_size` reads the window only to seed its first value, and later sizes
come through `ViewportSize`.

A `RedockGuard` in the pin state ignores focus events from the start of a re-dock until the new
window's first presented frame (spec "A focus change that the GUI's own re-dock causes").

The spike is task group 2. It lands the re-dock with no panel host: a press re-creates a normal
window. That proves the doc hand-over before any pinwin code exists.

### D6. The pin state and the width rules

`window/pin/state.rs` holds `PinState { Unpinned, Pinned { view: PinnedView } }` with
`PinnedView { Queue, Full }` in a window-scope `Signal`. The pure width function is:

```text
queue view width = queue_column_width(toggles, dragged_queue_width)   // 72 when collapsed
full view width  = saved full_view, else queue view width (expanded) + LIBRARY_WIDE_MIN_WIDTH_PX
```

The Full view width is clamped between the expanded queue width plus
`LIBRARY_MINI_MAX_WIDTH_PX` and the host's `output_size` width minus the gutters, when it is
known. `GuiPanelWidths` gets `full_view: Option<u32>` (serde default), and Reset UI State drops
it with the other widths.

The reserve mode for each request comes from the pure `pinned_reserve(view, config.reserve)`:
the Queue view uses `Push` when `reserve` is on and `Cover` when it is off, and the Full view
always uses `Cover`. This is the TUI's covering rule (`collapsed_reserve`).

The tween comes from the pure `pinned_tween(edge)`: `Some(0)` on a left dock, and `None` (the
panel's 200 ms) on a right dock. On a right dock both directions look right without a mirror. A
grow configures the GUI to the target width first, so the Library appears from the left. A
shrink crops from the right edge, so the Queue column stays visible. On a left dock the crop
would show the Library and hide the Queue, so the change snaps.

An effect in the root watches the view, the collapse toggle, and the dragged Queue width. It
sends one `set_size` when the computed width changes. The Owner and the core know nothing about
the pin.

### D7. The mouse transitions

- **Header.** `destination_pressed` gets the pin state. While pinned it does not touch
  `MiniView`. In the Queue view it sets the Full view after it activates the destination. In
  the Full view, a press on the already active destination sets the Queue view. An unpinned
  window keeps today's rule.
- **Play.** The core increments `user_play_epoch: u64` inside
  `collapse_pinned_view_after_play`, which every user play tail already calls after the Owner
  accepts the replace, and which the websocket Play handler never calls. The GUI frame projects
  the counter in the `panel` section. A changed value in the pinned Full view sets the Queue
  view. The counter is neutral: it names no Client.
- **Drawer.** `MbvApplication::window_event` forwards `WinitWindowEvent::Focused(false)` as
  `WindowEvent::FocusLost` on the window channel. The root sets the Queue view when the window is
  pinned, the view is Full, and the `RedockGuard` is clear. The GUI's menus and dialogs paint in
  the document, so they never move the keyboard focus.
- **Rail.** `Panel` masks `collapsed` with `library_visible || pinned`, and `QueueFooter` shows
  the collapse control on the same condition (issue #1066). A collapse in the Full view first
  sets the Queue view. The root does not mount `LibraryTopBar` while the window is pinned and
  collapsed.
- **Pin control.** `LibraryTopBar` paints a `HeaderIconButton` with the Lucide `pin` and
  `pin-off` glyphs beside the Settings control, only while the window has `Available`. The
  Lucide set adds both glyphs by name.

### D8. The inner edge handle

`window/pin/edge_handle.rs` paints a thin handle at the window edge away from the docked edge,
in the Queue view and the Full view. It reuses `PanelDivider`'s pointer capture and its
cancel-on-unmount rule. A window-relative x is not stable while a right-docked window grows
leftward, so the drag measures the pointer's distance from the docked edge: `x` on a left dock
and `width - x` on a right dock. The target width is that distance plus the offset at the
press.

Each pointer move sends at most one `set_size(target, reserve, Some(0))` per presented frame.
At the release, the Queue view saves through `PanelWidthsStore::drag_queue_width` (which clamps
to 400..800 px), and the Full view saves through a new `PanelWidthsStore::drag_full_view_width`.
A refused size keeps the last accepted width and shows no toast during the drag.

### D9. The Panel settings in the GUI

In `mbv-ui-model` `SettingKey::scope`, `Panel`, `PanelSide`, `PanelReserve`, the four gutters,
`PanelAccent`, `PanelAccentColor`, and `PanelAccentWidth` become `AllClients`. `PanelWidth` and
`PanelWidthExpanded` stay `TuiOnly`. The TUI ignores the scope.

The GUI sidebar lists Panel after Display. Side and Accent color are cycle controls, Reserve
space and Accent are toggles, and the gutters and Accent width are steppers with a minus and a
plus control. A Shift press steps by 10, as the TUI's Shift step does. Every Panel control sends
the new `GuiSettingsCommand::PanelStep { setting, delta }`, and the core thread calls
`App::apply_panel_setting(key, delta)`. For the core the GUI is unpinned, so that call validates
and saves.

While the window is pinned, the pin module handles a Side, Reserve space, or gutter step before
the core does:

1. Read the projected `PanelConfig` (a new field of the `panel` frame section).
2. Compute the candidate with `mbv_ui_model::settings::changed_panel_config(key, config, delta,
   None)`.
3. Call `set_edge`, `set_gutters`, or `set_reserve` on the host. A Reserve space step in the
   Full view calls nothing.
4. On `Ok`, send `PanelStep`. On `Err`, send the new `GuiPinCommand::Refused { reason }`, and
   the core thread calls `App::flash(reason, Warning)`. Nothing is saved.

An accent step while pinned sends `PanelStep` and `GuiPinCommand::AccentDeferred`, which shows
a neutral toast. The toast texts live in mbv-gui.

Alternative: make the core's pinned-request path a Client port (rejected). The core's pinned
logic also drives the TUI's `PanelMode`, `pinned_width`, and `x` binding, so a GUI port would
need a second meaning for each of them.

### D10. The pinned state at start and at exit

`GuiLaunchState` gets `pinned: bool` (serde default `false`). The orderly exit that already saves
the launch location also writes it. At start, when the value is `true` and the host is
`Available`, `can_create_surfaces` calls `open_panel` with the Queue view width before it calls
`View::init` with the pinned attributes. A refusal logs a warning and opens a normal window. The
saved value changes only at the next orderly exit.

### D11. Licence notices

`LICENSES/GPL-3.0.txt` holds the GPL text, copied from pinwin's `LICENSE`. It ships in the same
places as `LICENSES/MPL-2.0.txt`:

- the `.deb` assets in `crates/mbv-tui/Cargo.toml`, and the tarball step and its `dpkg-deb`
  check in `.github/workflows/build.yml`;
- `PKGBUILD` and `PKGBUILD-git`, with `license=('MIT' 'GPL-3.0-or-later')`;
- the `Makefile` install.

The `README.md` gets one sentence that says the `mbv` binary is distributed under
GPL-3.0-or-later because it links pinwin. The crate `license` fields stay MIT, because they
describe the source.

## Risks / Trade-offs

- [A pinwin proxy defect ends every Wayland GUI window, also for a user who never pins.] → The
  embed contract has no exit and no cross-API panic. A host that ends closes the connection,
  and the GUI exits as it does on a lost compositor. The Owner keeps playing, and the user can
  start the GUI again.
- [The live re-dock depends on Blitz internals of a pinned beta (`View` fields, renderer
  suspend and resume).] → The exact pins already freeze them, and task group 2 lands the
  re-dock first with no pinwin code. If the re-dock fails, the relaunch Pin becomes a new
  change.
- [The prerequisite pinwin `v0.5.0` does not exist yet.] → The `PanelHost` seam lets task groups
  1 to 8 land and pass the gates against the mock. Only task group 9 needs the tag.
- [`WAYLAND_SOCKET` is set while two Rust threads run.] → Both threads are Rust-only, and std
  serializes its own environment access. The variable is gone before the core thread spawns,
  so no C library on another thread reads it.
- [A GUI panel and a TUI panel at the same edge overlap.] → This is a documented limit (Non-Goals).
- [The Full view at the natural Wide width can be wider than a small output.] → The width clamps
  to the host's output width, and pinwin's validation refuses what is left.

## Migration Plan

Additive. Old `gui-launch-state` and `gui-panel-widths` files load with `pinned = false` and
`full_view = None`. There is no config key migration: the GUI reads the existing `[panel]` keys.
To roll back, revert the change. A saved `pinned = true` is then ignored as an unknown field.
