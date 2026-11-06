# GUI Client guide

Read before any change to `crates/mbv-gui/`: the core thread, a bridge DTO, a
frame section, a leaf, a reported region, the theme, or the front end. Term
definitions live in `CONTEXT.md` and win any disagreement with this file. The
`gui-client` spec and the `add-tauri-gui-client` design own the requirements
and the decisions; this file describes what the code does.

The GUI Client is the Tauri 2 program `mbv-gui` (`crates/mbv-gui/`). It is a
workspace member outside `default-members`: the default `cargo build` and
nextest runs skip it, while `--workspace` clippy and fmt cover it. Every
`--workspace` build compiles it, so it needs WebKitGTK 4.1 and a built `ui/`
(`tauri::generate_context!` fails when `frontendDist` does not exist). The
`make ui-install`, `ui-build`, `ui-check`, `ui-lint`, and `ui-test` targets
wrap the pnpm steps. `make dev` builds `ui/` and starts the window with
`cargo run -p mbv-gui`.

## Process shape: one core thread

Tauri owns the main thread and the window's event loop. `src/main.rs` builds
the Tauri application, registers the three bridge commands, starts the core
thread in `setup`, turns the webview's context menu off, and joins the core
thread after the event loop ends so a quit's exit teardown completes before
the process exits.

`src/core_thread.rs` runs one dedicated, synchronous thread. It loads the
configuration, declares the GUI Client (`declare_gui_client`), runs the Owner
attach campaign (`src/attach.rs`), and then runs `ClientCore::tick` in a
loop. The thread waits on a `std::sync::mpsc` channel of `CoreInput` values
(`Key`, `Pointer`, `Layout`, `Wake`, `Close`) with a timeout of
`CLIENT_IDLE_CADENCE`. The installed `ClientWaker` sends `Wake` into the same
channel, so Owner events and timers wake the core as they wake the TUI.
`CoreThread` implements `ClientTickHost` over the slice's Surface state; the
drains for surfaces the first slice does not present are documented no-ops.
The card-image completion drain (`App::drain_card_image_completions`, the
shell drain without the TV-workspace re-push) runs because the Queue
playback visual slot paints: it lands the slot's artwork fetches in the
image cache the frame encodes from.

The Tauri command handlers never touch the `App`. They only translate the
input and push it into the channel. tokio stays inside Tauri's runtime; no
mbv code in `mbv-gui` is `async` apart from what a Tauri command signature
requires (design D12 — `mbv-gui` is a tokio edge, like `mbv-desktop`).

The core thread emits a `GuiWindowFrame` after every tick that changed state,
and after every `report_layout`, changed or not. The front end sends
`report_layout` when it mounts, so the first frame and the frame after a
window-content reload never wait for a state change. The window's `Destroyed`
event delivers `CoreInput::Close`; the run ends as a quit, and the exit
boundary runs `client_exit_teardown` before the `App` drops.

## The bridge contract

Rust to front end there is one event, `mbv://frame` (`FRAME_EVENT` in
`src/main.rs`). It carries a `GuiWindowFrame` (`src/bridge/frame.rs`): either
`Slice(GuiFrame)` for the attached window or `Attach(GuiAttachFrame)` for a
pre-attach state. The front end keeps only the last frame
(`ui/src/lib/bridge/frames.ts`), so each frame carries everything the window
paints: the focused surface and Panel (`GuiFocus`, mirroring `ComponentId`
and `PanelFocus`), the Queue viewport rows with their stable `QueueSlotId`
raw numbers, cursor, marks, active slot, and each item row's zebra parity
(`GuiQueue`), the Queue playback projection (`GuiQueuePlayback`), the
footer pills (`GuiQueueFooter`), the Status bar projection
(`GuiStatusBar`), the breakpoint class (`GuiBreakpoint`, mirroring
`BreakpointClass`), and the theme when it is armed (`GuiTheme`).

`GuiQueuePlayback` carries two facts beside the transport that the TUI's
queue band also paints. The status-indicator cluster (`statusIndicators`)
is the resolution, audio, and subtitle labels the shared projection builds
from the same indicator data the TUI's spans build from: the frame carries
each uppercased label with its foreground role, the field is `None` while
the local player is inactive, and the band paints the cluster right-aligned
on its controls row as bold glyph-free text — the TUI's per-style
treatments (chips, brackets, dots, pipes) stay terminal presentation. The
visual slot's artwork (`artwork`) follows the theme's arming rule (design
D10): `src/artwork.rs` memoizes the slot's encoded art per state — the
cached art's display encode, the bundled placeholder, a loading fill, or
hidden — and a frame carries the value again only when the slot's art
changed, while a window-content reload's `report_layout` re-arms it and the
front end keeps the last carried value between frames. The payload is a
`data:` URL of bytes the core's image cache already holds, never a fetch
the webview makes, so the CSP grows only `img-src 'self' data:`. The band
keeps the TUI's width-driven arrangement
(`queue_playback_transport_area` over `queue_playback_column_wide`):
below 100 column cells the art centers above the stacked transport rows,
and at 100 and over the art sits left-aligned with the transport beside
it after the two-column `SLOT_TRANSPORT_GAP`, the taller of the two
governing the band's height. The band reports the slot's `visualSlot`
region (the TUI's hit target), and the art element's resizes report the
regions a swapped bitmap moved.

Front end to Rust there are three commands, registered in
`src/main.rs`. Each only pushes into the core channel:

- `key(DomKey)`: the DOM `KeyboardEvent` fields. `src/bridge/keys.rs`
  translates them to a neutral `KeyEvent`, following the TUI's crossterm
  edge: SHIFT folds an uppercase letter to lowercase, Shift+Tab is `BackTab`,
  Meta produces `Key::Null` (the neutral modifiers have no Meta bit), and a
  key the vocabulary cannot name is `Key::Null` too.
- `pointer(DomPointer)`: kind (press, release, move, wheel), button, held
  buttons, position in logical pixels, and wheel deltas.
  `src/bridge/pointer.rs` translates to `Option<PointerEvent>`, mirroring the
  TUI's `neutral_pointer`: a move with the primary button held is the neutral
  drag, the vertical delta's sign picks the wheel direction, and an event the
  neutral vocabulary cannot represent drops.
- `report_layout(LayoutReport)`: whole visible rows per list, the painted
  regions with their semantic target ids, and the window's inner width in
  logical pixels (`src/bridge/layout.rs`).

The front end interprets no input. `ui/src/lib/bridge/input.ts` prevents the
default action of every keydown (no webview default such as reload, find,
zoom, or back navigation runs) and forwards pointerdown, pointerup,
pointermove, and wheel events; it converts a wheel delta in lines or pages to
logical pixels before it sends. The core's `PointerGestureState` recognizes
the gesture; a recognized gesture's point resolves against the combined hit
map of the last `LayoutReport`; the core applies the focus change the
matching TUI click applies; and `App::route_gui_pointer` receives the
surface that owns the region as its leaf. A point in no reported region
resolves to nothing. The leaf adapters (`src/leaves.rs`: `QueueLeaf`,
`PlaybackLeaf`, `StatusBarLeaf`) are built per routing call over the core's
fields and emit the same semantic `Msg`s the TUI components' pointer paths
emit.

## Layout reports and the hit map

`ui/src/lib/bridge/layout.ts` is the single layout registry. Every mounted
surface registers a contribution of list capacities and painted regions
under a `Symbol` key; the reporter merges all contributions into one
`LayoutReport` and sends it through `report_layout`. Reports coalesce to one
send per turn of the event loop, and a contribution identical to its last
value sends nothing — a frame that moves only the cursor moves no region.
The core rebuilds its whole hit map from each report, so a surface must
never send a partial one: report on mount, report after each paint that
moved a region, and `retractLayout` on unmount. Even a surface with no
interactive regions reports its (empty) contribution, so the merged report
accounts for it.

The core's `apply_layout` (`src/core_thread.rs`) does three things: a list's
`whole_rows` becomes the surface's viewport height (the paging stride and
resting-scroll clamp — `QueueSurface::set_viewport_height`); the reported
regions rebuild the combined `HitMap<RegionTarget>` (`hit_map` in
`src/bridge/layout.rs`, in paint order); and the reported width classifies
through the shared `mbv-surfaces` breakpoint `classify` with the GUI's own
thresholds (`GUI_BREAKPOINTS`: Mini below 720, Wide from 1200 logical
points). The `RegionTarget` ids mirror the surface hit-target enums variant
for variant — `QueueTargetId` names `QueueHitTarget`,
`QueuePlaybackTargetId` names `QueuePlaybackHitTarget`,
`StatusBarTargetId` names `StatusBarHitTarget` — plus `AttachTargetId` for
the pre-attach buttons.

A GUI list paints only the viewport: fixed-height rows, no native DOM
scroll. The core's viewport decides which rows the frame carries; wheel
input goes to the core like any pointer event, so scroll follows the cursor
through the same Surface state rules as the TUI. The front end measures the
list's height and reports whole rows as capacity.

## Theme mapping

The frame's `GuiTheme` (`src/bridge/frame/theme.rs`) carries two things. The
`roles` map holds one colour per role: `GuiRole` mirrors `mbv_theme::Role`
variant for variant — `Role` stays foreground-only — and the two `From`
conversions are exhaustive, so a new role fails the build until both sides
carry it (`GuiRole::ALL` is the one unchecked list; add the role there too).
The `chrome` section carries the eight fill values the shadcn-svelte
chrome variables point at (`background`, `card`, `popover`, `muted`,
`secondary`, `sidebar`, `border`, `input`) plus the queue column's own
surfaces: `queueColumn` and `queueColumnFocused`
(`Surface::QueueColumn`'s resting and focused fills, the gutter the
column paints), `queuePanel` and `queuePanelFocused`
(`Surface::QueuePanel`, the recessed box), `queuePlaybackBand`
(`Surface::QueueOnlyPlaybackPanel`, the fixed fill of the header row and
transport band), and `queueFooterBand` (`Surface::QueuePanelBand`, behind
the footer pills). Each chrome fill resolves the `Surface` the TUI paints
the same concept with through `surface_colors` — the focus-reactive
pairs carry both slots, and the front end picks between them on the
frame's Panel-focus fact — so the `mbv-theme` surface table stays the
one fill authority and a palette edit recolours the chrome together with
the roles.

The theme rides a frame only when armed: the first frame, and the first
frame after each `report_layout`, carry it — a window-content reload
re-arms it through the mount-time report. `ui/src/lib/bridge/theme.ts`
writes each role and chrome fill into a `--mbv-<name>` custom property on
`:root`, and `ui/src/app.css` points every shadcn-svelte variable at the mbv
properties. There is one palette and no light or dark variant.

## Text and icons

The front end sets its text face to the bundled SUSE variable font
(`@fontsource-variable/suse`; `font-family: 'SUSE Variable'` with
`system-ui` as fallback, in `app.css`) and keeps `ui-monospace` for
monospace text. The woff2 files build into `ui/dist`, so the window fetches
no font at runtime and the CSP's `default-src 'self'` covers `font-src`.

The GUI paints no emoji and no Nerd Font glyph. Icons come from
`@lucide/svelte` only: the transport controls, the playing-row marker, and
the playback-target indicator are Lucide icons. The transport glyphs paint
as solid shapes — `fill` and `stroke` both the role colour, as the TUI's
solid transport glyphs paint — with the availability the colour already
carried gating them. The header's playback-target indicator maps both of
`target_icon`'s states to the one Lucide `Cast` — the TUI paints the
cast family for local and remote alike — so the role colour and the
host label carry the distinction. `GuiFooterPill.text` carries glyph-free
text; each pill's slot maps to its Lucide icon in the component —
`ListMusic` for the playlist, `Cable` for the owner indicator,
`SaveCheck` for autosave, `SaveOff` for the dirty pill — with the label
beside the icon where the frame carries one.

## The queue-only window

The slice's one window paints the TUI's queue-column arrangement
(`App.svelte`) in cell units: `ui/src/lib/bridge/cells.ts` fixes one TUI
text row at 28 logical pixels (the Queue list's row height) and one text
column at 14 (the terminal cell's 1:2 aspect at that height), writes them
to the `--mbv-cell-row` and `--mbv-cell-col` properties, and every
queue-column offset a component paints — the bands' two-column insets,
the one-row pads and gaps, the rows' two-column indent, the six-column
date gutter — is a `calc()` over the two, mapped from
`crates/mbv-render/src/arrangements/chrome.rs`, `arrangements/queue.rs`,
and `components/media_list/row.rs`.

The queue column's backdrop — the `queueColumn` pair, picked on the
frame's Panel-focus fact — owns the window. The Queue playback band sits
one row below the column's top edge and two columns inside it with a gap
row below. Its header row paints in every state — target icon, remote
host name, right-aligned status word — and the now-playing title paints
on its own row below it: where the TUI composites the title into the
artwork's top text row with a scrolling marquee, the GUI truncates it
with CSS ellipsis. The transport band keeps the TUI's width-driven
arrangement (100 column cells): below it the visual slot's art centers
above the stacked band rows, and at and over it the art sits
left-aligned with the band rows beside it after the two-column
`SLOT_TRANSPORT_GAP`, the taller of the two governing the band's
height. The art has no fixed pixel box: the slot sizes from the TUI's
`queue_card_height_cap` tiers — 12 rows under 40 terminal rows of window
height, 16 under 50, 24 otherwise — read off the window's own height in
row units.

The Queue panel's recessed box sits another two columns inside the
placement on the `queuePanel` pair, with its one-row top pad and bottom
spacer, and the item rows zebra-stripe as the TUI's `media_list/wide.rs`
walk paints them: the frame carries each row's parity — a grouped list
alternates within each group from its first member, an ungrouped list
throughout — and a striped row takes the `queueColumn` fill inside the
box. The footer row
paints below the box between gap rows on the `queueFooterBand` fill. The
list paints no scrollbar column yet — the TUI reserves one only while
the list is focused and overflows. The Status bar floats over the
placement's bottom pad row instead of reserving a band.

## The Status bar

`GuiStatusBar` mirrors the shared `StatusBarProjection`: the prefix-armed
state and the live toast with its severity. `StatusBar.svelte` shows the
armed pill while prefix mode is armed, and a live toast takes over the row
in its severity colour, as the TUI's status bar paints it. The bar
reserves no band: it floats over the queue placement's bottom pad row —
the TUI's queue-only mode paints no status bar there — so the queue
footer keeps its one-row clearance and a toast's arrival moves no
layout. Its fill (the `StatusBar` surface's slot, carried on
`--mbv-pillSelectedFg`, inset two columns each side like the TUI's
`status_bar_row`) paints only while the bar has content; an idle bar
paints nothing. The spec permits the GUI to show toasts in
a conventional GUI form that TUI geometry does not constrain, so a toast
presentation outside the bar is lawful.

## The attach states

Before the attach there is no `App`, so `src/attach.rs` (`AttachCampaign`)
owns the pre-attach phase on the core thread. It emits `GuiAttachFrame`
states — the two refusal texts, the waiting state while an Owner shuts
down, the build mismatch, and the attach-failed text — the same texts
`startup::run_local_instance` gives the TUI Client. The mismatch state
answers key R (Restart) and key Q (Quit) and resolves presses on the
reported `Attach` button regions against its own hit map. Dead-end states
park the window: they serve only `report_layout` (re-showing the state with
its theme) and `Close`.

The core thread does not run the TUI's post-attach service bootstrap
(`spawn_startup_services`, `fetch_home_at_startup`, `init_idle_feed`,
capability registration). Those steps belong to the TUI shell `Model`, and
the first slice presents no library surface. A later change that ports a
library surface must first factor those steps into a shared Client step; it
must not duplicate them in `mbv-gui` (design D9).

## Adding a surface to the GUI

A surface joins the GUI slice in this order. A surface the GUI does not yet
present stays absent — no stub, no placeholder (the `gui-client` spec).

1. **Hold the interaction state once.** The surface's cursor, selection,
   viewport, and drafts live in `mbv-surfaces` as Surface state, shared by
   both Clients (`docs/architecture/interactive-surface-ledger.md` names
   the owner). If a TUI component still owns the state, port it to Surface
   state first (`docs/architecture/tui-frontend.md`, *Surface state and
   adapters*). Never reimplement an interaction rule in `mbv-gui`.
2. **Carry it in the frame.** Add a serde DTO under `src/bridge/frame/`,
   built from the shared projection or the adopted Surface state, and a
   field on `GuiFrame`. Keep text glyph-free.
3. **Name its regions.** Add the target ids to `src/bridge/layout.rs`,
   mirroring the surface's hit-target enum variant for variant, and a
   `ListId` variant when the surface reports whole-row capacity.
4. **Adapt the leaf.** Add a `GuiSurfaceLeaf` in `src/leaves.rs` that
   resolves gestures against the reported regions and emits the same
   semantic `Msg`s the TUI component's pointer path emits. A control whose
   TUI target the GUI has not ported is a documented no-op, never a
   wildcard.
5. **Wire the core.** Mount the `ComponentId` in the core's `FocusStack`,
   hold the Surface state, adopt its projection once per emitted frame (as
   `sync_queue_surface` does), and route keys and resolved gestures with
   the leaf.
6. **Paint it.** Add a Svelte component under `ui/src/lib/components/` that
   paints the frame section, reports its list capacities and painted
   regions through `reportLayout`, retracts on unmount, and mounts in
   `App.svelte`. Icons are Lucide only. Every control a pointer can reach
   must keep a key chord (the `gui-client` spec: every action is reachable
   from the keyboard).
7. **Prove the contract.** Rust side, a nextest in `crates/mbv-gui/tests/`
   for a mapping that is new (region-to-`Msg`, capacity applied). Front
   end, a vitest asserting delivery and structure, not appearance.

## Tests and checks

The Rust tests (`crates/mbv-gui/tests/`, nextest) mock the channel, the
frame sink, and the Owner steps: no Tauri runtime, no webview, no live
Owner. The front-end tests (vitest with `@testing-library/svelte`,
`@tauri-apps/api` mocked) assert delivery and structure. Live behaviour in
the real window is the user's manual check.

The crate's logs go to `mbv-gui.log` in the mbv state directory, not the
TUI's `mbv.log`.

## WebKitGTK renderer note

On some GPU and Wayland setups, WebKitGTK's DMA-BUF renderer shows a blank
window. The workaround is `WEBKIT_DISABLE_DMABUF_RENDERER=1` in the
environment that starts `mbv-gui`. The app does not set it by default
(design, *Risks*).
