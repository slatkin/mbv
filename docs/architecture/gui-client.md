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
wrap the pnpm steps. `make dev` builds `ui/`, builds `mbv` and `mbv-gui`
together, and starts the window with `cargo run -p mbv-gui`. Both binaries
must come from one build: the GUI starts its Owner from the sibling `mbv`
binary, and the handshake refuses an Owner with a different build id.

## Process shape: one core thread

Tauri owns the main thread and the window's event loop. `src/main.rs` builds
the Tauri application, registers the four bridge commands, starts the core
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

When the Owner process itself exits while the window is open, the Client
closes with it, exactly as the TUI Client does. The daemon's deliberate
shutdown is announced before its socket closes (`daemon-disconnect-`
`handling`: "A deliberate shutdown is announced before the socket closes"),
and the announce reaches the GUI's App through the same shared drain half
that serves the TUI: `PlayerEvent::DaemonShutdownAnnounced` sets the
run-loop quit signal the TUI's `'outer` loop breaks on
(`src/app/infra/signals.rs`), the GUI core's `step` polls it beside the
tick outcome, and the run ends through the quit edge — the window closes,
and the run's exit boundary saves the GUI's launch location first
(design D9), then runs `client_exit_teardown`. (`gui_core.run_quit` logs
the reason with the App's exit message.) An unannounced ctrl-link loss is
not an exit: the App raises the DaemonLost recovery modal
(`daemon-disconnect-handling`: "An unannounced connection loss raises a
recovery dialog"), and the GUI presents it as the blocking
`GuiOverlay::DaemonLost` overlay (`DaemonLostOverlay.svelte`, the
`daemonLost` frame section). The window stays open and dead until one of
the dialog's three choices answers it, and each answer runs the shared
`App::apply_daemon_lost_intent` — the same action the TUI's modal handler
runs — so the GUI holds no recovery path of its own.

## The bridge contract

Rust to front end there is one event, `mbv://frame` (`FRAME_EVENT` in
`src/main.rs`). It carries a `GuiWindowFrame` (`src/bridge/frame.rs`): either
`Slice(GuiFrame)` for the attached window or `Attach(GuiAttachFrame)` for a
pre-attach state. The front end keeps only the last frame
(`ui/src/lib/bridge/frames.ts`), so each frame carries everything the window
paints: the focused surface and Panel (`GuiFocus`, mirroring `ComponentId`
and `PanelFocus`), the Queue viewport rows with their stable `QueueSlotId`
raw numbers, cursor, marks, active slot, row hover, row drag, and each item
row's zebra parity (`GuiQueue`), the queue header's remaining count, next
item title, finish clock, right-column mode, and filter text with its caret
fact (`GuiQueueSummary`), the collapse and queue-fold toggles the Client
core holds (`GuiPanelToggles`), the open menu or popover (`GuiOverlayFrame`:
the row menu with its entries, key hints, and destructive flags, the track
popover, the playback-target chooser, the shared confirmation modal with
its message and button row, and the daemon-lost recovery dialog with its
diagnostics and recovery choices), the Queue playback projection
(`GuiQueuePlayback`), the footer pills (`GuiQueueFooter`), the Status bar
projection (`GuiStatusBar`), the Library panel's section (`GuiLibraryFrame`;
*The Library panel* names it), the Settings screen's section
(`GuiSettingsFrame`; *The Settings screen* names it), the UI-reset counter
(`uiResetEpoch`), and the theme when it is armed (`GuiTheme`).
The window picks the panel state from the frame's toggles and its own
measurement of the panel container; the frame carries no breakpoint class
(design D10).

`GuiQueuePlayback` carries the player panel's facts. The status-indicator
cluster (`statusIndicators`) is the resolution, audio, and subtitle labels
the shared projection builds from the same indicator data the TUI's spans
build from: the frame carries each uppercased label with its foreground
role, and the field is `None` while the local player is inactive. The canvas
player panel paints no cluster, so the field travels unused in the slice. The
visual slot's artwork (`artwork`) follows the theme's arming rule (design
D10): `src/artwork.rs` memoizes the slot's encoded art per state — the
cached art's display encode, the bundled placeholder, a loading fill, or
hidden — and a frame carries the value again only when the slot's art
changed, while a window-content reload's `report_layout` re-arms it and the
front end keeps the last carried value between frames. The payload is a
`data:` URL of bytes the core's image cache already holds, never a fetch
the webview makes, so the CSP grows only `img-src 'self' data:`. The panel
reports the slot's `visualSlot` region (the TUI's hit target), and the art
element's resizes report the regions a swapped bitmap moved.

The scrubber's hover preview (`hover`) carries the pointer's fraction along
the painted bar, the time it names, the covering chapter, and the trickplay
tile at that position. The tile (`GuiScrubberTile`) names the thumbnail's
place in its sheet — the sheet key, the thumbnail's pixel width and height,
the sheet's columns, and the thumbnail's column and row — plus the sheet as
a `data:image/jpeg` URL. `src/trickplay.rs` holds the sheet's encoded URL
per key and a frame re-carries the image only when the hovered sheet
changes; the front end latches the last image per key, so the preview paints
the tile by cropping the sheet as a CSS sprite and the window fetches no
image. A move away from the bar carries no hover. An item with no
trickplay, a sheet not yet cached, or a tile the server does not hold paints
the time and the chapter without a tile.

Front end to Rust there are five commands, registered in
`src/main.rs`. Each only pushes into the core channel:

- `key(DomKey)`: the DOM `KeyboardEvent` fields. `src/bridge/keys.rs`
  translates them to a neutral `KeyEvent`, following the TUI's crossterm
  edge: SHIFT folds an uppercase letter to lowercase, Shift+Tab is `BackTab`,
  Meta produces `Key::Null` (the neutral modifiers have no Meta bit), and a
  key the vocabulary cannot name is `Key::Null` too.
- `pointer(DomPointer)`: kind (press, release, move, wheel, cancel), button,
  held buttons, position in logical pixels, and wheel deltas.
  `src/bridge/pointer.rs` translates to `Option<PointerEvent>`, mirroring the
  TUI's `neutral_pointer`: a move with the primary button held is the neutral
  drag, the vertical delta's sign picks the wheel direction, and an event the
  neutral vocabulary cannot represent drops. A `cancel` is the one kind no
  button or position decides: the front end sends it when the webview loses
  the gesture — a `pointercancel`, a `pointerleave` with a button held, or the
  window's blur — and the core drops the drag state and sends no move.
- `report_layout(LayoutReport)`: whole visible rows per list, the painted
  regions with their semantic target ids, and the panel container's painted
  size in logical pixels (`src/bridge/layout.rs`).
- `library(GuiLibraryCommand)`: the Library panel's one semantic request,
  pushed into the core channel as `CoreInput::Library` (*The Library panel*
  names the variants).
- `settings(GuiSettingsCommand)`: the Settings screen's one semantic request,
  pushed into the core channel as `CoreInput::Settings` (*The Settings screen*
  names the variants).

The front end interprets no input apart from the Library panel's own keys and
gestures (*The Library panel*). `ui/src/lib/bridge/input.ts` prevents the
default action of every keydown (no webview default such as reload, find,
zoom, or back navigation runs) and forwards pointerdown, pointerup,
pointermove, and wheel events; it converts a wheel delta in lines or pages to
logical pixels before it sends. A keydown meets the window's key router first
(`KeyRouter`), which `App.svelte` installs, and only a key the router leaves to
the core reaches the `key` command. A lost gesture — a `pointercancel`, a
`pointerleave` with a button held, or the window's blur — forwards the
`cancel` kind. The core's `PointerGestureState` recognizes
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
`src/bridge/layout.rs`, in paint order); and the reported container size is
stored for the frame. The core does not classify the size. The front end
picks the panel state from the same measurement, and the reported capacity
and regions already measure the layout that size painted (design D10). The
`RegionTarget` ids mirror the surface hit-target enums variant
for variant — `QueueTargetId` names `QueueHitTarget`,
`QueuePlaybackTargetId` names `QueuePlaybackHitTarget`,
`StatusBarTargetId` names `StatusBarHitTarget` — plus `ConfirmTargetId` and
`DaemonLostTargetId`, the confirmation modal's and the daemon-lost
dialog's buttons by index, and `AttachTargetId` for the pre-attach buttons.
A confirm or daemon-lost button owns no surface: the core resolves its
press itself (`App::execute_gui_confirm_button`,
`App::execute_gui_daemon_lost_button`) and mounts no leaf for it.

A GUI list paints only the viewport: fixed-height rows, no native DOM
scroll. The core's viewport decides which rows the frame carries; wheel
input goes to the core like any pointer event, so scroll follows the cursor
through the same Surface state rules as the TUI. The list's rows are 30
logical pixels tall (`QueuePanel.svelte`), and the front end measures the
list's height and reports whole rows as capacity. `App.svelte` measures the
panel container with a `ResizeObserver` and reports that size through the
same reporter (`reportContainerSize`), so the state it paints and the
capacity the core applies come from one measurement.

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
(`Surface::QueueOnlyPlaybackPanel`, the fill of the compact strip and the
collapsed rail), and `queueFooterBand` (`Surface::QueuePanelBand`, behind
the footer pills). Each chrome fill resolves the `Surface` the TUI paints
the same concept with through `surface_colors` — the focus-reactive
pairs carry both slots, and the front end picks between them on the
frame's Panel-focus fact — so the `mbv-theme` surface table stays the
one fill authority and a palette edit recolours the chrome together with
the roles.

The queue's canvas paint also uses colours that name no `mbv-theme` slot.
`app.css` carries them as GUI-local `--canvas-*` custom properties — the
header, rows, footer, hover, row menu, popover, and drag chrome — and every
component references a token, never a literal hex. No `theme.rs` remap rides
along: `mbv-theme` stays the TUI's palette (`align-gui-with-design-canvas`
decision DR9). One canvas value stays deliberately off the tokens: the
panel background keeps its `mbv-theme` role (`#272e33`) while the canvas
paints `#2d353b` — the change's accepted open risk, left for the user's
live check.

The theme rides a frame only when armed: the first frame, and the first
frame after each `report_layout`, carry it — a window-content reload
re-arms it through the mount-time report. `ui/src/lib/bridge/theme.ts`
writes each role and chrome fill into a `--mbv-<name>` custom property on
`:root`, and `ui/src/app.css` points every shadcn-svelte variable at the mbv
properties. There is one palette and no light or dark variant.

## Text and icons

The front end bundles the two canvas faces (design D8): IBM Plex Sans at
weights 400, 500, and 600 for text, and JetBrains Mono at the same weights
for times, key hints, chips, and counters, with tabular figures. `main.ts`
imports the latin woff2 subsets from `@fontsource/ibm-plex-sans` and
`@fontsource/jetbrains-mono`, and `app.css` sets `--font-sans` to
`'IBM Plex Sans', system-ui, sans-serif` and `--font-mono` to
`'JetBrains Mono', monospace`. The woff2 files build into `ui/dist`, so the
window fetches no font at runtime and the CSP's `default-src 'self'` covers
`font-src`.

The GUI paints no emoji and no Nerd Font glyph. Icons come from
`@lucide/svelte` only: the transport controls and the playback-target
indicator are Lucide icons. The now-playing row shows the canvas's play glyph
beside its title, in the accent colour (the `GuiQueue` frame still names
`active`). The row keeps its own fill, because a highlight marks focus only. The transport glyphs paint
as solid shapes — `fill` and `stroke` both the role colour, as the TUI's
solid transport glyphs paint — with the availability the colour already
carried gating them. The header's playback-target indicator maps both of
`target_icon`'s states to the one Lucide `Cast` — the TUI paints the
cast family for local and remote alike — so the role colour and the
host label carry the distinction. `GuiFooterPill.text` carries glyph-free
text. The autosave and dirty pills map to their Lucide icons in the
component (`SaveCheck` and `SaveOff`); the playlist pill and the session
marker paint plain label text — the marker adds its liveness dot — with
no leading icon, as the canvas draws them.

## The panel's four states

`ui/src/lib/bridge/panel.ts` picks one of four states (design D10):
expanded, compact strip, expanded without queue, and rail. The state comes
from the measured size of the panel container plus the frame's two toggles.
It never comes from the window size, and the window holds no interaction
state: the size is a measurement and the toggles are the Client core's.
`App.svelte` measures the container with a `ResizeObserver` and reports the
size through `reportContainerSize` — the same measurement
`LayoutReport.containerSize` carries to the core — so the painted state, the
list capacity, and the hit map follow one size.

The `z` toggle (`GuiPanelToggles.collapsed`) wins over every size and paints
the 72 px rail: a progress ring around the artwork, play/pause, next, the
remaining time, and a control that expands the panel again. A measured
height below 520 px paints the compact strip: the 96×54 still, the title,
the parent line, play/pause, next, and a thin progress line. Its next-item
line reads `GuiQueueSummary.nextTitle`, which the core resolves from the
queue rather than from the painted rows, because the strip paints no queue
list. The `q` toggle (`GuiPanelToggles.queueFolded`) paints the expanded
artboard with the queue replaced by its one-line summary. A height of zero
reads as "not measured yet" and paints the expanded artboard, so a reload
does not flash a strip. `Panel.svelte` holds the artwork and the trickplay
sheet above the state branches, because a frame re-carries each only when it
changed: a state that mounts on a later frame paints the value the expanded
artboard already adopted.

`tauri.conf.json` declares a 400×200 minimum window size, so a floating
window cannot shrink past a usable state. The width is the canvas
artboard's authored 400 px; the height sits under the 520 px strip
threshold, so the floor leaves every state reachable — the rail through
`z`, the strip by size, the expanded artboard above it. A tiling manager
may impose its own larger floor; that floor is the compositor's, not the
window's.

## The player panel and the queue

The slice's one window paints the design canvas's player panel on the queue
column's backdrop (`App.svelte`, `Panel.svelte`).
`ui/src/lib/bridge/cells.ts` fixes one TUI text row at 28 logical pixels and
one text column at 14 (the terminal cell's 1:2 aspect at that height), and
every panel offset — the paddings, the gaps, the queue box's inset — is a
`calc()` over the `--mbv-cell-row` and `--mbv-cell-col` properties.

The `align-gui-with-design-canvas` change aligned the queue's paint with the
canvas and nothing else: a paint-only restyle of the queue's four bands
with no new command, derivation, key, or contract. The artboards (`Main.dc.html`, `Menus.dc.html`)
and the change's gap audits, copied into that change's directory under
`openspec/changes/align-gui-with-design-canvas/`, are the reference for
the exact values; this file records what the paint changed, not every
canvas value.

The queue column's backdrop — the `queueColumn` pair, picked on the frame's
Panel-focus fact — owns the window. The player header row paints the
playback target (its icon, the host label, and the collapse and queue-fold
controls) and the right-aligned status word. A press on the target opens the
`targetChooser` overlay, whose rows carry the choices with the attached
output marked; Esc or a press elsewhere closes it in the core. The artwork
follows the header with the season-and-episode badge, and the title and
parent line sit below it: where the TUI composites the title into the
artwork's top text row with a scrolling marquee, the GUI truncates it with
CSS ellipsis. The art has no fixed pixel box: the slot sizes at most 24 row
units of the cell grid.

The scrubber paints the played fraction, the chapter boundaries, the intro
and credits mark segments, the thumb, and the elapsed, remaining, and finish
clock texts. A press on the bar seeks through the core. The transport row
below it paints previous, back 10 s, play/pause, forward 30 s, and next as
solid Lucide shapes, with the availability that the colour carries. The
track chips row sits under the transport: the video stream's resolved label,
and the audio and subtitle chips, each opening its `trackPopover` overlay
with the choices and the chosen one marked. Both rows are absent while the
local player is inactive.

The Queue panel's recessed box sits below the player block on the
`queuePanel` pair, with its header row, its rows, and its bottom spacer. The
header paints the remaining count, the queue finish clock, the Length /
Ends-at switch, and the filter control; the switch, the filter, and each row
report their own region. The item rows zebra-stripe as the TUI's
`media_list/wide.rs` walk paints them: the frame carries each row's parity —
a grouped list alternates within each group from its first member, an
ungrouped list throughout — and a striped row takes the `queueColumn` fill
inside the box. Each row paints the canvas row: its padded mono ordinal in
place of the date gutter, one title line over a 2 px progress track, and
the right-column value — no second text line. The rows paint at the
canvas's fixed 30 logical pixel height. The list paints no scrollbar column
yet — the TUI reserves one only
while the list is focused and overflows.

The row hover and the drag paint from the frame, and the window holds
neither. `GuiQueue.hover` names the row the pointer rests on: that row shows
the grip and the row-actions button. The button opens the `rowMenu` overlay,
whose entries — Play now, Play next, Mark watched, Go to episode, Remove
from queue, and Clear queue — carry their key hints; the menu paints the
canvas's return glyph `⏎` where the core's hint chord is `Enter`, at paint
time only, so `direct_select` and the key routes keep matching the raw
chord. `Clear queue` lists while the queue holds any slot, and it raises the
same prompt the `c` chord raises (`App::request_clear_queue`). The `Remove`
and `Clear queue` entries are destructive: they paint in the canvas's
destructive tone under a divider on a GUI-only `destructive` flag that
`RowMenuEntryProjection` and `GuiRowMenuEntry` carry, derived from the
entry's `ContextAction::RemoveFromQueue`, `ContextAction::RemoveSelection`,
or `ContextAction::ClearQueue` action; the shared `ContextMenuEntry` and
every TUI paint site stay untouched. The menu anchors on the row it opened
on (`anchorSlot`).

The menu a multi-row selection opens is not the per-row menu: the gesture
names the marked slots, and `App::build_queue_selection_menu` returns the
entries the TUI's right-click builds from the same `build_selection_menu`
builder, over capabilities intersected across the marked items, with no
raise of the TUI's `pending_overlay`. Its `Remove` names every marked slot
and the core applies it as one Owner `UnifiedQueueRemoveSlots` edit; a
selection that holds the now-playing slot defers to the confirmation modal,
whose answer carries the removal. The menu anchors on the first marked
slot. `GuiQueue.drag` carries the
dragged row's content as the ghost, the final index the drop leaves it at,
and the insertion line's stable-slot anchor; the ghost floats at that line
— with no ordinal, and the dragged row's painted percent — and the dragged
row keeps its in-place paint (the canvas keeps it lit). The core owns the
gesture and the drop index, and it sends
one move request to the Owner on release. `Alt+J` and `Alt+K` ask the core
for the same move on the keyboard-selected row. A `cancel` pointer kind
drops the drag, the row hover, and the scrubber hover without a move.

The shared confirmation modal (`ConfirmOverlay.svelte`) paints window-modal
over every panel on the `--canvas-modal-scrim` backdrop. `ask_confirm` holds
the modal as `GuiOverlay::Confirm` and still raises the shell's own
`OverlayRequest::Confirm`, which the GUI leaves undrained because it mounts
no TUI modal component, so the same `ConfirmModal` the TUI's Confirm
component paints reaches the GUI: the frame carries its message and its
button row, each button with its label, its key hint, and its tone. The
action stays in the core's held state, and both a key and a button press
answer it through the shared `apply_confirm_action`. The modal owns the
keyboard while it is up: Enter accepts, Esc cancels, the three-way
dirty-playlist prompt also takes `d`, and every other chord is consumed with
the modal still up. A press outside the modal answers Esc, so a staged
transfer the prompt guarded unwinds. Each button reports a `confirm` region
and the core executes the button's own key hint, so a press answers what the
window painted; a press whose region outlived the modal executes nothing.

The daemon-lost recovery dialog (`DaemonLostOverlay.svelte`) paints in that
same window-modal language, on the same scrim, whenever the App raised the
TUI's DaemonLost modal: `raise_daemon_lost_modal` holds the shared
`DaemonLostModal` as `GuiOverlay::DaemonLost` beside the TUI's raise, and
`dismiss_daemon_lost` clears it with the shell request. The frame carries
the diagnostics the TUI's modal paints — the last playing title, the daemon
log path, and a failed restart's `Restart failed:` line — and the three
recovery choices from the one table the dialog's chords read,
`Restart and resume` (`r`), `Restart, don't resume` (`s`), and `Quit`
(`q`). The dialog blocks every other input: Enter, Esc, and a press outside
it all answer nothing, because the TUI binds only those three chords and
the dialog must not be dismissed back to a dead Owner. Each button reports
a `daemonLost` region, and a press and a chord run the same
`App::apply_daemon_lost_intent`, so a failed restart leaves the dialog up
with the restart's own diagnostic and the quit choice returns the run's
quit decision to the core.

The footer line paints below the box between gap rows on the
`queueFooterBand` fill. Its left group carries the `PREFIX` badge while
prefix mode is armed, the autosave indicator, and the playlist pill; its
right group carries the dirty indicator and the session marker; and a live
toast paints inline at its centre in the severity colour.

## The Status bar in the footer line

`GuiStatusBar` mirrors the shared `StatusBarProjection`: the prefix-armed
state and the live toast with its severity. The Queue column's canvas has no
separate status bar: `QueueFooter.svelte` paints both facts in the footer
line. The
badge drops while a toast is live — the TUI's pill-drop rule — and the
toast moves no layout, because the line is always painted and the toast
only fills its centre span. The line reports no regions: the playlist pill
paints but is no press target (the slice presents no playlists surface), and
a point anywhere on the line resolves to nothing. The spec permits the GUI to show toasts in a
conventional GUI form that TUI geometry does not constrain, so a toast
presentation outside the line is lawful. While the Library panel shows, the
Library Status bar paints the same projection (design D10): the hints on the
left, the location on the right, and the armed prefix or the live toast in
place of the hints.

## The Library panel

The GUI paints a Library panel in a column beside the Queue panel
(`LibraryPanel.svelte`). It follows the design canvas's Library artboard, not
the TUI: the GUI does not keep parity with the TUI, and the TUI does not change
(ADR 0036). The panel's interaction state lives in the window content, and the
Client core holds the data selection and the actions (design D1).

| Window content (`ui/src/lib/library/`) | Client core (`App`) |
| --- | --- |
| Cursor, hover, scroll, open tree nodes | The active destination (`App::tab`) |
| Focus inside the panel (pill row, list, Hero, tracklist) | The selected collections of each destination |
| Filter text and Search query text | The album each collection last showed in the Hero (`App::gui_library_picks`) |
| The open Hero overlay and the item menu | Service data, its loading, and the artwork |
| The Library's keys while it has Panel focus | Panel focus (`FocusStack`) and the actions |

The data selection is core state because the core decides what to load and
what to save at exit. The window content changes it only by a command, and the
frame carries it back, so the pill row paints the core's set. The panel uses
no TUI component, no `mbv-surfaces` Surface state, no `GuiSurfaceLeaf`, and no
`[keys]` action, and its pointer input never reaches the core's gesture
recogniser or hit map. `LibraryPanel.svelte` paints `LibraryTopBar` (the
Home icon, one tab per Service library destination, and the Search field
with its results dropdown), the destination's pill row (`LibraryPills` in
Music; `LibraryShowsPills`, `LibraryEmbyPills`, `LibraryPodcastsPills`, and
`LibraryFeedsPills` — a destination with no pills paints no row), the
destination's list (`LibraryList` with `LibraryLetterRail` in Music;
`LibraryHomeList`, `LibraryShowsList`, `LibraryEmbyList`,
`LibraryAudiobooksList`, `LibraryPodcastsList`, and `LibraryFeedsList`),
the destination's Hero (`LibraryHero` with `LibraryTracklist` in Music;
`LibraryHomeHero`, `LibraryShowsHero` with its Workspace,
`LibraryEmbyHero`, `LibraryAudiobooksHero`, `LibraryPodcastsHero`, and
`LibraryFeedsHero`), and `LibraryStatusBar`, and the right-click item menu
(`QueueRowMenu` under the panel), which a right-press on a list row or a
Hero tracklist row opens through the `rowMenu` command: the command names
the row's stable Service id, the frame's `rowMenu` section carries the
shared builder's full entries for that item, and the menu paints at the
pointer position. A press outside the menu, a scroll, or Esc closes it, and
a chosen entry sends the same `menuEntry` command the Hero's More actions
sends. The Hero's More actions paints the shared builder's entries minus
the ones its own buttons already send (`Play`, `Play All`, `Add to
Queue`), under each entry's carried builder index, and paints no control at
all when the remainder is empty — the live finding that a Music album's
menu repeated the buttons and offered nothing new.

### The frame section and its revision

`GuiFrame.library` carries a `GuiLibraryFrame`
(`src/bridge/frame/library.rs`). A Library list can hold thousands of rows, so
the section carries a revision and the content rides a frame only when that
revision changes, as the visual slot's artwork does. `revision` changes when
any part of the built content changes, the art included. `content` is
`Option<Box<GuiLibraryContent>>`: `None` while the window already holds the
revision, so an unchanged frame carries only the number. The window content
keeps the last carried content (`carryLibraryContent`,
`ui/src/lib/library/state.ts`), and a window-content reload's `report_layout`
re-arms the section (`LibraryFrames::rearm`,
`crates/mbv-gui/src/library_frames.rs`), so the next frame re-carries the
whole content.

`search` is the Search section (design D12, row 12.1):
`Option<GuiSearchFrame>`, present while the top bar's Search field — the
field the `Ctrl+K` chord arms — holds a query or its rows, and `None`
while it is closed. It rides the section beside the
revision-gated `content`, never inside it, because Search is
cross-destination: a result that lands between two content changes still
reaches the window. `GuiSearchFrame`
(`src/bridge/frame/library/search.rs`) carries `query` (the query the core is
answering), `loading`, `failed`, and `results`, whose rows carry `id` and
`item_type` (the stable Service identity the `searchActivate` command sends
back), `title`, and `subtitle` (empty when the item has none).

`GuiLibraryContent` carries `tabs` (one tab per Service library
destination, plus the feed tab while subscriptions are configured, in
catalog order, each a `GuiLibraryTabFrame` with a `position` and a
`label`; Home is not a tab — the top bar's Home icon opens position 0),
`current_tab` (the active tab's position; `None` for Home, whose icon
marks itself current while `home` is present, and for an active
destination no tab names), `music`
(`Option<GuiMusicFrame>`, present while a Grouped Music library is active),
`home` (`Option<GuiHomeFrame>`, present while Home is the active
destination — add-gui-library-panel design D12, the USER-APPROVED GUI Home
design), and `shows` (`Option<GuiShowsFrame>`, present while an Emby TV
library's Shows destination is active — design D12, row 7.1), and `emby`
(`Option<GuiEmbyFrame>`, present while a plain Emby library's destination is
active — design D12, row 8.1), and `audiobooks` (`Option<GuiAudiobooksFrame>`,
present while an Audiobookshelf book library's Audiobooks destination is
active — design D12, row 9.1), `podcasts` (`Option<GuiPodcastsFrame>`,
present while an Audiobookshelf podcast library's Podcasts destination is
active — design D12, row 10.1), and `feeds` (`Option<GuiFeedsFrame>`, present
while the Feeds destination is active — design D12, row 11.1). No two
destination fields present at once: the destinations are exclusive, so the
present field names the active destination, and the top bar marks the Home
icon current exactly while `home` is present.

`GuiHomeFrame` carries `rows`, `selected`, and `hero`: Home's flat list in
the Music list slot, with no pill, no letter rail, and no Workspace. `rows`
are the two sections' heading rows and item rows in paint order —
`Continue watching` first, then `Recently added` — with a heading only above
its rows, an empty section omitted, and the spacer only between two
non-empty sections, the TUI's flat list's rules. An item row carries:
`id` (`GuiHomeRowId`), the stable identity `App::home_stable_target`
resolves — a `Continue` tag with the row's Emby item id, or a `Recent` tag
with the queue item's content id (`provider`/`value`, never a position);
`title` and `subtitle`, the split title parts the TUI's flat list paints;
`date`, the gutter date as the shared projection's `day month` text
(`fmt_publish_date_short` over the provider timestamp; `None` on a Continue
row and on an undated item); `kind` (`collection` when the row targets a
container the play path refuses, `media` on a playable leaf, mirroring the
shared flat-list vocabulary); `state` (`ordinary`, `played`, or
`{ active: { progress } }` with the Continue Watching resume percent);
and `art` (`GuiLibraryArt | null`, the row's card art for Emby items only —
the Audiobookshelf and Feed rows paint without art in Phase 2, and the
front end paints its own placeholder while `null`). `selected` carries the
core's selected row the select command last named — the first item row is
the Hero default, so the Hero paints as soon as a section lands; a stale
frame's selection resolves on its own rows or falls back without a hero.
`hero` carries `title`, `meta_rows` with `duration_row` and `progress_row`,
`overview`, `art_shape` (`landscape`, `square`, or `portrait`, the artwork
policy's declared arm), and `art` (the artwork policy chain's display
encode for Emby items only, again `null` on the rows without art) — the
shared hero producers' facts, the same facts the TUI paints for the same
item.

`GuiShowsFrame` (`src/bridge/frame/library/shows.rs`) carries `lib_idx` (the
Shows library index the Shows commands name), `pills`, `collection_names`
(the active mode's name, which the Library Status bar's location reads),
`loading`, `workspace_loading`, `series`, `upcoming_failed`, `rows`,
`selected`, and `hero` (design D14). The pill row is single-select and holds
the TUI's three modes in order — `latest`, `upcoming`, and `all`, labelled
New, Upcoming, and All — each with `mode`, `name`, `selected`, and
`collection` (`false`: the TUI's letter-range buckets are not pills here,
because the All mode's letter rail replaces them).

The All mode's list is the Music tree over the TUI's TV node kinds, and the
frame carries it as `series`: each node holds `id`, `title`, `sort_letter`,
`expandable`, and its `seasons`; each season holds `id`, `name`, and
`episodes`, which are `null` until that season's load lands, so an open
season asks the core for them. The window content owns the open and closed
node keys, exactly as the Music tree's does, and builds the painted rows at
the Music heights: series and seasons on the node row, episodes on the track
row, the letter heading above the first series of each letter, and the
`Series` header with the mode as the selection label and the `/` filter
field. A series whose detail has not landed paints closed and expandable.
`rows` carries the flat Latest and Upcoming lists only, each row with `id`,
`title`, `subtitle`, `date`, and `sort_letter`: Latest holds the newest
episodes, with the show's name as `title`, the episode's own name as
`subtitle`, and the provider date in `date`; Upcoming holds the snapshot's
premieres, with the series' name as `title`, the season/episode label and the
episode's name as `subtitle`, and the premiere date in `date`. Both flat
lists are date-sorted and carry no `sort_letter`; `upcoming_failed` carries
the eager fetch's final failure, which the list names with the pill as the
retry. `selected` carries the core's selected row as the frame painted it —
the series id in All, the episode id or the premiere's stable key in the flat
modes — with the first node as the default, so the Hero paints as soon as a
list lands, and a stored id the list no longer carries falls back to that
first node. The All mode's `hero` is present exactly while a series is
selected; `ensure_gui_shows_workspace` loads the selected series' detail and
its selected season's episodes on the frame path, so the Workspace paints
without a season press.

`hero` is the selected row's Hero and is absent in Upcoming, which paints
none. In All series it carries the TUI series producer's facts —
`hero_content_series_with_episode`'s genre, year range (else the release
date), season count, and network rows, no duration row while the Workspace
has no focus, and the artwork policy's landscape arm — plus `workspace`: the
detail's season pills (`season_id`, `name`, `selected`), `selected_season`
(the stored choice while the detail still holds it, else the first season,
because the pills start on the first season and are never kept across a
series change), and that season's `episodes`. The Workspace-focus overlay
that swaps the facts for the selected episode's is recorded in D14 as a
follow-up and is not carried. A Workspace episode row carries `episode_id`,
`number`, `name`, `duration`, and `state` (the shared row state the Home rows
carry). In Latest the Hero is the selected episode's facts from
`hero_content_emby` with no `workspace`. The landscape arm paints the still
at the Hero content width at 16:9 above the facts, as the TUI does; the
portrait and square arms keep the canvas's 300 px box beside the text. A
click or a double-click on a Workspace episode sends that episode's
`episode_id` with `showsPlay`, and a right-press on a tree row opens the item
menu at that row's stable Service id.

`GuiEmbyFrame` (`src/bridge/frame/library/emby.rs`) carries `lib_idx` (the
plain Emby-library index the Emby commands name), `pills`,
`collection_names` (the selected collection's name the Library Status bar's
location reads; empty in plain mode, whose list is the whole library),
`feed_group`, `loading`, `failed`, `rows`, `selected`, and `hero`. A plain
Emby library (row 8.1: every Emby library that is neither a Grouped Music
library nor a TV library) paints the plain mode: `feed_group` is `false`,
the rows are the root-list store's full list in the server's `SortName`
order, each with `sort_letter` over the article-stripped sort name (`#`
past Z), the year or the runtime on `subtitle`, and no `date` — the letter
rail replaces the TUI's letter-range pill row, so no pill rides. A library
in the feed-group mode (`feed_view_libraries`) paints `feed_group` `true`
with a single-select pill row of All and each root folder whose video count
is positive, in listing order, each pill with `group` (`{ kind: 'all' }` or
`{ kind: 'folder', id }`), `name`, `selected`, and `collection` (`false`,
design D6). Its rows are the selected group's items in the server's date
order — All newest first, a folder oldest first — with the production year
in `date` and no `sort_letter`. In both modes `selected` carries the core's
selected row as the frame painted it, with the first row as the default, so
the Hero paints as soon as a list lands, and a stored id the list no longer
carries falls back to that first row; a plain mode's fetch that failed
carries `failed` and shows no row until the library opens again. `hero` is
the selected row's shared Emby hero producer's facts — `title`, `meta_rows`
with `duration_row` and `progress_row`, `overview`, `art_shape` at the
artwork policy's declared arm, and `art` — and carries no Workspace.

`GuiAudiobooksFrame` (`src/bridge/frame/library/audiobooks.rs`) carries
`lib_idx` (the Audiobooks library index the Audiobooks commands name),
`loading`, `failed` (the browse state's catalog states),
`workspace_loading` (the selected book's detail fetch), `rows`, `selected`,
and `hero`. No pill rides here: the A-Z letter rail replaces the TUI's eight
surname buckets, as it does for Shows and the plain Emby libraries. The rows
are the catalog's books in the TUI's order — author sort key, then title —
each with `id` (the book's stable `library_item_id`), `title`, `subtitle`
(the author display credit), `duration` (the book's total duration, the TUI
row's duration gutter), and `sort_letter` (the first letter of the author
sort key, `#` for a digit- or symbol-led surname). `selected` carries the
core's selected book as the frame painted it, with the catalog's first book
as the default, so the Hero paints as soon as the catalog lands, and a stale
pick falls back to that first book.

`hero` is the selected book's Hero: the shared `hero_content_abs_book`
producer's facts — `title`, `meta_rows` (the author, the duration, and the
`Finished` or `N%` progress row) with `duration_row` and `progress_row`, and
no narrator, series, or description — `art_shape` at the book artwork
policy's portrait arm, `art` (the cover through the Service-scoped book-cover
chain, `null` while the book declares no cover or no server is configured),
and `workspace`: the book's chapter rows in the TUI's `visible_rows` order.
A chapter row carries `chapter` (the stable Service discriminator the TUI's
chapter activation addresses — the chapter number, or the audio-part index
on a `Part N` row, never a display position), `title`, `duration`, and
`current` — the current-chapter mark while that book plays, derived exactly
as the TUI's `WorkspaceNowPlaying::Chapter` projection derives it; a frame
re-carries the content when the mark moves, so the Workspace paints what the
Queue's active slot paints. An audio-part row never carries the mark, as the
TUI's rows never light.

`GuiPodcastsFrame` (`src/bridge/frame/library/podcasts.rs`) carries `lib_idx`
(the Podcasts library index the Podcasts commands name), `pills`, `loading`
(the show catalog's pages or the active view's own episode fetches),
`failed` (the catalog's error), `rows`, `selected`, and `hero`. No letter rail
rides here: the list is sorted by date. `pills` holds the single-select pill
row in paint order — `Latest` first, then one pill per show in the show
list's title order — each with `pill` (`{ kind: 'latest' }` or
`{ kind: 'show', id }`), `name` (`Latest`, or the show's title), `selected`,
and `collection` (`false`). The rows are the active pill's episodes: `Latest`
merges every listed show's episodes, and a show pill holds that one show's,
both newest first with undated episodes last, exactly as the TUI's
`visible_episodes` and `visible_episodes_from` order them. A row carries `id`
— the episode's stable identity, the provider's `(library_item_id,
episode_id)` pair encoded as one opaque string
(`GuiPodcastEpisodeId::encode`: the show id's byte length, a `:`, then the
show id and the episode id; `decode` is its only reader) — `title` (the
episode), `subtitle` (the parent show's title), `date` (the `day month`
gutter, `null` on an undated episode), and `state` (`ordinary`, `played`, or
`{ active: { progress } }`, the TUI podcast row's own arms). `selected`
carries the core's picked episode as the frame painted it while the active
list still holds it, else that list's first row, so the Hero paints as soon
as a list lands; a stored pill whose show the published show list no longer
carries falls back to `Latest`.

`hero` is the selected episode's Hero: the shared `hero_content_abs_episode`
producer's facts — `title` (the episode), `meta_rows` (the show title, the
duration, and the publish date) with `duration_row` and `progress_row` (the
producer fills no progress row for an episode) — the cleaned description as
`overview`, `art_shape` at the episode artwork policy's Square arm, and `art`:
the parent show's cover through the Service-scoped cover chain (`null` while
the show declares no cover or no server is configured). There is no
Workspace: a podcast Hero is episode information only.

`GuiFeedsFrame` (`src/bridge/frame/library/feeds.rs`) carries `pills`,
`loading`, `failed`, `rows`, `selected`, and `hero`. It carries no `libIdx`: a
Client holds one feed tab, so the Feeds commands name no library. No letter
rail rides here — the list is sorted by date — and no state filter.
`failed` is always `false`, because the feed tab keeps no failure state: a
failed subscription's fetch flashes the TUI's toast and keeps that
subscription's previous entries. `pills` holds the single-select pill row in
paint order — `Latest` first, then one pill per configured subscription in the
tab's order — each with `pill` (`{ kind: 'latest' }` or
`{ kind: 'subscription', id }`, the id being the subscription's URL),
`name` (`Latest`, or the subscription's name), `selected`, and `collection`
(`false`). The rows are the active pill's entries: `Latest` merges every
subscription's entries, and a subscription pill holds that one
subscription's, both newest first with undated entries last, exactly as
`FeedTabState::rebuild_all_entries` orders them. A row carries `id` — the
entry's stable identity, its `(subscription url, guid)` pair encoded as one
opaque string (`GuiFeedEntryId::encode`: the URL's byte length, a `:`, then
the URL and the `guid`; `decode` is its only reader), because two
subscriptions can publish the same `guid` — `title` (the entry), `subtitle`
(the subscription's name, `null` when the tab no longer holds it), `date`
(the `day month` gutter, `null` on an undated entry), and `state`
(`ordinary`, `played`, or `{ active: { progress } }`, the TUI feed row's own
arms). `selected` carries the core's picked entry as the frame painted it
while the active list still holds it, else that list's first row, so the Hero
paints as soon as a list lands; a stored pill whose subscription the tab no
longer holds falls back to `Latest`.

`hero` is the selected entry's Hero: the shared `hero_content_feed`
producer's facts — `title` (the entry) and `meta_rows` (the subscription
name, then the duration when the entry declares one) with `duration_row` and
`progress_row` (the producer fills no progress row for an entry) — and
`art_shape` at the feed artwork policy's arm: Square for a podcast feed
(`FeedKind::Audio`), Landscape otherwise. No `overview`, no `art`, and no
Workspace ride here: a feed entry has no artwork source and no description,
so the Hero paints its placeholder in the reserved shape.

`GuiMusicFrame` carries `lib_idx` (the Grouped Music library index the
set-collections command names), `pills` (`group_id`, `name`, `selected`, and
`collection`), `loading`, `failure` (the failure line naming the selected
groups whose album list failed), `artists` (`artist_key`, `name`,
`sort_letter`, and `albums`), `hero`, `art`, and `playing_track_id` (the
playhead's Emby track id, from the Queue panel's own authority, which the
tracklist's playing row paints with a play glyph beside the title, as the
Queue's now-playing row does; the library tree shows no playing marker). An album carries `album_id`,
`name`, `year`, and its `tracks` and `total` when the cache holds them; a
track carries `track_id`, `name`, `number`, and `duration`. `GuiMusicHeroFrame`
carries `artist`, `collections`, `album_count`, `artist_tracks`, the picked
`album`, and its `menu` entries with `label`, `key_hint`, and `index` (the
entry's index in the shared builder's order, so a front end that paints only
some entries still executes the one it painted). `GuiMusicFrame` also
carries `row_menu` (`Option`): the item of the `rowMenu` command with the
TUI menu builder's full entries for it, which the right-click item menu
paints. `GuiLibraryArt`
is the cached artwork's display encode or the bundled placeholder as a
`data:` URL, or `Loading`. The window fetches no Library image: every byte is
the core's image cache.

The core warms that cache around the pick. Each destination's projection ends
with `App::warm_gui_hero_neighbours`
(`src/app/gui_library/artwork.rs`): it fetches the Hero art of the entries
neighbouring the picked one in the painted order — one behind and three ahead,
never the picked entry, whose art the Hero's own fetch covers — on the sources
the same producers the Hero reads declare (`music_album_source`, the Emby
policy, the Shows `Primary` poster). The window keeps the TUI's navigation-idle
gate, so fast arrow traffic warms nothing, and the three select commands
(`showDetail`, `embySelect`, `showsSelect`) stamp navigation on a real pick
move. The Music window is the TUI's own rule
(`openspec/specs/music-library-hero/spec.md`). An Audiobookshelf or Podcasts
Hero keeps its Service-scoped cover key and is not warmed.

### The `library` command

`library(GuiLibraryCommand)` is the fourth bridge command (`src/main.rs`). It
only pushes `CoreInput::Library` into the core channel, as the other commands
do. `src/bridge/library.rs` translates the wire variants and
`App::apply_gui_library_command` (`src/app/gui_library.rs`) applies each one
through the existing `App` action, so the TUI's own paths do not change. The
variants are `openDestination` (`position`), `selectCollection` (`libIdx`,
`groupId`), `showDetail` (`itemId`), `openAlbum` (`albumId`), `takeFocus`,
`play` (`itemId`), `playNext` (`itemId`), `addToQueue` (`itemId`),
`shuffleArtist` (`albumIds`), `menuEntry` (`itemId`, `index`), `rowMenu`
(`itemId`), and the Home commands — `homeSelect`, `homePlay`,
`homePlayNext`, and `homeQueue` (each carrying `row`, the
`GuiHomeRowId` the frame painted). Item
identities are stable Service ids, never row positions, so a command built
from a stale frame addresses the item that frame painted or nothing. The
Home row id, not a Music item id, is
what `App::home_stable_target` resolves: `homePlay` runs the Home flat
list's play path (`App::home_play_target`, whose resume behaviour the row's
Continue Watching membership decides), and `homePlayNext` and `homeQueue`
both run its enqueue path (`App::home_enqueue_target`) — the Owner owns the
queue, so a Home command never mutates it directly, and Home's rows carry
no library the Music play-next admission resolves against. A stale frame's
row, one whose item the App's Home data no longer holds, logs and changes
nothing, as every item-named variant's stale arm does.

The Shows commands name a Shows library by its `libIdx` and its content by
the stable Service id the frame painted: `showsSetMode` (`libIdx`, `mode`)
loads the chosen mode through the TUI's own mode load, `showsSelect`
(`libIdx`, `row`) selects the row the Hero shows — the All series mode's
series, which also starts its detail load, or a flat mode's episode or
premiere — `showsSelectSeason` (`libIdx`, `seasonId`) moves the Workspace to
that season's episodes, and `showsPlay`, `showsPlayNext`, and `showsQueue`
(`libIdx`, `itemId`) run the shared library play, play-next, and enqueue
paths on an episode. The Owner owns the queue, so no Shows command mutates
it directly, and a row, season, or episode id the active list no longer holds
logs and changes nothing.

The plain Emby-library commands name their library by its `libIdx` and its
content by the stable Service id the frame painted (row 8.1):
`embySelect` (`libIdx`, `row`) selects the row the Hero shows, and
`embySetFeedGroup` (`libIdx`, `group`) runs the TUI's own feed-group
selection path — the session cache, the stale-drop, and the lazy fetch
behave as the TUI pill's press does — with a group the library does not
present, or a library that presents no feed-group mode, logging and changing
nothing. `embyPlay` (`libIdx`, `itemId`) — the command Enter and a
double-click both send — runs the shared library play path, whose
folder-play rule expands a folder or a boxset into its playable content, so
the queue is replaced with the folder's items and no nested list opens;
`embyPlayNext` and `embyQueue` (`libIdx`, `itemId`) run the shared play-next
and add paths. The Owner owns the queue, so no Emby command mutates it
directly, and a row or folder id the active list no longer holds logs and
changes nothing.

The Audiobooks commands name their library by its `libIdx` and the book by
its stable `library_item_id` (row 9.1): `absSelect` (`libIdx`, `row`)
selects the book the Hero shows, whose select runs the TUI's own detail
loader; `absPlay` (`libIdx`, `itemId`) — the command Enter and a double-click
both send — replaces the queue with the book at its saved position, through
the TUI's book play path; `absPlayChapter` (`libIdx`, `bookId`, `chapter`)
runs the TUI's `ActivateChapter` rule on a Workspace chapter row — one
absolute seek on the active book's merged timeline, or a replace-and-start at
the chapter's offset on another book; and `absQueue` (`libIdx`, `itemId`)
appends the book at the tail. There is no play-next command: `n` is consumed
by the front end and does nothing for books. Every play and enqueue honours
`player.can_admit_audiobookshelf()`, and a refusal flashes the TUI's
owner-unavailable toast; the Owner owns the queue, so no Audiobooks command
mutates it directly, and a book or chapter id the Service data no longer
holds logs and changes nothing.

The Podcasts commands name their library by its `libIdx` and an episode by the
opaque row id the frame carried (row 10.1): `podcastsSetPill` (`libIdx`,
`pill`) stores the single-select pill — `{ kind: 'latest' }` or
`{ kind: 'show', id }` — and starts the loads that pill needs through the
TUI's own loaders, with a show the published show list no longer carries
logging and changing nothing; `podcastsSelect` (`libIdx`, `row`) selects the
episode the Hero shows; `podcastsPlay` (`libIdx`, `itemId`) — the command
Enter and a double-click both send — replaces the queue with the episode
through the TUI's episode play path; and `podcastsQueue` (`libIdx`, `itemId`)
appends it at the tail. There is no play-next command: `n` is consumed by the
front end and does nothing for podcast episodes. Every play and enqueue
honours `player.can_admit_audiobookshelf()`, and a refusal flashes the TUI's
owner-unavailable toast; the Owner owns the queue, so no Podcasts command
mutates it directly. A row id of no valid shape, or an episode the per-show
cache no longer holds, logs and changes nothing.

The Feeds commands name an entry by the opaque row id the frame carried and
carry no library index (row 11.1): `feedsSetPill` (`pill`) stores the
single-select pill — `{ kind: 'latest' }` or `{ kind: 'subscription', id }` —
and starts no load, because the shared startup fetch already fills every
subscription's entries; a subscription the tab no longer holds logs and
changes nothing. `feedsSelect` (`row`) selects the entry the Hero shows.
`feedsPlay` (`itemId`) — the command Enter and a double-click both send —
replaces the queue through the TUI's `play_feed_entry`, and `feedsQueue`
(`itemId`) appends through `enqueue_feed_entry`; both flash the TUI's
"Feed entry has no playable source" toast for an entry with no enclosure and
no link. There is no play-next command: `n` is consumed by the front end and
does nothing for feed entries, because the TUI has no feed play-next producer.
The Owner owns the queue, so no Feeds command mutates it directly. A row id of
no valid shape, or an entry the App's data no longer holds, logs and changes
nothing.

The Search commands carry no library index, because Search is
cross-destination (row 12.1), and their arms are the `gui_search_query`,
`gui_search_activate`, and `gui_search_close` methods over the one
`GuiSearchStore` (`src/app/gui_library/search.rs`). The field sends
`searchQuery` (`query`) debounced while the user types: it stores the live
query and starts the TUI's own search worker through
`App::spawn_search_sidebar_query`, and an empty or blank query clears the
results at once.
`searchActivate` (`id`, `itemType`) — the command Enter and a press both send
— navigates to the row's item through the TUI's own navigation and closes the
results. `searchClose` clears them. No overlay request rides these commands,
because the GUI fronts its own results. The GUI mounts no search component,
so the core thread's tick host runs `App::drain_gui_search_results` in place
of its no-op, and the frame carries the store that drain fills; the TUI's own
drain and its component do not change. A row the store no longer holds logs
and changes nothing.

The `takeFocus` variant also moves the core's `FocusStack`, so the frame paints
the focus the `App`'s key gates read (design D4). The `play` and
`addToQueue` variants apply the library playback policy's autoload rule to
the Library's routes: with autoload on, a track whose album the track cache
holds replaces the queue with that album's cached tracks starting at the
selected track, or queues them from the selected track onward; with autoload
off, the play and the add act on the track alone. The `play` and
`shuffleArtist` variants replace the queue directly (the shared folder and
album-expansion paths run with the confirm off), so the Library's Play and
its explicit Shuffle artist never raise the TUI's populated-queue
replace prompt. The
saved-playlist save/discard prompt still presents, through the confirmation
modal. The `rowMenu` variant arms the frame's `rowMenu` section (the
right-click item menu the next paragraph describes), and each carried menu
entry names its index in the shared builder's order, so a chosen entry
executes the entry the frame painted, resolved by the item's id and not by
the TUI's library cursor.

### The key routing order

`input.ts` forwards a keydown only after the window's key router, and it
never interprets the key itself. The router answers one of three outcomes
(`KeyOutcome`): `core` sends the key to the `key` command with its default
action prevented, `handled` means the window content took the key so the core
receives none, and `text` means an open text field takes it, so the browser's
own default types it. `App.svelte` installs `routeLibraryKey`
(`ui/src/lib/library/keys.ts`) as that router, so the Library's handler runs
before the forwarder (design D3).

While the frame carries `settings`, `routeLibraryKey` hands the key to
`routeSettingsKey` (`ui/src/lib/settings/keys.ts`) after the overlay, prefix,
and Queue-filter steps, and the Library steps do not run
(add-gui-settings-screen design D11). The Settings order is:

1. An active capture takes every key: Esc cancels it, and any other key
   sends `keysCapture` with the capture's target and the raw `DomKey`. A
   sent capture keeps swallowing keys until the next frame answers it.
2. A focused text field takes the key: Esc cancels the form, Enter submits
   it, and every other key is the field's typing — the `text` answer lets
   the browser's own default run, so Tab walks the form's fields.
3. The screen keys run: Up, Down, Page Up, Page Down, Home, and End move
   the cursor in the focused pane; Tab and Shift+Tab switch between the
   sidebar and the content; Enter opens the sidebar entry or activates
   the content row; Space activates a focused toggle or choice mark; Esc
   walks the spec's ladder — an open edit, capture, or form cancels first,
   then the content hands the keyboard back to the sidebar, then the
   sidebar's Esc sends `close`.
4. Every other key goes to the core's keyboard router, so playback and
   global chords keep working behind the screen: Space on the sidebar
   plays or pauses.

With no Settings frame, the Library chain runs. The order is:

1. While a core overlay is open (the row menu, the track popover, the
   playback-target chooser, the confirmation modal, the daemon-lost recovery
   dialog) or prefix mode is armed, the key goes to the core.
   Both facts are in the frame. The Queue filter's text entry is the core's
   too, so every key is the core's while it holds the keyboard.
2. The `Ctrl+K` chord arms the Search field: wherever the Library panel
   shows and its top bar mounts the field, the field takes the keyboard;
   in Mini with Queue focus no top bar paints the field, so the chord
   keeps its core effect there.
3. While a text field of the panel holds the keyboard, the field takes the
   key: the Search field first — its hold is DOM focus, Esc closes its
   results, the arrow keys move the highlight, and Enter activates the
   marked one — then the `/` filter, whose Esc closes it and Enter
   accepts it.
4. While the Library panel shows, the window keys run: Tab and Shift+Tab move
   the pane focus (the pill row, the list, the Hero, and across the panel
   boundary to the Queue panel), and `1` to `7` open the first to seventh
   destination tab after Home.
5. While the Library panel has Panel focus, the Library keys run in the window
   content: `/` opens the list filter, Enter plays the item under the cursor
   or presses the focused control, `n` plays it next, `a` adds it to the
   queue, Space and Alt+Space toggle and solo a focused pill, the arrow, Page
   Up, Page Down, Home, and End keys move the cursor, Right and Left open or
   close a tree node or walk the pill row, and Esc closes the open filter,
   overlay, or menu.
6. Every other key goes to the core's keyboard router. So Space plays or
   pauses while no pill has focus, `[` and `]` seek, and the GUI-only chords
   `z`, `q`, and `c` and every `[keys]` binding keep their effect.

With Queue Panel focus the Library takes no key, so `/` opens the Queue filter
and `a` opens the audio popover as before (design D3).

### The arrangement

`libraryArrangement` (`ui/src/lib/bridge/arrangement.ts`) picks the
arrangement from the measured Library column width — the window width minus
the Queue panel's current painted width, 400 px expanded, 72 px in the rail,
or the width a drag gave it — with the canvas sizes as thresholds. Below
480 px is Mini, from 880 px with at least 600 px of height is Wide, and
between them is Narrow. The thresholds are front-end constants beside the
Queue panel's state constants, and the core does not classify the size. Mini
mounts only the panel that holds Panel focus. Narrow mounts both panels, and
a picked item opens the Hero as an overlay over the list. Wide mounts both
panels with the Hero beside the list. The Queue panel keeps its four states
inside every arrangement, and a resize across a threshold moves no cursor,
pill selection, or picked item, because those live in the window content and
the core.

Both columns are resizable (`ui/src/lib/bridge/resize.ts`). A drag on either
divider (`PanelDivider.svelte`) reports the sized column's new width in
pixels — the Queue panel's column, or the Wide arrangement's list column —
and the window content keeps it for the session. A width survives tab and
breakpoint changes because the store is module state, and every drag is saved
to the webview's local storage (`mbv.gui.panelWidths`), so the next launch
reads the widths back; a corrupt value falls back to the defaults. The list
column defaults to 368 px, the canvas's 320 px index 15 % wider. The Queue
column holds between its 400 px artboard and the width that leaves the
Library column its 480 px Mini threshold, and the list column holds between
320 px and 640 px, short enough to leave the Hero pane its minimum.
`panelWidths` resolves the two painted widths and the Library column they
leave — a divider's 6 px included — and the arrangement classifies that same
value, so a drag paints the geometry the thresholds measure and the two
never disagree. A double-click returns a divider to its default. The
dividers are the window content's: they report no region to the core, which
needs none, because the Queue panel's own resize observer re-reports the
regions a drag moved.

While the Settings screen is open it replaces the two columns
(add-gui-settings-screen design D11): the sidebar sits in the Queue column's
place at the column's expanded width — the dragged or the default width,
whatever the collapse toggle, because the 72 px rail cannot hold the section
list — and paints no divider; the shown section's content takes the rest of
the window, and the status line closes the screen with the frame's `status`
facts (the prefix-armed state and the live toast). Below the Mini threshold
— `libraryArrangement` measured on the window width minus the sidebar's
column — only the pane that holds the keyboard shows: the sidebar until a
section opens, then its content. A resize across the threshold keeps the
open section, which the core owns.

Panel focus stays in the core's `FocusStack`, and the frame's `GuiFocus`
carries it. `CoreThread::new` mounts `ComponentId::Library`; a press inside the
Library panel sends `takeFocus`, and the core sets the stack and
`App::panel_focus` together. The GUI's `App` applies neither the TUI's
terminal-width Mini check nor its hidden empty queue column, so the core's key
gates see the stored Panel focus.

### The launch location

The GUI keeps its own launch location (design D9). Its `App` starts with
`LaunchRestore::Done`, so the TUI launch state never moves the GUI's tab or
Panel focus, and the GUI never writes the TUI file. The GUI file
(`mbv_config::load_gui_launch_state` and `save_gui_launch_state`) holds the
destination, the selected collections of each destination with the album each
collection last showed, and Panel focus as stable Service identities. The core reads it once before the first step, saves
it at orderly exit before `client_exit_teardown`, and never while the window
is open.

The restore runs in two phases (`GuiLaunchRestore`,
`CoreThread::restore_launch_location`). The first phase opens the saved
destination once a presented one holds its identity — no destination takes
its place: one the presented set does not hold opens nothing, and the
phase keeps waiting while its parts can land, the Audiobookshelf catalog
and the feed subscriptions — and applies the saved Panel focus over the
Library focus the open takes, unless the user already took focus while the
restore was pending. A start with no saved location — or one that fails
to load — restores through the same path (`mbv_config::first_launch`):
its Home destination resolves to the first presented one, with Library
Panel focus and no saved collection choice, so the Library panel is
reachable from the first launch. The
second phase applies the saved
collection choice once the destination's group list lands: the first saved
group the library still holds is selected, and a saved group the library
no longer held, or no saved entry, leaves the first group selected. The
restore applies to a destination that holds no choice and never clobbers one
the session already made. A mid-restore tab switch abandons the saved sets rather than landing
them under another destination. The restore is idempotent, so the catalog and
library drains call it whenever their content can complete a phase. The saved
picks restore with the group list, and a group with no pick heroes its first
album (design D13).

## The Settings screen

`settings(GuiSettingsCommand)` is the fifth bridge command (`src/main.rs`). It
only pushes `CoreInput::Settings` into the core channel, as the other commands
do. `src/bridge/settings.rs` translates the wire variants and
`App::apply_gui_settings_command` (`src/app/gui_settings/command.rs`) applies
each one through the existing `App` method, so the TUI's own paths do not
change. The variants are `close`, `selectSection` (`section`), `activate`
(`setting`, the `SettingKey` name), `applyChoices` (`kind`, `selected`),
`assignRoute` (`library`, `endpoint` or none), `embySetup` (`serverUrl`,
`username`, `password`), `audiobookshelfSetup` (`serverUrl`, `apiKey`),
`serviceAction` (`action`), `cancelSetup` (`service`), `feedAdd` (`name`,
`url`, `kind`), `feedEdit` (`url`, `name`, `kind`), `feedRemove` (`url`),
`keysCapture` (`target`, `key` as a `DomKey`), `keysReset` (`action`),
`keysClear` (`target`), `selectTarget` (`target`), `playlistOpen`,
`playlistBack`, and `playlistRefresh` (`id` or none), `playlistAction` and
`playlistItemAction` (`id`, the item `id`, and `play`, `shuffle`, or
`enqueue`), `playlistRename` (`id`, `name`), `playlistDelete` (`id`), and
`resetUiState`. Identities are stable — a `SettingKey` name, a playlist id, a
feed URL, a chord — never row positions, so a command built from a stale frame
addresses what that frame painted or nothing, and a `setting` name this build
declares no key for logs and changes nothing. `src/bridge/keys.rs` translates a
`keysCapture` press to the neutral `KeyEvent` the router normalizes, so the
window never forms a chord string and the two Clients cannot disagree on one
chord's spelling.

The Client core owns whether the screen is open and which section it shows
(`App::gui_settings`, `src/app/gui_settings.rs`); the window content owns its
interaction state — the sidebar cursor, the keyboard pane, each list's cursor
and scroll, the form drafts, the capture, and the Mini pane (ADR 0037). The
frame's section is `GuiSettingsFrame`: the shown `section`, the `sections` the
sidebar lists, and `content`, a tagged union with `rows` (an F2 page's rows),
`services`, `feeds`, `keys`, `sessions`, and `playlists`. Each F2 row carries
its stable setting name, its label, its kind (toggle, cycle, choice list, or
action), its value text, its toggle flag, and its choices once the section's
load lands. The Services content carries each Service's state and the actions
the TUI offers for it, with the open setup form's progress and error; the Keys
content the prefix chord, one row per `AllClients` action, and the last
refusal; the Sessions content the same playback-target choices the header's
chooser lists; and the Playlists content the saved list, the open playlist, and
the loading flags. No variant carries a password, an API key, or any other
Service credential. `GuiFrame.settings` is `None` while the screen is closed,
so the window mounts the screen only for a frame that carries it.

The sidebar requests reach the GUI through one per-tick drain
(`CoreThread::drain_pending_overlay`, `src/core_thread/settings.rs`).
`ClientTickHost` gets no new hook: after input routing the core takes
`App::pending_overlay` once and matches it exhaustively — one documented arm
per variant, no wildcard. `ToggleSidebar(Settings)` toggles the F2 group,
`ToggleSidebar(Sessions)` and `ToggleSidebar(Playlists)` toggle their own
sections, and the toggle rule is the spec's: closed opens at the action's
section, the action's own group closes, and any other section switches.
`OpenSidebar(Settings)` opens the screen at the section `settings_destination`
names, so the first-start open reaches the GUI with no new code in `construct`,
and `DismissSidebar(Playlists)` closes the screen while it shows Playlists,
which is what the playlist play's `ReplacementExecutor` request does. Every
other variant is a TUI overlay the GUI does not present: it logs
`gui.overlay_request.undelivered` at debug level and drops. The drain also
clears a stale request, which `run_replacement` reads through
`pending_overlay.is_none()`.

Opening the screen pushes `ComponentId::Overlay(OverlayId::Settings)` onto the
core `FocusStack`, and closing pops it. The GUI keeps no `GuiSurfaceLeaf` for
that id, so a key routes with no leaf: global and transport actions fire, while
a queue-scoped gate such as `ClearQueuePrompt` reads a focus that is not the
Queue and stays closed. Settings is not added to `is_blocking_overlay`, so the
two Clients' overlay rules stay the TUI's.

The frame carries the UI-reset counter `uiResetEpoch` (design D10): the window
resets its Library interaction stores and its panel widths when the value
changes, and the TUI's own reset does not move it.

The window-content half lives in `ui/src/lib/settings/` (design D11):
`state.ts` holds the screen store — the keyboard pane, the sidebar and
content cursors, the Mini pane (which is the keyboard pane), the form
drafts, the choice marks, and the capture — and `entries.ts` builds each
section's flat list of activatable controls, which `keys.ts` routes the
screen keys over. `ui/src/lib/components/Settings*.svelte` holds the screen,
the sidebar, one component per section kind, and the status line, and
`App.svelte` mounts `SettingsScreen` in place of the two columns while
`frame.settings` is present. Commands go through `sendSettingsCommand`
(`ui/src/lib/bridge/settings.ts`) to the `settings` bridge command.

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

After the attach both Clients run the same Service startup (design D11).
`App::start_client_services` is the shared step: the Emby startup worker, the
Audiobookshelf startup worker, `start_feed_fetch`, and capability
registration. `Model::spawn_startup_services` calls it in place of its own
body at the same point, so the TUI's order and timing do not change, and
`CoreThread::new` calls it in place of the deleted minimal Emby client fill.
`App::load_home_at_startup` is the `App` half of the launch Home load: it
fetches Home, arms the `Recently added` fetch, and clears a stale status row,
or flashes the "Couldn't load home" toast. Both Clients call it right after
`start_client_services`; the `Model` keeps its own half (`home_content.loading`
and the content assignment), and the GUI drops the returned content until Home
is presented.

The GUI keeps its playback-target discovery, which the full Emby startup does
not start. `App::start_playback_target_discovery` starts the `/Sessions` fetch
and the cast receiver browse, and the core thread runs it when the attach
brought a cached Emby client and in its startup drain host when the drain
reports a successful Emby startup. `Model::init_idle_feed` stays the TUI's.

## Adding a surface to the GUI

A surface joins the GUI slice in this order. A surface the GUI does not yet
present stays absent — no stub, no placeholder (the `gui-client` spec).

The GUI does not keep parity with the TUI; the design canvas is the
reference. These steps are for a surface that both Clients share, such as
the Queue panel. A GUI-only surface owns its interaction state in the
front end, and the Client core supplies only its data and actions: the GUI
Library panel is the first one
([ADR 0036](../adr/0036-gui-library-panel-owns-its-interaction-state.md),
and `docs/architecture/interactive-surface-ledger.md` records it GUI-owned
with no Surface state). Such a surface skips step 1, names no region in step
3, mounts no leaf in step 4, and mounts only its `ComponentId` in step 5: its
state is the window content's own, and the core supplies its data and its
actions.

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

### Library Hero and list shared pieces

Invariant 30 (`docs/invariants/30-shared-ui-is-never-bent-to-fit-a-consumer.md`)
governs these. `LibraryHeroFrame` paints the Hero for Music, Shows, Podcasts,
Audiobooks, Feeds, and Movies: a required header band (the largest unit's
kicker, title, and plain facts — a movie is its own largest unit), the detail
block (art, selected unit, actions, overview), and the list band (the movie's
credits table and the IMDb button).
`LibraryWorkspaceTable` paints the list band's rows,
`LibraryHeroActions` the Play, Play next, and Add to queue buttons,
`LibraryPillButton` every pill, `LibraryItemRow` and `LibraryLetterHeading` the
flat list rows, and `LibraryVirtualList` the scroller of every list. Home still
paints `LibraryHeroSummary`.
