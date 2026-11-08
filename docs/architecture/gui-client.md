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
recovery dialog"), a TUI-owned surface the GUI does not yet present, so
the GUI window stays open and no GUI-local recovery path exists yet.

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
popover, the playback-target chooser, and the shared confirmation modal with
its message and button row), the Queue playback projection
(`GuiQueuePlayback`), the footer pills (`GuiQueueFooter`), the Status bar
projection (`GuiStatusBar`), the Library panel's section (`GuiLibraryFrame`;
*The Library panel* names it), and the theme when it is armed (`GuiTheme`).
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

Front end to Rust there are three commands, registered in
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
`StatusBarTargetId` names `StatusBarHitTarget` — plus `ConfirmTargetId`, the
confirmation modal's buttons by index, and `AttachTargetId` for the
pre-attach buttons. A confirm button owns no surface: the core resolves its
press itself (`App::execute_gui_confirm_button`) and mounts no leaf for it.

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
indicator are Lucide icons. The item rows paint no playing marker: the
active row's canvas green fill with dark ink marks it (the `GuiQueue`
frame still names `active`). The transport glyphs paint
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
| Filter text and Search query text | The item whose detail the Hero shows |
| The open Hero overlay and the item menu | Service data, its loading, and the artwork |
| The Library's keys while it has Panel focus | Panel focus (`FocusStack`) and the actions |

The data selection is core state because the core decides what to load and
what to save at exit. The window content changes it only by a command, and the
frame carries it back, so the pill row paints the core's set. The panel uses
no TUI component, no `mbv-surfaces` Surface state, no `GuiSurfaceLeaf`, and no
`[keys]` action, and its pointer input never reaches the core's gesture
recogniser or hit map. `LibraryPanel.svelte` paints `LibraryTopBar`,
`LibraryPills`, `LibraryList` with `LibraryLetterRail`, `LibraryHero` with
`LibraryTracklist`, and `LibraryStatusBar`, and the right-click item menu
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

`GuiLibraryContent` carries `tabs` (the presented destinations in catalog
order, each a `GuiLibraryTabFrame` with a `position` and a `label`, Home at
position 0), `current_tab` (the active tab's position, `None` for Home and for
every destination the GUI does not present yet), and `music`
(`Option<GuiMusicFrame>`, present while a Grouped Music library is active).

`GuiMusicFrame` carries `lib_idx` (the Grouped Music library index the
set-collections command names), `pills` (`group_id`, `name`, `selected`, and
`collection`), `loading`, `failure` (the failure line naming the selected
groups whose album list failed), `artists` (`artist_key`, `name`,
`sort_letter`, and `albums`), `hero`, `art`, and `playing_track_id` (the
playhead's Emby track id, from the Queue panel's own authority, which the
tracklist's and the list's playing rows paint — the canvas's green fill with
dark ink, as the Queue's active row paints). An album carries `album_id`,
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

### The `library` command

`library(GuiLibraryCommand)` is the fourth bridge command (`src/main.rs`). It
only pushes `CoreInput::Library` into the core channel, as the other commands
do. `src/bridge/library.rs` translates the wire variants and
`App::apply_gui_library_command` (`src/app/gui_library.rs`) applies each one
through the existing `App` action, so the TUI's own paths do not change. The
variants are `openDestination` (`position`), `setCollections` (`libIdx`,
`groupIds`), `showDetail` (`itemId`), `openAlbum` (`albumId`), `takeFocus`,
`play` (`itemId`), `playNext` (`itemId`), `addToQueue` (`itemId`),
`shuffleArtist` (`albumIds`), `menuEntry` (`itemId`, `index`), and `rowMenu`
(`itemId`). Item
identities are stable Service ids, never row positions, so a command built
from a stale frame addresses the item that frame painted or nothing. The
`takeFocus` variant also moves the core's `FocusStack`, so the frame paints
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
before the forwarder (design D3). The order is:

1. While a core overlay is open (the row menu, the track popover, the
   playback-target chooser, the confirmation modal) or prefix mode is armed,
   the key goes to the core.
   Both facts are in the frame. The Queue filter's text entry is the core's
   too, so every key is the core's while it holds the keyboard.
2. While a text field of the panel holds the keyboard (the `/` filter), the
   field takes the key. Esc closes it and Enter accepts it.
3. While the Library panel shows, the window keys run: Tab and Shift+Tab move
   the pane focus (the pill row, the list, the Hero, and across the panel
   boundary to the Queue panel), and `1` to `7` open the first to seventh
   destination tab after Home.
4. While the Library panel has Panel focus, the Library keys run in the window
   content: `/` opens the list filter, Enter plays the item under the cursor
   or presses the focused control, `n` plays it next, `a` adds it to the
   queue, Space and Alt+Space toggle and solo a focused pill, the arrow, Page
   Up, Page Down, Home, and End keys move the cursor, Right and Left open or
   close a tree node or walk the pill row, and Esc closes the open filter,
   overlay, or menu.
5. Every other key goes to the core's keyboard router. So Space plays or
   pauses while no pill has focus, `[` and `]` seek, and the GUI-only chords
   `z`, `q`, and `c` and every `[keys]` binding keep their effect.

With Queue Panel focus the Library takes no key, so `/` opens the Queue filter
and `a` opens the audio popover as before (design D3).

### The arrangement

`libraryArrangement` (`ui/src/lib/bridge/arrangement.ts`) picks the
arrangement from the measured Library column width — the window width minus
the Queue panel's current width, 400 px expanded or 72 px in the rail — with
the canvas sizes as thresholds. Below 480 px is Mini, from 880 px with at
least 600 px of height is Wide, and between them is Narrow. The thresholds are
front-end constants beside the Queue panel's state constants, and the core
does not classify the size. Mini mounts only the panel that holds Panel focus.
Narrow mounts both panels, and a picked item opens the Hero as an overlay over
the list. Wide mounts both panels with the Hero beside the list. The Queue
panel keeps its four states inside every arrangement, and a resize across a
threshold moves no cursor, pill selection, or picked item, because those live
in the window content and the core.

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
destination, the selected collections of each destination, and Panel focus as
stable Service identities. The core reads it once before the first step, saves
it at orderly exit before `client_exit_teardown`, and never while the window
is open.

The restore runs in two phases (`GuiLaunchRestore`,
`CoreThread::restore_launch_location`). The first phase opens the saved
destination once the catalog presents it — a saved destination the GUI does
not present falls back to the first presented one — and applies the saved
Panel focus over the Library focus the open takes, unless the user already
took focus while the restore was pending. A start with no saved location — or
one that fails to load — restores through the same path
(`mbv_config::first_launch`): the first presented destination, Library Panel
focus, and no saved collection sets, whose empty set is the show-all
selection, so the Library panel is reachable from the first launch. The
second phase applies the saved
collection sets once the destination's group list lands: a saved group the
library no longer holds drops, and an empty result — a saved set the library
emptied, or no saved entry — is the show-all selection. The restore applies
to a destination that holds no selection and never clobbers one the session
already made. A mid-restore tab switch abandons the saved sets rather than landing
them under another destination. The restore is idempotent, so the catalog and
library drains call it whenever their content can complete a phase.

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
