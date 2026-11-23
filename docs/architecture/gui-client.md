# GUI Client guide

Read before any change to `crates/mbv-gui/`: the core thread, a bridge DTO, a
window section, a leaf, a reported region, the theme, or the window. Term
definitions live in `CONTEXT.md` and win any disagreement with this file. The
`gui-client` spec owns the requirements. The design of the first GUI change
(`add-tauri-gui-client`, Tauri) recorded the first decisions, and the ADRs
record the later ones. This file describes what the code does.

The GUI Client is the Dioxus Native program `mbv-gui` (`crates/mbv-gui/`),
rendered by Blitz in the same process as its core thread (ADR 0039). It is a
workspace member outside `default-members`: the default `cargo build` and
nextest runs skip it, while `--workspace` clippy and fmt cover it. It needs no
webview and no Node toolchain. Its GPU surface needs a Vulkan or GL driver.
`make dev` builds `mbv` and `mbv-gui` together, and starts the window with
`cargo run -p mbv-gui`. Both binaries
must come from one build: the GUI starts its Owner from the sibling `mbv`
binary, and the handshake refuses an Owner with a different build id.

## Process shape: one core thread

The window's event loop runs on the main thread. `src/main.rs` installs the
app log, starts the core thread with `core_thread::spawn`, and launches the
window (`src/window/launch.rs`, a copy of the `dioxus-native` launch body with
five changes: the net provider, the viewport and frame hooks, the window
attributes, the shell proxy for the quit edge, and no tokio runtime). It joins
the core thread after
the event loop ends so a quit's exit teardown completes before the process
exits.

`src/core_thread.rs` runs one dedicated, synchronous thread. It loads the
configuration, runs the Owner attach campaign (`src/attach.rs`) — before
`attach_owner` the GUI claims its `ClientCapabilities` (`ShownQueueEntries`
and `TransportChipKeys`) and its no-Pin-swap participation itself, and
`CoreThread::new` calls `skip_launch_restore` on the attached `App` — and
then runs `ClientCore::tick` in a loop over a `GuiCore { app, gui }`
(ADR 0042). `GuiCore` wraps the attached `App` by composition and owns the
`GuiClientState` beside it: every GUI-only field, the library stores the
GUI's projections read and write, the open overlay, the Settings and
Playlists screens, and the panel toggles. The core never names a field of
`GuiClientState`; the state is its own `ClientLibraryStores` implementor, so
the core's library-event drain applies each event's collected
`ClientLibraryEffect`s through it, after that event and before the next.
The thread waits on a `std::sync::mpsc` channel of `CoreInput` values
(`Key`, `Pointer`, `Layout`, `Library`, `Settings`, `Sync`, `Art`, `Wake`,
`Close`) with a timeout of `CLIENT_IDLE_CADENCE`. `Art` carries one image load
that the `mbv-art` scheme raised: an `ArtRequest` with the request path and a
reply callback that answers it once. The installed `ClientWaker` sends
`Wake` into the same channel, so Owner events and timers wake the core as
they wake the TUI. `CoreThread` implements `ClientTickHost` over the slice's
Surface state; the drains for surfaces the first slice does not present are
documented no-ops. The GUI's Client-only fetch results (a group's albums, a
plain Emby library's root list, a Shows library's New episodes) do not ride
the shared library event enum (ADR 0042): their workers send a
`GuiClientEvent` on a channel the `App` owns, and the shared tick drains it
through `ClientTickHost::drain_client_events` (`GuiCore::drain_client_event`,
looping) after the library drain; a landed fetch wakes the tick through the
same `ClientWaker` the other channels use. The card-image completion drain
(`App::drain_card_image_completions`, the shell drain without the
TV-workspace re-push) runs because the Queue playback visual slot paints: it
lands the slot's artwork fetches in the image cache the `mbv-art` scheme
answers from.

The window's event handlers never touch the `App`. They only translate the
input and push it into the channel through `CoreLink`. No mbv code in
`mbv-gui` is `async`. The `dioxus-native` asset resolver is the only tokio
edge in `mbv-gui`.

The core thread sends a section batch after every tick that changed state,
through `SectionLedger::collect` (`src/sections.rs`, design D2): the compared
sections build on every tick and compare with the last built value, the
gated sections build only while a mark names them, and the batch goes out
only when at least one section differs from the value the window holds. A
timeout tick with nothing changed sends nothing. The wait on
`CLIENT_IDLE_CADENCE` is a tick timer, not a repaint timer: the shared tick
has time-based phases (the delayed settings save, the session polls, the
websocket keepalive, toast expiry), so the wait stays, and a tick it ends
sends only what changed. The window asks for a full snapshot with `CoreLink::sync` when its root
mounts, so a restarted window does not wait for a state change. When the
event loop ends, `main` delivers `CoreInput::Close`; the run ends as a quit, and the exit
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
`GuiOverlay::DaemonLost` overlay (`src/window/components/daemon_lost_overlay.rs`, the
`daemonLost` overlay section). The window stays open and dead until one of
the dialog's three choices answers it, and each answer runs the shared
`App::apply_daemon_lost_intent` — the same action the TUI's modal handler
runs — so the GUI holds no recovery path of its own.

## The bridge contract

The core sends the window one message, `WindowEvent::Batch`, on the window
channel. It carries a `GuiSectionBatch` (`src/bridge/sections.rs`), the one
update the core sends for a tick. The batch is a Rust value that crosses the
channel as it is: no serialization runs. The batch has one optional field
for each window section; a present field is a `GuiSection { revision, value }`,
and an absent field means the window already holds that section's revision.
A section whose content can be absent has an `Option` value, so a `null`
clears it. The sections are: the pre-attach state (`attach`, `null` once the
attach succeeds), the theme (`theme`, `GuiTheme`), the
Queue viewport rows with their stable `QueueSlotId` raw numbers, the active
row, marks, active slot, row hover, and row drag (`queue`, `GuiQueue`), the Queue
chooser facts the header's controls read, the filter text with its caret
fact, and the totals the footer shows, beside the folded strip's remaining
count, next item title, and finish tail (`summary`, `GuiQueueSummary`), the collapse and queue-fold
toggles the
GUI Client holds in its `GuiClientState`, with the UI-reset counter
`ui_reset_epoch` beside them
(`panel`, `GuiPanelToggles`), the open menu or popover (`overlay`,
`Option<GuiOverlayFrame>`: the row menu with its entries, key hints, and
destructive flags, the track popover, the playback-target chooser, the
Queue chooser with its cursor, entries each with its slot count, and rename
draft, the shared
confirmation modal with its message and button row, and the
daemon-lost recovery dialog with its diagnostics and recovery choices), the
Queue playback projection (`playback`, `GuiQueuePlayback`), the scrubber's
hover preview (`hover`, `Option<GuiScrubberHover>`), the footer pills
(`footer`, `GuiQueueFooter`), the Status bar projection (`status`,
`GuiStatusBar`), the Library panel's three sections (`libraryList`, the destination tabs and
the list part; `libraryDetail`, the detail part; and `rowMenu`, the open item
menu; *The Library sections and their revisions* names them), the Search field's
section (`search`, `Option<GuiSearchFrame>`), and the Settings screen's
content (`settings`, `Option<GuiSettingsFrame>`; *The Settings screen* names
it). The window picks the panel state from the `panel` section's toggles and
its own measurement of the panel container; no section carries a breakpoint
class (design D10).

A section's revision starts at 1 and grows by one for each built value that
differs from the last built value. The compared sections build on every tick
and compare with the last built value, so a build that matches sends
nothing. The gated sections (`theme`, `library` for the three Library sections, `search`, `settings`) build
only while a mark names them (design D3): an input that resolved, a drain
hook that reported work, a passed deadline, or a moved playhead. The
pre-attach campaign (`src/attach.rs`) sends its own batches of the `attach`
and `theme` sections while the window shows a pre-attach state, and the
attached core's first batch carries `attach: null` at a revision past the
campaign's (design D4).

`GuiQueuePlayback` carries the player panel's facts. The status-indicator
cluster (`statusIndicators`) is the resolution, audio, and subtitle labels
the shared projection builds from the same indicator data the TUI's spans
build from: the `playback` section carries each uppercased label with its
foreground role, and the field is `None` while the local player is inactive.
The canvas player panel paints no cluster, so the field travels unused in
the slice. The visual slot's artwork (`artwork`) is a `GuiArtRef`: an `image` reference
to `mbv-art://localhost/card/<key>` for a cached card, a `placeholder`
reference to the bundled placeholder, a `loading` reference with no URL while
the fetch is in flight or awaits its retry, or no reference when the slot has
none. `src/art.rs` builds every URL, and the core answers each load from its
image cache through an `ArtRequest` (`src/art.rs`). The `playback` section's compare
carries the reference again only when the slot's art changed (design D10); a
window restart's `sync` re-sends the section. The window paints the
reference through `src/window/components/art_image.rs`. The payload is a URL, never the cached
bytes, so the section does not grow with the artwork. The net provider
(`src/window/net.rs`) answers a cached key with the source's bytes. A key with
no entry sends `ArtEvent::Failed` for the URL, and the window asks again on its
capped retry (*Clocks and frames*). The GUI attaches with `ImageDecode::SourceOnly`, so its image cache keeps
only the server bytes and the size from the image header, and never decodes
pixels; only the TUI decodes, for its terminal graphics. The panel
reports the slot's `visualSlot` region (the TUI's hit target), and the art
element's resizes report the regions a swapped bitmap moved. Inside one
report the last region pushed wins, so the panel marks `visualSlot` first and
the transport buttons and the seekbar follow it in mark order, while the chips
and the footer's controls report from their own region sets after it — so a
press on a control over the artwork lands
on the control, not on the artwork. The section also carries the core's
`auto_hide` preference, the fact the stage's `data-auto-hide` attribute
reads (below); the window derives the stage's `data-playing` fact from the
playhead anchor.

The scrubber's hover preview (`hover`) carries the pointer's fraction along
the painted bar and the time it names (`GuiScrubberHover`); the covered
chapter and the trickplay tile the old preview carried are gone
(`gui-queue-panel-redesign` design D4, D5). A move away from the bar carries
no hover.

The window reaches the core through six methods on `CoreLink`
(`src/window/channel.rs`). Each only pushes into the core channel:

- `key(key, target)`: a neutral key with the panel target the pointer
  resolved for it (ADR 0040). `src/window/input.rs` builds a `DomKey` from
  the Dioxus keyboard event, and `src/bridge/keys.rs` translates it to a
  neutral `KeyEvent`, following the TUI's crossterm
  edge: SHIFT folds an uppercase letter to lowercase, Shift+Tab is `BackTab`,
  Meta produces `Key::Null` (the neutral modifiers have no Meta bit), and a
  key the vocabulary cannot name is `Key::Null` too.
- `pointer(PointerEvent)`: a neutral pointer event. `src/window/input.rs`
  builds a `DomPointer` (kind: press, release, move, wheel, cancel; button;
  held buttons; position in logical pixels; wheel deltas), and
  `src/bridge/pointer.rs` translates it to `Option<PointerEvent>`, mirroring the
  TUI's `neutral_pointer`: a move with the primary button held is the neutral
  drag, the vertical delta's sign picks the wheel direction, and an event the
  neutral vocabulary cannot represent drops. A `cancel` is the one kind no
  button or position decides: the window sends it when the window loses the
  gesture — a `pointercancel` or a `pointerleave` — and the core drops the
  drag state and the double-click pair and sends no move. An element's blur
  sends none: Blitz blurs the root on every click that moves the focus, and a
  forwarded blur erased the double-click pair on each press.
- `layout(LayoutReport)`: whole visible rows per list, the painted
  regions with their semantic target ids, and the panel container's painted
  size in logical pixels (`src/bridge/layout.rs`).
- `library(GuiLibraryCommand)`: the Library panel's one semantic request,
  pushed into the core channel as `CoreInput::Library` (*The Library panel*
  names the variants).
- `settings(GuiSettingsCommand)`: the Settings screen's one semantic request,
  pushed into the core channel as `CoreInput::Settings` (*The Settings screen*
  names the variants).
- `sync()`: the window asks for a full snapshot when its root mounts. The core pushes `CoreInput::Sync`, and
  the ledger releases every section the window holds, so the next batch
  carries every present section at its current revision. No batch can be lost
  between the mount and the snapshot, because the channel holds it until the
  root drains it. A `sync` marks no section.

The window interprets no input apart from the Library panel's own keys and
gestures (*The Library panel*). `src/window/input.rs` builds the `DomKey` and
`DomPointer` values from the Dioxus events. It prevents the default action of
every keydown that the window does not leave to a focused text field, so no
browser-style default runs, and it converts a wheel delta in lines or pages to
logical pixels before it sends. A keydown meets the window's key router first
(the Settings screen's steps, then the Library panel's), and only a key the
router leaves to the core reaches the `key` method. A lost gesture — a `pointercancel` or a
`pointerleave` — forwards the `cancel` kind; a blur does not. The core's `PointerGestureState` recognizes
the gesture; a recognized gesture's point resolves against the combined hit
map of the last `LayoutReport`, and `App::route_gui_pointer` receives the
surface that owns the region as its leaf. A point in no reported region
resolves to nothing. The leaf adapters (`src/leaves.rs`: `QueueLeaf`,
`PlaybackLeaf`, `StatusBarLeaf`) are built per routing call over the core's
fields and emit the same semantic `Msg`s the TUI components' pointer paths
emit.

Every key rests on one DOM fact: the document's focus sits on the root
`main`, which carries the window's only `keydown` handler (`ROOT_ELEMENT_ID`
in `src/window/app.rs`). Blitz clears the DOM focus after a click that
matches no text input, checkbox, radio, summary, label or link — every queue
row is a `button` and every panel is a `div` — and it drops the focus when
the focused node is removed, so the launch copy gives the root the focus
back after each window event (`restore_root_focus` in
`src/window/launch.rs`, invariant 33). Without that repair the shell delivers
every keydown to the document's `<html>` element, which carries no Dioxus
id, and no key of the window works at all.

## Layout reports and the hit map

`src/window/layout.rs` is the single layout registry. Every mounted
surface reports through one region set (`src/window/layout/region_set.rs`),
created once in its own body with `use_region_set` for a signal-driven
surface or `use_region_set_for` for one that triggers through reactive
props. The set holds the surface's report keys — one for its base
contribution and one per overlay, made in that order — and the reporter
merges every set's contribution of list capacities and painted regions into
one `LayoutReport` and sends it through `LayoutReporter::report_layout`.
Reports coalesce to one send per turn of the event loop, and a contribution
identical to its last value sends nothing — a batch that moves only the
active row moves no region.

A set's `mark`, `mark_over`, and `mark_list` calls are the surface's whole
report: each call records the region target or measured list it paints, and
mark order is report order. Marks run only in the surface owner's own
render — inline in its `rsx` or in `let`s before it — and each returns a
`MarkHandler` (`EventHandler<Event<MountedData>>`) that the owner passes to
the child component that owns the element as a prop; the queue header's
controls, the playback stage's and scrubber's controls, and each popover's
rows take theirs this way. A child never calls `mark`: Dioxus memoizes a
child whose props compare equal and renders it after its parent, so a mark
inside a child could be skipped while its element still paints, or land after
the parent's own marks. Each render starts at the hook call, which clears
the painted list, so one render's marks are the whole of what it reports.

One effect spawns one report task. It resolves the base contribution's
painted targets in mark order and reports them under the base key, then does
the same for each overlay in index order, so a popover's regions merge after
the content beneath it (the hit map's last push wins) and an overlay with no
marks retracts its key, which lets its next open push at the end again. A
base with no marks still reports its empty contribution. The task keeps only
the handles of the targets the latest render painted, so a row that scrolled
away leaves no region behind; that prune replaced the popovers' per-row
truncation, which is gone on purpose. The merge rule is unchanged: a key
keeps its insertion position, a re-set updates in place, and the core's hit
map rebuilds from each merged report. The set retracts every key in
`use_drop`.

A report marks no section and forces no batch (design D4): a layout change
that moves a section's content moves it through the section's own compare,
and a report that moves nothing sends nothing. The core rebuilds its whole
hit map from each report, so a surface must
never send a partial one: report on mount, report after each paint that
moved a region, and retract on unmount. Even a surface with no
interactive regions reports its (empty) contribution, so the merged report
accounts for it.

The core's `apply_layout` (`src/core_thread.rs`) does three things: a list's
`whole_rows` becomes the surface's viewport height (the paging stride and
resting-scroll clamp — `QueueSurface::set_viewport_height`); the reported
regions rebuild the combined `HitMap<RegionTarget>` (`hit_map` in
`src/bridge/layout.rs`, in paint order); and the reported container size is
stored as painted (design D10). The core does not classify the size. The window
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
scroll. The core's viewport decides which rows the `queue` section carries;
wheel
input goes to the core like any pointer event, so scroll follows the active
row through the same Surface state rules as the TUI. The list's rows are 34
logical pixels tall with 2 px gaps — a 36 px stride
(`src/window/components/queue_panel/geometry.rs`) — and the window measures
the list's height and reports whole rows as capacity: a box fits
`floor((height + gap) / stride)` rows, so a stacked row starts one stride
below its neighbour. `track_container_size`
(`src/window/layout.rs`) measures the panel container after each layout epoch
and reports that size through the same reporter, so the state it paints and the
capacity the core applies come from one measurement.

## Clocks, the stylesheet, and the ellipsis

The window has no `setTimeout` and no `requestAnimationFrame`. `src/window/clock.rs`
provides both as a `Clock`.

- **Timers.** `TimerClock` runs one thread with a deadline queue. A passed
  deadline sends `WindowEvent::Timer`, and the root routes it to the surface
  that asked. The search debounce (200 ms, `library/search.rs`), the artwork
  retry, the minute clock, and the playhead tick use it. Tests inject a clock,
  and no test sleeps.
- **Playhead tick.** While it plays, the playhead arms a timer at the predicted
  position's next whole second, so the window repaints about once a second.
  Nothing paces at vsync: a frame loop repainted the whole Blitz scene every
  frame and held the main thread near half a core.
- **Frames.** Every presented frame makes the launch copy's redraw hook send
  `WindowEvent::Frame`. Nothing requests a redraw from that hook, so the window
  presents frames only when the tree, a resize, or a net answer gives it work,
  and a paused window still answers a pending re-measure (see below).
- **Artwork retry.** A missed image keeps its loading fill and asks again for
  the same URL. The wait starts at 1 s, doubles, and stops at 30 s
  (`components/art_image.rs`). A new URL drops the failure and its timer.

The stylesheet is `assets/app.css`. It imports Tailwind CSS v4 and the vendored
shadcn and `tw-animate-css` files (MIT, in `assets/vendor/`). It scans
`src/window` for classes, and it declares the six `@font-face` rules. `make
gui-css` builds `assets/app.generated.css` from it with the Tailwind standalone
CLI, release v4.3.5. The developer installs the release asset
`tailwindcss-linux-x64` (SHA-256
`e47695cee3f0d90623a91f3939b08bdee610eb5ad99340adff9d4145b284d6cc`) on `PATH`
as `tailwindcss`, or passes `TAILWINDCSS=/path`. The generated file is
committed, and `src/window/app.rs` includes it with `include_str!`. A normal
build runs no CSS tool, and CI does not check the committed file against the
sources. A class added to the Rust sources needs `make gui-css` before its
commit. A fresh worktree needs only the CLI on `PATH`; no Node package is
installed.

`Truncate` (`src/window/truncate.rs`) replaces the `truncate` text box. It keeps
the box's `MountedData`. When the full text does not fit the content width, an
effect writes a cut text that ends in `…`. The measure uses one process-wide
`parley` font context with the six bundled faces, and a `TextStyle` that each
call site passes with the family, size, weight, and letter spacing of its class
string. A binary search over grapheme boundaries finds the longest prefix that
fits with the ellipsis. The `truncate` class stays, so the box still clips. A
measure that runs one frame late shows a clipped text, not an overflow. A box
with no measured width — a client rect of zero, from a document whose layout
has not resolved — keeps its whole text and clips it; it never paints the
ellipsis alone. A box mounts unmeasured: its first measure can read the layout
from before its insertion, so that first measure sets a window-wide re-measure
request, and the drain's presented-frame handler (`WindowEvent::Frame`) takes it
and bumps the layout epoch once, after the frame resolved the document. Only a
first measure asks, so the re-measure it drives asks for nothing and a
collapsed box settles after one bump.

A `Truncate` box measures its own width, so that width must come from the
layout, never from the text it paints. Give the box `min-w-0 flex-1` in a row,
or `w-full` in a column, and give every flex ancestor up to a fixed-width one
`min-w-0 flex-1` or `w-full` too. A box sized by its text keeps the cut of its
first narrow measure: the cut text is the width it re-measures, so it never
grows back when the room does. An ancestor that is content-sized is the usual
cause (the now-playing title block, the Library workspace name cell, and the
target chooser rows had it).

## Theme mapping

The `theme` section's `GuiTheme` (`src/bridge/frame/theme.rs`) carries two
things. The
`roles` map holds one colour per role: `GuiRole` mirrors `mbv_theme::Role`
variant for variant — `Role` stays foreground-only — and the two `From`
conversions are exhaustive, so a new role fails the build until both sides
carry it (`GuiRole::ALL` is the one unchecked list; add the role there too).
The `chrome` section carries the eight fill values the shadcn
chrome variables point at (`background`, `card`, `popover`, `muted`,
`secondary`, `sidebar`, `border`, `input`) plus the queue column's own
surfaces: `queueColumn` (`Surface::QueueColumn`, the gutter the column
paints), `queuePanel` (`Surface::QueuePanel`, the recessed box),
`queuePlaybackBand` (`Surface::QueueOnlyPlaybackPanel`, the fill of the
compact strip and the collapsed rail), and `queueFooterBand`
(`Surface::QueuePanelBand`, behind the footer pills). Each chrome fill
resolves the `Surface` the TUI paints the same concept with through
`surface_colors`, at its resting fill — the queue column paints one fill at
every focus state, and the TUI keeps its focus-reactive pick — so the
`mbv-theme` surface table stays the one fill authority and a palette edit
recolours the chrome together with the roles.

The queue column's paint names no `mbv-theme` slot, and
`gui-queue-panel-redesign` gave it its own token set: `assets/app.css`
declares the design's Everforest Dark Hard values as `--ef-*` custom
properties — the player backdrop, the recessed list, the header bar, the
row and hover fills, the row states, the seek bar's greys, and the text
colours — and the column's components read those tokens, never a literal
hex. The `--canvas-*` tokens remain where components still read them: the
drag chrome the rows paint — the insertion line and the ghost — the row
menu, popover, and modal chrome, the Library
column's panels, and the typed-text colour tokens. A
`--canvas-*` token no component reads any more is deleted. No `theme.rs`
remap rides along: `mbv-theme` stays the TUI's palette
(`align-gui-with-design-canvas` decision DR9).

The theme is a gated section (design D4): the ledger marks it once at core
start, so the first batch carries it; a palette change marks it again; and
a `sync` re-sends it, so a window restart gets the theme without a
state change. `src/window/theme.rs`
writes each role and chrome fill into a `--mbv-<name>` custom property on
`:root`, and `assets/app.css` points every shadcn variable at the mbv
properties. There is one palette and no light or dark variant.

### Colour by data type

Weight and size carry hierarchy; colour carries the data type (design D14).
A title or a body line stays `--canvas-heading`, `--canvas-text`, or
`--canvas-muted`, and each data type names one `--canvas-type-*` token in
`assets/app.css`. Each token aliases the hue the palette already carried, except
the secondary title's purple, which the GUI adds.

| Type | Token |
|---|---|
| time and duration | `--canvas-type-time` (`--canvas-accent`) |
| year and date | `--canvas-type-date` (`--canvas-source`, the link hue) |
| count and meta | `--canvas-type-count` (`--canvas-partial`) |
| section label | `--canvas-type-label` (`--canvas-insertion`) |
| secondary title (series, show, parent) | `--canvas-type-secondary` |
| role and character | `--canvas-type-role` (`--canvas-owner`) |

A line that joins values of several types is built as typed segments where it
is built, never split in the window. The core's Hero meta rows and the
window's own facts lines (`src/window/library/hero.rs`) return segments, and
`src/window/components/typed_text.rs` paints them: each segment maps its
kind to its token, and a plain segment inherits the line's own colour. A
time or a date repeated down a list — a duration column, a Music tree row's
year — paints at `--canvas-type-repeat-opacity`; a
selected or current row keeps its own ink at full strength by painting
without `repeat`. The transport row's remaining clock paints at 0.85
opacity, as the design draws it. A single-type field needs no segments: its painter names its
token (`--canvas-type-count` for an episode badge, `--canvas-type-secondary`
for a parent line).

## Text and icons

The window bundles the two canvas faces (design D8): IBM Plex Sans at
weights 400, 500, and 600 for text, and JetBrains Mono at the same weights
for times, key hints, chips, and counters, with tabular figures. The latin
woff2 subsets of `@fontsource/ibm-plex-sans` and `@fontsource/jetbrains-mono`
(5.3.0) sit in `assets/fonts/`, and `src/window/net.rs` compiles them into the
binary with `include_bytes!` and answers `mbv-asset://localhost/font/<file>`.
The window reads no font from disk at runtime. `assets/app.css` sets
`--font-sans` to `'IBM Plex Sans', system-ui, sans-serif` and `--font-mono` to
`'JetBrains Mono', monospace`. The window loads an image only through the
`mbv-art` scheme, which the net provider answers.

The GUI paints no emoji and no Nerd Font glyph. Icons come from
`src/window/icons.rs` only (the Lucide set, drawn as inline SVG), with two
exceptions, both because Lucide carries no brand glyph: a playback-target
chooser row paints the brand mark of its target kind (`src/window/brand_icons.rs`)
for Emby, Google Cast, and the mpv Player owner, and a Library list header
paints its destination's Service mark (`src/window/service_mark.rs`), the RSS
mark excepted because it stays the Lucide `Rss` icon. Both painters draw
simple-icons paths (CC0); `brand_icons.rs` keeps the chooser marks and reads
the Emby path from `service_mark.rs`, so the Emby path exists once. The transport controls and the playback-target indicator are Lucide
icons. The shown queue's start slot shows the canvas's play glyph
beside its title, in the accent colour: filled while its queue plays, and
outlined while it is stopped or resting (the `queue` section still names
`active`). The row keeps its own fill, because a highlight marks focus only. The transport glyphs paint
as solid shapes — `fill` and `stroke` both the role colour, as the TUI's
solid transport glyphs paint — with the availability the colour already
carried gating them. The footer's playback-target control maps both of
`target_icon`'s states to the one Lucide `Cast` — the TUI paints the
cast family for local and remote alike — so the role colour and the
host label carry the distinction. `GuiFooterPill.text` carries glyph-free
text. The autosave and dirty pills map to their Lucide icons in the
component (`SaveCheck` and `SaveOff`); the queue's source text paints plain
label text with no leading icon, as the canvas draws it.

## The panel's four states

`src/window/panel.rs` picks one of four states (design D10):
expanded, compact strip, expanded without queue, and rail. The state comes
from the measured size of the panel container plus the `panel` section's
two toggles.
It never comes from the window size, and the window holds no interaction
state: the size is a measurement and the toggles are the Client core's.
`track_container_size` measures the container after each layout epoch and
reports the size through `LayoutReporter::report_container_size` — the same
measurement `LayoutReport.container_size` carries to the core — so the painted state, the
list capacity, and the hit map follow one size.

The `z` toggle (`GuiPanelToggles.collapsed`) wins over every size and paints
the 72 px rail at the window's right edge: a progress ring around the
artwork, play/pause, next, the
remaining time, and a control that expands the panel again. A measured
height below 520 px paints the compact strip: the 96×54 still, the title,
the parent line, play/pause, next, and a thin progress line. Its next-item
line reads `GuiQueueSummary.nextTitle`, which the core resolves from the
queue rather than from the painted rows, because the strip paints no queue
list. The `q` toggle (`GuiPanelToggles.queueFolded`) paints the expanded
artboard with the queue replaced by its one-line summary. A height of zero
reads as "not measured yet" and paints the expanded artboard, so a restart
does not flash a strip. `src/window/components/panel.rs` holds the artwork
above the state branches, because a frame re-carries it only when it
changed: a state that mounts on a later frame paints the value the expanded
artboard already adopted.

The window sets a 400×200 logical-pixel floor, the Tauri build's
`minWidth`/`minHeight` (`tauri.conf.json`): `window/launch.rs` builds the
window attributes with `with_min_surface_size`, so a floating window cannot be
dragged below the size at which the compact strip lays out. A tiling manager
may impose a larger floor; that floor is the compositor's, not the window's.

## The player panel and the queue

The slice's one window paints the design canvas's player panel on the queue
column's backdrop (`src/window/app.rs`, `src/window/components/panel.rs`).
`src/window/cells.rs` fixes one TUI text row at 28 logical pixels and
one text column at 14 (the terminal cell's 1:2 aspect at that height), and
every panel offset — the paddings, the gaps, the queue box's inset — is a
`calc()` over the `--mbv-cell-row` and `--mbv-cell-col` properties.

The `align-gui-with-design-canvas` change aligned the queue's paint with the
canvas and nothing else: a paint-only restyle of the queue's four bands
with no new command, derivation, key, or contract. The artboards (`Main.dc.html`, `Menus.dc.html`)
and the change's gap audits, copied into that change's directory under
`openspec/changes/archive/2026-11-07-align-gui-with-design-canvas/`, are the reference for
the exact values; this file records what the paint changed, not every
canvas value.

The queue column's backdrop (`queueColumn`) owns the window. The column paints
no player header row: the stage is its topmost content
(`move-playback-header-into-queue-footer` D6), directly under the global
header (below). The window paints no now-playing status word on any surface
(D5) — a paused panel reads from the transport's play glyph. The playback
target and the collapse controls moved to the Queue footer's left group
(below), and the footer's press on the target opens the `targetChooser`
overlay upward; the chooser's rows carry the choices with the attached output
marked, and Esc or a press elsewhere closes it in the core. The title and
parent line sit below the stage: where the TUI composites the title into the
artwork's top text row with a scrolling marquee, the GUI measures and cuts it with
`Truncate`. The stage is full-bleed at the column's top edge: a loaded image
fills the panel's width and takes its height from the artwork's aspect, with
no fixed pixel box.

The stage (`queue_playback_panel/art_stage.rs`, design D1) is a `relative`
wrapper with no clip, and it carries the `visualSlot` region for the whole
box. It holds three siblings: the media box, the clipped one — so only the
image is cut, and a track popover that hangs past the artwork's bottom
paints in full — the absolute top layer, which holds the track chips at its
left and the season-and-episode badge at its right and always shows, and the
absolute bottom layer, which holds the solid `rgb(0 0 0 / 0.6)` scrim (the
`gui-queue-panel-redesign` design's gradient became the solid band at the
live test) and the two control rows.
While the image loads, the media box stays empty at the panel's width with a
16:9 aspect, and both overlays paint on it as if the image were there; the
box takes the image's own aspect when the image loads.

The bottom layer's visibility is the stylesheet's, and the window holds no
hover state (design D2). The stage carries `data-auto-hide` while the core's
auto-hide preference is on and `data-playing` while the playhead anchor is
active and not paused. Each attribute is present only while its fact holds —
never `"false"` — and one rule set in `assets/app.css` acts on them:

```
.stage[data-auto-hide][data-playing] > .bottom         { opacity: 0 }
.stage[data-auto-hide][data-playing]:hover > .bottom   { opacity: 1 }
```

The controls show at all times by default. With auto-hide on, they hide
while an item plays and the pointer is away from the artwork, and show while
the pointer is over the artwork and while the playback is paused. The hidden
bottom layer stays laid out, so its regions still report, and a press can
reach a hidden control only with the pointer over the stage, where the rules
show it. The pointer-driven `:hover` is Blitz's, and a pointer that leaves
the window unhovers the chain, so the controls hide. Auto-hide never hides
the top layer: the chips and the badge show at all times.

The control rows scale with the artwork's height (design D3). Container
queries are unimplemented in Blitz 0.3.0-beta.4 — `query_container_size`
returns a default size — so `cq*` units do not resolve, and the window
measures instead. The panel's effect reads the stage's mounted rect,
computes `scale = min(1.0, (stage_height / 3.0) / BOTTOM_FULL_HEIGHT_PX)`,
and holds it in a window `Signal<f32>`; this is layout, not interaction
state. `BOTTOM_FULL_HEIGHT_PX` is the bottom layer's full height at scale
1.0 — the 104 px the design names (`gui-queue-panel-redesign` design D3):
the top pad, the 14 px seek row, the gap, the 44 px transport row, and the
bottom pad. The bottom layer sets `--player-scale` from the scale, and its
sizes — the button boxes, the glyph and font sizes, the gaps, the pads, and
the seek row's height — are `calc(<px> * var(--player-scale))`, so the
controls take no more than one third of the artwork's height at every
height, with no floor.
The stage height depends only on the panel width and the image aspect, never
on the overlay, so there is no feedback loop; the scrubber's pointer math
reads its own measured rect, so it stays correct at any scale.

The auto-hide preference is core-owned (design D5). `App` holds
`gui_player_auto_hide`, read from the prefs key `gui_player_auto_hide` with
`false` as the default and saved back through `save_prefs`, so the choice
persists across launches. The control that flips it is the player panel's
right-click menu (gui-player-overlay-menu). A right-click on the panel's
artwork or controls sends
`PlaybackRequest::OpenPlayerMenu` with the pointer's whole-pixel anchor, the
core opens its `PlayerMenu` overlay — the cursor over the one auto-hide entry
plus the anchor — and the frame projects it. The panel paints the card in
the shared menu-card face at the anchor, and it marks the entry in its
overlay, which merges after the panel's own regions, so the card wins the hit
map where it paints over the panel. A press on the entry sends
`PlaybackRequest::PlayerMenuSelect`,
which runs the same execution the menu's Enter applies — the shared toggle
that flips the field and saves the prefs — and closes the menu with the
choice. The overlay's list key rules own the keyboard while it is open: Esc
closes it with no toggle, and the core's outside-press rule closes it on a
press elsewhere. The GUI-only fact rides `GuiQueuePlayback.auto_hide` — the
menu entry's checked mark reads it — and stays out of the shared
`QueuePlaybackProjection`; no key chord rides with it.

The seekbar paints the played fraction and the thumb,
all derived from the playhead anchor (below); it carries no chapter ticks
and no mark segments, and its clocks are the transport row's. The data the
old bar read is deleted, not idle (design D5): the chapter, media-segment,
and trickplay fetches are gone — the `EmbyClient` methods
`get_item_chapters`, `get_media_segments`, `get_trickplay`, and
`get_trickplay_tile`, the `App::playback_metadata` cache and the
`playback_trickplay` sheet store that served them, the frame fields they
filled, the `mbv-art` scheme answer for the sheet bytes, and the
`EmbyChapter`, `EmbyMediaSegment`, and `EmbyTrickplayGrid` model types.
The hover state and its time tip come from the core's `hover` section, not
from `:hover`, so the track grows with the tip and the state survives a
drag. A press on the bar seeks through the core.

The window predicts the playhead between the `playhead` section's anchors
(design D5 of `gui-change-driven-sections`). The core sends the anchor —
`GuiPlayhead`: the active fact, the active slot, the position, runtime
in ticks, and the paused fact — only on a changed fact or a new sample that
drifted past the one-second position tolerance, and it carries no timestamp, because the
core and the window share no clock. The window stamps each anchor
with its own monotonic clock at receipt: `apply_batch` (`src/window/app.rs`)
calls `Playhead::stamp_if_new` (`src/window/playhead.rs`), which reads the clock
through the `Times` seam. The window derives from that stamp until the next
anchor: the predicted position (the anchor's position advanced by the elapsed monotonic time while playing, held while paused, clamped into
the runtime), the played fraction, the elapsed and remaining clocks
(`format_duration_short`, the port of the TUI's shared duration formatter),
and the active item's finish epoch. The frame clock advances the prediction
only while the anchor is active and not paused (see *Clocks* above). The
`minute` counter (`Playhead::minute`) moves the finish times while playback is
paused, with no update from the core: one timer re-arms at each wall-clock
minute. The panels read the derived signals: the seekbar's fill, the transport's elapsed and remaining clocks, the rail's
ring and remaining time, the compact
strip's progress line, the folded line's finish
clock — the wall clock at the summary's `tail_secs` past the active item's
end, with the ` +1d` suffix when the finish lands past the local day; the
rule `queue_finish_clock` runs in Rust for the TUI — the active
queue row's progress, and the Audiobooks Workspace's current chapter: the
chapter row whose `startTicks <= position < endTicks` span holds the
predicted position while the Hero's book is the frame's `playing_book_id`;
the span's end is exclusive, and an empty span — an audio-part row's `0`/`0`
— never holds one.

What stays in Rust are the facts that change only on a discontinuity: the
end offsets themselves (`App::gui_queue_end_offsets` over
`queue_end_offsets` in `src/app/queue_summary_projection.rs`), the Music
playing track id and the playing book id (a mark source, design D3), and
the scrubber hover's `time_label` (the pointer drives it, not the clock).
`GuiQueuePlayback` carries no playhead facts: its `playback`,
`elapsed_clock`, `remaining_clock`, and `finish_clock` fields are gone, and
the queue row DTO carries no NowPlaying progress — the window reads the
playhead's played fraction for the row whose slot is the anchor's active
slot. mbv has no playback-rate fact, so the anchor has no rate and a rate
change re-anchors only through the drift rule. The bottom layer's two rows
split the controls (`gui-queue-panel-redesign` design D3): the seek row is
the scrubber alone, and the whole row is the `seekbar` region; the
transport row holds play/pause, previous, back 10 s, forward 30 s, and
next at its left and the elapsed and `−remaining` clocks at its right, all
as solid Lucide shapes with the availability that the colour carries. The top layer's
chips line holds the video stream's resolved label, and the audio and
subtitle chips, each opening its `trackPopover` overlay with the choices and
the chosen one marked. The control rows paint while the title shows, and the
stopped state paints them without one (below).

The stage and the no-image column are the panel's two arrangements (design
D6). When the active item's art is `Placeholder` or absent, the panel paints
no artwork box at all: under the header it shows the chips line, then the
control rows at scale 1.0 — always visible, whatever the auto-hide setting —
then the title. The panel's right-click menu still carries the auto-hide
entry and its setting. The stopped state (design D7) shows the control rows
without a title: while a shown local queue holds slots and nothing plays,
the frame's `can_start` fact is true while `now_playing_title` is `None`,
and the panel paints, under the header, only the control rows at scale 1.0 —
no stage, no chips line, no title — with the scrubber's empty track, which
resolves no seek while nothing plays. A shown peer queue and an empty shown
local queue leave `can_start` false, so no control rows paint.

The Queue panel's recessed box sits below the player block on the
`queuePanel` fill, with its header row, its rows, and its bottom spacer. The
header is a 48 px bar on `--ef-bg1` with a 1 px `--ef-bg3` bottom border
(`--canvas-queue-header-height`), padding `0 12px`, and gap 4 px
(`gui-queue-panel-redesign` design D7). Its controls paint in the design's
order and nothing else: the `QUEUE` label — an 11 px letter-spaced small
caption — the switcher, the new-queue control, the close control, a flex
spacer, and the filter control at the trailing edge; no remaining count and
no queue finish clock ride the bar (the folded line keeps both). The
switcher is always a button, with no plain-title branch and no two-entry
gate: it carries the list glyph, the shown entry's name — `Queue` while the
name is empty — the shown queue's slot count in a mono count chip, and a
chevron that points up while the chooser's dropdown is open. The count is
`GuiQueueHeaderFacts.shown_len`, the shown queue's whole slot count; the
filter narrows the painted rows, never the count. The icon buttons are
32×32 with the muted glyph; a hover raises the fill and brightens the
glyph, the enabled close control hovers the destructive fill and glyph, and
the disabled close control keeps its opacity and claims no pointer. The
header's controls and each row report their own region: the switcher, the
filter, the new control, and the close control report their
`queueChooser`, `filter`, `newQueue`, and `closeQueue` targets.

### The Queue chooser

The switcher opens the Queue chooser
(`src/window/components/queue_chooser_menu.rs`), a dropdown card the header
paints from the switcher's own anchor box: it hangs 4 px under the
switcher, left-aligned on the switcher's leading edge, and paints over the
queue rows below. The card is the design's own chrome
(`gui-queue-panel-redesign` design D8): min-width 260 px on `--ef-bg2` with
a 1 px `--ef-bg4` border, radius 8, 4 px padding, and items at the queue
rows' height and gap (40 px, 0 px apart) that take the raised fill on the
chooser's cursor and under the pointer. Its entries
are the local Owner's Named queues in queue order, then one entry for the
connected peer Player owner, named by the peer's own name (its device name,
or the route or endpoint it connects by). Each entry paints, in order: the
check (`--ef-green`) on the shown entry, the name, then at the trailing end
the playing mark (`AudioLines` in `--ef-aqua`) or the current queue — the
one that holds the playhead — as an accent dot, or the direct-peer brand
mark, so a peer is never read as a local queue; last, the queue's slot
count in mono `--ef-grey2`, so a count never reads as a mark. The count
rides the projection (`QueueChooserEntryProjection.len` and
`GuiQueueChooserEntry.len`): the shown entry's is its own live view's slot
count, the current queue's is its view's, any other resting queue's is the
`NamedQueueEntry.len` the Owner keeps fresh, and the peer's is its remote
view's slot count.

Choosing an entry — a press on its row, or Enter on the chooser's cursor —
shows it and starts, stops, and moves no playback; a press sends
`PlaybackRequest::QueueChooserEntry` with the entry's own identity (the
local queue's `NamedQueueId`, or none for the peer), never a painted row.
While the chooser is open it owns the keyboard: the arrow keys move its
wrapping cursor, Enter shows the entry under the cursor, `r` opens the name
of the local queue under the cursor for edit, and `t` or Esc closes it. A
rename edits inline, in place of the entry's label and with the filter's
block caret; Enter confirms it as `NamedQueueRename` and Esc cancels the
edit and keeps the old name, with the chooser still open. A peer entry
cannot be renamed.

The chooser rides the `overlay` section as the `GuiOverlayFrame::QueueChooser`
variant: the core's cursor, the entries, and the rename draft while one is
open. Its entry rows are the panel's second overlay, marked after the row menu's
overlay, naming each painted entry's own identity, so the dropdown wins the
hit map over the queue rows beneath it.

The shown entry decides which queue the panel shows and which owner the
player panel follows (ADR 0041). While a local Named queue is shown, the
player panel shows and controls the local Owner's playback — including
while that queue rests — and while the peer entry is shown, it shows and
controls the peer's. The transport controls the current queue of the owner
it shows, whichever of that owner's queues is shown. Every queue edit,
enqueue, play-next, and whole-queue replacement from the window reaches the
shown entry's queue, and a Play with nothing playing starts the shown
queue. Showing an entry clears the Queue filter and the marks and puts the
shown queue's start slot on the Queue's active row, or its first row when
it has no start slot.

The `t` chord opens and closes the chooser, `Ctrl+T` runs the new-queue
control, and `Ctrl+W` runs the close control. The three chords are GUI-only
key-context bindings (`src/app/input/key_policy.rs`) gated on the Queue
list box pointer target: they run only while the pointer target is the
Queue list box, and the TUI Client's bindings do not change. The new-queue
control asks the local Owner for a new Named queue; the Owner answers with
the new queue's own snapshot, the panel shows it, and its given name opens
for edit in the chooser. The close control acts on the shown entry: a peer
entry disconnects and the peer keeps playing; a local queue closes, with a
confirmation only on the playing current queue, because closing it stops
playback; and after a close the panel shows the current local queue.

The item rows carry no zebra stripe and no leading accent edge
(`gui-queue-panel-redesign` design D9): the row's look comes from its
state, and the hover veil stays the shared `button` look. Each row paints
the design's delineated row (`gui-delineated-rows` design D3): its
right-aligned two-digit mono ordinal in a 28 px column, one title line, and
the item's duration in a 96 px right column in every row state, at the
fixed 40 logical pixel height with no gap between rows, 10 px padding, and
a 1 px `--canvas-border` divider under each row. The rows are square, the
now-playing row included (user request 2026-11-20). A row with
progress — a partial row, and the now-playing row, whose ring follows the
playhead — carries the progress ring in its right column, directly before
the duration, so the ring and the duration read as one cluster (user
request 2026-11-23): a 14 px svg in the
collapsed rail's two-circle form, the track in `--ef-bg2` and the played arc
in `--ef-orange`, or `--ef-green` on the now-playing row, each circle
naming `currentColor` with its token in its own computed `color`, because
usvg drops a `var()` in a `stroke` attribute. The ring is the row's only
progress mark and adds no height, no row paints a start mark, and no row
paints a time left. The rows paint three fills, and the three are distinct:
the now-playing row fills
`--ef-bg-green` with its green 600-weight title, a look no hover
displaces; every marked row fills `--ef-bg3`, kept under the
pointer too; a hovered row that is neither fills `--ef-bg0`; and an
ordinary row paints no fill of its own. The frame's `cursor` slot paints
nothing: only a multi-selection shows as selected, and the hover names the
row a right-click acts on. A watched row paints `--ef-grey1`
with the `--ef-bg5` strike, and a hovered watched row keeps the primary
title. The looks need no focus fact, because the window holds none (ADR
0040). The list paints no scrollbar column yet — the TUI
reserves one only while the list is focused and overflows.

The row hover and the drag paint from the frame, and the window holds
neither. `GuiQueue.hover` names the row the pointer rests on: that row paints
the hover fill and nothing else (user request 2026-11-23 retired the hover
chrome — the six-dot grip and the actions button). The `rowMenu` overlay
opens on a right-click over the row and
on the context-menu chord, and its
entries — Play now, Play next, Mark watched, Go to episode, Remove
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
slot. The drag begins on the first pointer motion after a press on a row —
anywhere in the row, with no grip (user request 2026-11-23) — and a press
with no motion arms none, so a plain click paints no drag chrome.
`GuiQueue.drag` carries the
dragged row's content as the ghost, the final index the drop leaves it at,
and the insertion line's stable-slot anchor; the ghost floats at that line
— with no ordinal and no right column, painted with the dragged row's ring
and percent — and the dragged row keeps its in-place paint (the canvas keeps
it lit). The core owns the
gesture and the drop index, and it sends
one move request to the Owner on release. `Alt+J` and `Alt+K` ask the core
for the same move on the active row. A `cancel` pointer kind
drops the drag, the row hover, and the scrubber hover without a move.

The shared confirmation modal (`src/window/components/confirm_overlay.rs`) paints window-modal
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

The daemon-lost recovery dialog (`src/window/components/daemon_lost_overlay.rs`) paints in that
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

The footer line paints below the box between gap rows on the design's
footer fill (`--ef-bg0` with a 1 px `--ef-bg3` top border,
`gui-queue-panel-redesign` design D2). Its left group leads with the
collapse control the player panel's header used to paint
(`move-playback-header-into-queue-footer` D1, D2), at the band's 28 px control
height with a muted `ChevronsRight` glyph — the inverse of the collapsed rail's
`ChevronsLeft` expand control. After it follow the autosave indicator (while
save-on-consume is on for a saved playlist), the queue's source text with no
leading icon, and the dirty mark beside the source it describes; its centre
carries the armed `PREFIX` badge; and its right group carries the shown queue's
totals — the marked rows' total, named `Sel`, while rows are marked, beside the
queue total, each with a `+` while its sum carries an unknown runtime. The
filter narrows no total. The footer names no Player owner and paints no
liveness dot: the shown queue's owner reads from the playback target in the
Queue header. The playback target control leads the Queue header, to the left
of the Queue switcher (`queue_target_control.rs`, user request 2026-11-20): its
glyph is the one the chooser paints for the attached choice (the local
monitor, or the Cast, Emby, or direct-peer mark), never a fixed Cast glyph. Its
press opens the target chooser as a popover hung under the control, in the
control's overlay, which merges after its own regions, so a press on a
popover row wins over the queue row beneath it (D4); the collapse control's
press runs the same toggle as the `z` chord.
No surface paints the now-playing status word (D5): the footer carries no
status fact, and the `playback` section no longer carries one.

## The Status bar in the footer line

`GuiStatusBar` carries the prefix-armed state from the shared
`StatusBarProjection`. The Queue column's canvas has no separate status bar:
`src/window/components/queue_footer.rs` paints it in the footer line. No
status line shows toasts: they show as floating cards from the `Toasts`
section (`gui-toasts`), painted by `src/window/components/toast_stack.rs` at
the bottom-right of the window, over every panel, the Settings screen, and
the modal overlays, with no layout change and no pointer input. The line
reports no regions: the source text paints but is no press target (the GUI
presents no playlists surface), and a point anywhere on the line resolves to
nothing. While the Library panel shows, the Library Status bar paints the
same armed state (`move-playback-header-`
`into-queue-footer` D9): the armed prefix paints at its
left edge and the location stays flush right; it paints no toast. The bar
paints no key hints. The location names the destination, `All` or the checked
collections' names, and the picked item, joined by the canvas's separator.
No Search field paints here (user request 2026-11-19): the top bar's Search
control paints it at the window's top edge in every screen but Settings, so
the field and its `Ctrl+K` arm are not bound to the Library panel or its
status bar.

## The Library panel

The GUI paints a Library panel in the column at the left window edge,
beside the Queue panel in the column at the right window edge
(`src/window/components/library_panel.rs`). It follows the design canvas's Library artboard, not
the TUI: the GUI does not keep parity with the TUI, and the TUI does not change
(ADR 0036). The panel's interaction state lives in the window content, and the
Client holds the data selection and the actions (design D1): the shared core
state on `App`, the GUI's own stores on `GuiClientState`.

| Window content (`src/window/library/`) | Client (`App` / `GuiCore`) |
| --- | --- |
| Hover (the pointer target), scroll, open tree nodes | The active destination (`App::tab`) |
| Filter text, the collection dropdown's open state and filter text, and Search query text | The selected collections of each destination |
| The open Hero overlay, item menu, and row menu | The album each collection last showed in the Hero (`GuiClientState::library_picks`) |
| The Library's keys, sent to their pointer target | Service data, its loading, and the artwork |
| No cursor and no focus inside the panel | Panel focus (`FocusStack`) and the actions |

The data selection is core state because the core decides what to load and
what to save at exit. The window content changes it only by a command, and the
frame carries it back, so the collection dropdown paints the core's set. The
panel holds no cursor and no focus inside it: its keys go to the *Pointer
target*, not by Panel focus (ADR 0040). The panel uses
no TUI component, no `mbv-surfaces` Surface state, no `GuiSurfaceLeaf`, and no
`[keys]` action, and its pointer input never reaches the core's gesture
recogniser or hit map. The window's global header (`move-playback-header-`
`into-queue-footer` D7) is `LibraryTopBar` (`src/window/components/`
`library_top_bar.rs`): the root mounts it in `src/window/app.rs`, above the
panel row and across both columns or the collapsed rail, in every
arrangement; the Settings screen mounts none. It paints the Home icon and
one tab per presented Service library destination — its Home icon opens
Home — and the Settings control at its right end; it carries the Library
pointer zone, so a key pressed over it routes as over the Library column.
The Search control sits at its right end, before the Settings control
(`src/window/components/library_search_control.rs`, user request
2026-11-19): the bar's measure decides the form — wide enough it paints the
field inline with its `Ctrl K` hint; too narrow it paints the Search icon
whose press opens a dropdown panel that carries the same field, hint, and
results, which open downward. `src/window/components/library_panel.rs` paints the destination's list (`LibraryList` with
`LibraryLetterRail` in Music; `LibraryHomeList` in Home;
`LibraryShowsList` in Shows; and `LibraryLetteredList` in Emby, Audiobooks,
Podcasts, and Feeds),
the destination's Hero (`LibraryHero` with `LibraryTracklist` in Music;
`LibraryHomeHero`, `LibraryShowsHero` with its Workspace,
`LibraryEmbyHero`, `LibraryAudiobooksHero`, `LibraryPodcastsHero`, and
`LibraryFeedsHero`), and `LibraryStatusBar`. The panel paints no selector
row and no pills region between `LibraryTopBar` and the list. The lettered
list is one component for the four destinations (`gui-tech-debt-simplification`
design D4): the `LetteredDestination` trait (`src/window/library/lettered.rs`)
names the parts a lettered list needs, and four zero-sized adapters
(`EmbyList`, `AudiobooksList`, `PodcastsList`, and `FeedsList` in
`src/window/library/lettered/adapters.rs`) fill them with each destination's
carried row, its list and detail parts, its attribute word, its art scope, its
pointer list, its header facts, its notice texts, and its row painter. The row
model, the `LibraryState` filter, scroll, measure, and letter-jump methods, and
the list's body over the shared column scaffold are then one generic component,
`LibraryLetteredList<D>`
(`src/window/components/library_lettered_list.rs`), and each destination keeps
only its data; `play_next` defaults to no play-next, and only Emby overrides it.
Home and Shows keep their own list modules (`src/window/library/home_list.rs`,
`src/window/library/shows_list.rs`): Home's rows carry headings and spacers,
and Shows' rows carry the New and Upcoming modes. Every list's column wiring
goes through one scaffold, `library_list_column`
(`src/window/components/library_list_column.rs`), beside the scroll shell
`library_virtual_list`: the column's own hook, the header block, the notice
lines, the shell's measure, scroll, and rows-in-view reports, and the row that
holds the shell beside the letter rail. Each list keeps its own row model, its
header facts, its notice texts, and its row painter, so the scaffold shares the
wiring, not the list (`gui-tech-debt-simplification` design D14). Each list
paints its own header: `LibraryListHeader` paints the heading line — the
destination's Service mark, or Home's text title alone, the dropdown where
one applies, and the sort control where one applies — on that line. Music,
Podcasts, Feeds, and an Emby feed-group library paint
`LibraryCollectionDropdown` with the collection frame; Shows paints its
single-choice view dropdown over the same prop-driven presentation (no
filter strip). A plain Emby, Audiobooks, or Home destination paints no
dropdown. Every flat list that carries a sort frame paints
`LibrarySortControl` last on that line (gui-flat-list-sort design D7): one
single-choice segmented control over the frame's choices and its active
sort, pointer-only and binding no key. The panel also
paints the right-click item menu (`QueueRowMenu` under the panel), which a
right-press on a list row or a
Hero tracklist row opens through the `rowMenu` command: the command names
the row's destination identity (`GuiLibraryMenuRow` — a destination row's
stable Service id, or a Home row's `GuiHomeRowId`), the frame's `rowMenu`
section carries the shared builder's full entries for that row, and the menu
paints at the pointer position. Every destination row carries the same
standard action set its TUI list menu carries, because the App builds it
from the same shared builders (`build_context_menu_for` for an Emby
library's items, `build_explicit_item_context_menu` and
`build_continue_watching_context_menu` for the Home rows,
`build_audiobookshelf_menu_entries`, and `build_feeds_menu_entries`):
Music, Shows, the plain Emby libraries, Audiobooks, Podcasts, Feeds, and
both Home sections. A press outside the menu, a scroll, or Esc closes it,
and a chosen entry sends the same `menuEntry` command the Hero's More
actions sends, with the row the frame carried, so the entry runs against
the row the user saw. The Hero's More actions paints the shared builder's entries minus
the ones its own buttons already send (`Play`, `Play All`, `Add to
Queue`), under each entry's carried builder index, and paints no control at
all when the remainder is empty — the live finding that a Music album's
menu repeated the buttons and offered nothing new.

The collection dropdown is one component for every destination that has
collections (gui-collection-dropdown design D6): Music, Podcasts, Feeds, and
an Emby library in feed-group mode paint it, and no other destination does.
Shows paints a separate single-choice view dropdown — All, New, Upcoming —
through the same prop-driven dropdown presentation; it is not a collection
filter and adds no key. Its closed state is one trigger that
names the selection — `All`, the one checked collection's name, or the count
with the destination's noun (`2 folders`) — with a `Clear` control while the
selection is not All. A press on `Clear` selects All without opening
anything; a press elsewhere on the trigger opens or closes the popover,
which paints over the list with the row menu's card chrome: a filter field,
the All row, and one
checkbox row per collection in catalog order. A check leaves the popover open.
A press outside it, a destination change, or Esc closes it and clears the
filter text. The window owns the open state and the filter text (design D7);
the core owns the checked set it paints back.

### The Library sections and their revisions

The Library panel reads three window sections, and each one has its own
revision: `libraryList` (`GuiLibraryList`, `src/bridge/frame/library/list.rs`),
`libraryDetail` (`GuiLibraryDetail`, `src/bridge/frame/library/detail.rs`), and
`rowMenu` (`GuiLibraryRowMenuFrame`, `src/bridge/frame/library.rs`). A Library
list can hold thousands of rows, so each section carries its revision, and its
value rides a batch only when that revision changes, as the visual slot's
artwork does. The ledger compares each slot on its own. A pick therefore sends
only `libraryDetail`, and a row-menu open sends only `rowMenu`. Each value is an
`Option`, so a `null` clears its section. A window restart calls `sync`,
which releases every section the window holds, so the next batch re-sends each
present Library section at its current revision.

One gated Library pass (`src/sections/library.rs`) builds all three sections.
The `library` mark (design D3) names the pass. The pass makes one projection
call per destination, so the three slots share one build, and each slot keeps
its own compare. A build that matches the slot's last built value sends nothing.

The frame path owns the destination's load order. The `GuiDestinationLoads`
trait (`src/app/gui_library/ensure.rs`) gives every destination the same three
steps in one fixed order: the root list, then the selection, then the
dependent content. A destination with no work for a step keeps the trait's
default no-op; the driver's exhaustive match on the destination kind makes a
new destination name its site. The driver,
`App::ensure_gui_active_destination`, runs the steps for the active
destination. Two callers reach it: `build` in
`crates/mbv-gui/src/sections/library.rs`, which runs the pass before the parts
are built, and the destination's open step (`open_gui_active_destination` in
`src/app/gui_library.rs`), which runs its user-action work first and then the
driver. So every presented Library destination loads from the frame with no
click.

The window keeps one signal per section: `libraryList`, `libraryDetail`, and
`rowMenu` in `src/window/sections.rs`. The four large sections (`library_list`,
`library_detail`, `queue`, and `settings`) are held as `SectionRef` — a shared
`Rc` handle — so a section update costs one copy of its content.
`Sections::apply_batch` writes a signal only
when the batch carries a revision the window does not hold. `src/window/components/library_panel.rs`
narrows the list and the detail through `library_pair`
(`src/window/library/parts.rs`). The pair narrows only when both `kind` tags
match, so a mismatched pair paints nothing.

`search` is the Search section (design D12, row 12.1):
`Option<GuiSearchFrame>`, present while the top bar's Search control
field — the field the `Ctrl+K` chord arms — holds a query or its rows, and `None`
while it is closed. It rides its own section beside the
Library sections, never inside them, because Search is
cross-destination: a result that lands between two content changes still
reaches the window. `GuiSearchFrame`
(`src/bridge/frame/library/search.rs`) carries `query` (the query the core is
answering), `loading`, `failed`, and `results`, whose rows carry `id` and
`item_type` (the stable Service identity the `searchActivate` command sends
back), `title`, and `subtitle` (empty when the item has none).

`libraryList` (`GuiLibraryList`) carries `tabs` (one tab per Service library
destination, plus the feed tab while subscriptions are configured, in catalog
order, each a `GuiLibraryTabFrame` with a `position` and a `label`; Home is not
a tab, and the top bar's Home icon opens position 0), `currentTab` (the active
tab's position; `null` for Home and for an active destination that no tab
names), and `destination` (`GuiDestinationList`), the active destination's list
part. `destination` is tagged by `kind`: `music`, `home`, `shows`, `emby`,
`audiobooks`, `podcasts`, or `feeds`. Only one destination is active at a time,
and the top bar's Home icon marks itself current exactly while `destination` is
`home`.

`libraryDetail` (`GuiLibraryDetail`) carries the active destination's detail
part, tagged by the same seven `kind` values: the core's selection, the Hero,
and the Workspace facts. `rowMenu` (`GuiLibraryRowMenuFrame`) carries `row`
(the `GuiLibraryMenuRow` the command named) and the shared builder's
`entries` while a row menu is open. Each destination
keeps its list and detail parts in one file under `src/bridge/frame/library/`;
`music.rs`, for example, defines `GuiMusicList` and `GuiMusicDetail`.

`GuiHomeList` carries `loading` and `rows`: Home's flat list in the Music list
slot, with no dropdown, no letter rail, and no Workspace. `rows` are the two
sections' heading rows and item rows in paint order — `Continue watching`
first, then `Recently added` — with a heading only above its rows, an empty
section omitted, and the spacer only between two non-empty sections, the
TUI's flat list's rules. The former `Latest episodes` and `Upcoming` sections
are gone: those lists paint in the Shows destination (gui-collection-dropdown
D9). An item row carries:
`id` (`GuiHomeRowIdFrame`), the stable identity `App::home_stable_target`
resolves — a `Continue` tag with the row's Emby item id, or a `Recent` tag
with the queue item's content id (`provider`/`value`, never a position);
`title` and `subtitle`, the split title parts the TUI's flat list paints;
`date`, the gutter date as the shared projection's `day month` text
(`fmt_publish_date_short` over the provider timestamp; `None` on a Continue
row and on an undated item); `kind` (`collection` when the row targets a
container the play path refuses, `media` on a playable leaf, mirroring the
shared flat-list vocabulary); `state` (`ordinary`, `played`, or
`{ active: { progress } }` with the Continue Watching resume percent);
and `art` (`GuiArtRef | null`, the row's card art for Emby items only —
the Audiobookshelf and Feed rows paint without art in Phase 2, and the
window paints its own placeholder while `null`). `GuiHomeDetail`'s `selected`
carries the core's selected row the select command last named — the first item row is
the Hero default, so the Hero paints as soon as a section lands; a stale
frame's selection resolves on its own rows or falls back without a hero.
`GuiHomeDetail`'s `hero` carries `title`, `meta_rows` with `duration_row` and
`progress_row`,
`overview`, `art_shape` (`landscape`, `square`, or `portrait`, the artwork
policy's declared arm), and `art` (the artwork policy chain's display
encode for Emby items only, again `null` on the rows without art) — the
shared hero producers' facts, the same facts the TUI paints for the same
item.

`GuiShowsList` (`src/bridge/frame/library/shows.rs`) carries `lib_idx` (the
Shows library index the Shows commands name), `view` (the single-choice view
frame: the active All, New, or Upcoming choice; the window paints the three
labelled entries), `sort` (`Option<GuiListSortFrame>`, the New view's sort
frame and `None` under All and Upcoming; gui-flat-list-sort designs D2, D7),
`loading`, `series`, `upcoming_failed`, and `rows`
(gui-collection-dropdown D8, D9).
`GuiShowsDetail` carries `workspace_loading`, `selected`, and `hero` (design
D14). All paints the series tree, its letter rail, and its Workspace. New
paints the merged latest-episodes rows in the active sort — Newest by
default, and under Title by show name, then season, then episode, over rows
that carry `sort_letter` and so paint headings and the rail; Upcoming paints
the merged premiere rows soonest first and takes no select. Both flat views
reuse the old Home row types and data sources; there is no collection
dropdown and no collection target for Shows. The TUI's Latest and Upcoming
modes stay TUI-only.

The list is the Music tree over the TUI's TV node kinds, and
`GuiShowsList` carries it as `series`: each node holds `id`, `title`, `sort_letter`,
`expandable`, and its `seasons`; each season holds `id`, `name`, and
`episodes`, which are `null` until that season's load lands, so an open
season asks the core for them. The window content owns the open and closed
node keys, exactly as the Music tree's does, and builds the painted rows at
the Music heights: series and seasons on the node row, episodes on the track
row, the letter heading above the first series of each letter, and the `Series` header with no selection label and the
`/` filter field. A series whose detail has not landed paints closed and
expandable. `GuiShowsDetail`'s `selected` carries the core's selected series
as the frame painted it — the series id — with the first node as the default,
so the Hero paints as soon as a list lands, and a stored id the list no
longer carries falls back to that first node. The `hero` is present exactly
while a series is selected; the driver's Shows steps run in order — the root
load, then the default series' detail, then the selected season's episodes —
so the Workspace paints without a season press.

That hero carries the TUI series producer's facts —
`hero_content_series_with_episode`'s genre, year range (else the release
date), season count, and network rows, no duration row while the Workspace
has no focus, and the artwork policy's landscape arm — plus `workspace`: the
detail's seasons (`season_id`, `name`, `selected`, painted as a dropdown), `selected_season`
(the stored choice while the detail still holds it, else the first season,
because the pills start on the first season and are never kept across a
series change), and that season's `episodes`. The Workspace-focus overlay
that swaps the facts for the selected episode's is recorded in D14 as a
follow-up and is not carried. A Workspace episode row carries `episode_id`,
`number`, `name`, `duration`, and `state` (the shared row state the Home rows
carry). The landscape arm paints the still
at the Hero content width at 16:9 above the facts, as the TUI does; the
portrait and square arms keep the canvas's 300 px box beside the text. A
click or a double-click on a Workspace episode sends that episode's
`episode_id` with `showsPlay`, and a right-press on a tree row opens the item
menu at that row's stable Service id.

`GuiEmbyList` (`src/bridge/frame/library/emby.rs`) carries `lib_idx` (the
plain Emby-library index the Emby commands name), `collections` (the
destination's collection dropdown frame; `Some` in the feed-group mode with
All, every root folder in listing order, and the Folders noun; `None` in plain
mode, whose letter rail replaces it, design D6), `collection_names` (the
checked folders' names the Library Status bar's location reads; empty in plain
mode, whose list is the whole library), `feed_group`, `sort`
(`Option<GuiListSortFrame>`: the list's sort frame with the target, the
active sort, and its choices; gui-flat-list-sort designs D2, D7), `loading`,
`failed`, and `rows`. `GuiEmbyDetail` carries `selected` and `hero`. A plain
Emby library (row 8.1: every Emby library that is neither a Grouped Music
library nor a TV library) paints the plain mode: `feed_group` is `false`,
the rows are the root-list store's full list in the server's `SortName`
order, each with `sort_letter` over the article-stripped sort name (`#`
past Z), the year or the runtime on `subtitle`, and no `date` — the letter
rail replaces the TUI's letter-range pills, so no collection dropdown
rides. A date sort orders the same rows by the date added and carries no
`sort_letter` at all (design D6). A library in the feed-group mode
(`feed_view_libraries`) paints `feed_group` `true` with a collection dropdown
over All and each root folder whose video count is positive, in listing order
(design D6). Its rows are the checked folders' items, or every folder's while
All is selected, in the list's active sort — Newest by default, Newest first
and Oldest last with undated rows last in both — with the production year in
`date`, and `sort_letter` set exactly while the sort compares a name (design
D6). In both modes the detail's `selected` carries the core's selected row as
the frame painted it, with the first row as the default, so
the Hero paints as soon as a list lands, and a stored id the list no longer
carries falls back to that first row; a plain mode's fetch that failed
carries `failed` and shows no row until the library opens again. The detail's `hero` is
the selected row's shared Emby hero producer's facts — `title`, `meta_rows`
with `duration_row` and `progress_row`, `overview`, `art_shape` at the
artwork policy's declared arm, and `art` — and carries no Workspace.

`GuiAudiobooksList` (`src/bridge/frame/library/audiobooks.rs`) carries
`lib_idx` (the Audiobooks library index the Audiobooks commands name),
`sort` (`Option<GuiListSortFrame>`: the list's sort frame over the book
catalog with the target, the active sort, and its choices Title, Author,
Newest, and Oldest; gui-flat-list-sort designs D2, D7), `loading`, `failed` (the browse state's catalog states), and
`rows`.
`GuiAudiobooksDetail` carries `workspace_loading` (the selected book's detail
fetch), `selected`, and `hero`. No collection dropdown rides here: the A-Z
letter rail replaces the TUI's eight
surname buckets, as it does for Shows and the plain Emby libraries. The rows
are the catalog's books in the list's active sort — Author by default, then
by title under Title, and by the added date under Newest and Oldest — each
with `id` (the book's stable `library_item_id`), `title`, `subtitle` (the
author display credit), `duration` (the book's total duration, the TUI row's
duration gutter), and `sort_letter`, which is present exactly while the sort
compares a name and takes the first letter of the key it compared (`#` for a
digit- or symbol-led key; gui-flat-list-sort design D6). The detail's `selected`
carries the core's selected book as the frame painted it, with the catalog's first book
as the default, so the Hero paints as soon as the catalog lands, and a stale
pick falls back to that first book.

The detail's `hero` is the selected book's Hero: the shared
`hero_content_abs_book`
producer's facts — `title`, `meta_rows` (the author, the duration, and the
`Finished` or `N%` progress row) with `duration_row` and `progress_row`, and
no narrator, series, or description — `art_shape` at the book artwork
policy's portrait arm, `art` (the cover through the Service-scoped book-cover
chain, `null` while the book declares no cover or no server is configured),
and `workspace`: the book's chapter rows in the TUI's `visible_rows` order.
A chapter row carries `chapter` (the stable Service discriminator the TUI's
chapter activation addresses — the chapter number, or the audio-part index
on a `Part N` row, never a display position), `title`, `duration`, and the
`start_ticks` and `end_ticks` span on the book's merged timeline. The window
marks the row whose span holds the playhead while this book plays. An
audio-part row carries no span, so it never lights.

`GuiPodcastsList` (`src/bridge/frame/library/podcasts.rs`) carries `lib_idx`
(the Podcasts library index the Podcasts commands name), `collections` (the
destination's collection dropdown frame: All, every listed show in title
order, and the Shows noun; design D6), `sort` (`Option<GuiListSortFrame>`: the
list's sort frame with the target, the active sort, and the choices Title,
Newest, and Oldest; gui-flat-list-sort designs D2, D7), `loading` (the show
catalog's pages or the active view's own episode fetches), `failed` (the
catalog's error), and `rows`. `GuiPodcastsDetail` carries `selected` and
`hero`. The letter rail rides
exactly while the sort compares a name: Title paints the headings and the
rail, and a date sort paints neither. The rows are the checked shows'
episodes: All merges every listed show's episodes, and a checked set merges
the checked shows', in the active sort's order — Newest by default, with
undated episodes last — exactly as the TUI's `visible_episodes` and
`visible_episodes_from` order them. A row carries `id`
— the episode's stable identity, the provider's `(library_item_id,
episode_id)` pair encoded as one opaque string
(`GuiPodcastEpisodeId::encode`: the show id's byte length, a `:`, then the
show id and the episode id; `decode` is its only reader) — `title` (the
episode), `subtitle` (the parent show's title), `date` (the `day month`
gutter, `null` on an undated episode), `state` (`ordinary`, `played`, or
`{ active: { progress } }`, the TUI podcast row's own arms), and
`sort_letter`, present exactly while the sort compares a name. The detail's
`selected` carries the core's picked episode as the frame painted it while the active
list still holds it, else that list's first row, so the Hero paints as soon
as a list lands; a checked show the published show list no longer carries
falls back to `All`.

The detail's `hero` is the selected episode's Hero: the shared
`hero_content_abs_episode`
producer's facts — `title` (the episode), `meta_rows` (the show title, the
duration, and the publish date) with `duration_row` and `progress_row` (the
producer fills no progress row for an episode) — the cleaned description as
`overview`, `art_shape` at the episode artwork policy's Square arm, and `art`:
the parent show's cover through the Service-scoped cover chain (`null` while
the show declares no cover or no server is configured). There is no
Workspace: a podcast Hero is episode information only.

`GuiFeedsList` (`src/bridge/frame/library/feeds.rs`) carries `collections`
(the destination's collection dropdown frame: All, every configured
subscription in the tab's order, and the Subscriptions noun), `sort`
(`Option<GuiListSortFrame>`: the list's sort frame with the target, the
active sort, and the choices Title, Newest, and Oldest; gui-flat-list-sort
designs D2, D7), `loading`, `failed`, and `rows`. `GuiFeedsDetail` carries
`selected` and `hero`. It carries no `libIdx`: a Client holds one feed tab,
so the Feeds commands name no library.
The letter rail rides exactly while the sort compares a name — a date sort
paints no heading and no rail — and there is no state filter.
`failed` is always `false`, because the feed tab keeps no failure state: a
failed subscription's fetch flashes the TUI's toast and keeps that
subscription's previous entries. The rows are the checked subscriptions'
entries: All merges every subscription's entries, and a checked subscription
holds that one subscription's, in the active sort's order — Newest by
default, with undated entries last — exactly as
`FeedTabState::rebuild_all_entries` orders them. A row
carries `id` — the entry's stable identity, its `(subscription url, guid)`
pair encoded as one opaque string (`GuiFeedEntryId::encode`: the URL's byte
length, a `:`, then the URL and the `guid`; `decode` is its only reader),
because two
subscriptions can publish the same `guid` — `title` (the entry), `subtitle`
(the subscription's name, `null` when the tab no longer holds it), `date`
(the `day month` gutter, `null` on an undated entry), and `state`
(`ordinary`, `played`, or `{ active: { progress } }`, the TUI feed row's own
arms), and `sort_letter`, present exactly while the sort compares a name. The detail's `selected` carries the core's picked entry as the frame painted
it while the active list still holds it, else that list's first row, so the Hero
paints as soon as a list lands; a checked subscription the tab no longer holds
falls back to `All`.

The detail's `hero` is the selected entry's Hero: the shared
`hero_content_feed`
producer's facts — `title` (the entry) and `meta_rows` (the subscription
name, then the duration when the entry declares one) with `duration_row` and
`progress_row` (the producer fills no progress row for an entry) — and
`art_shape` at the feed artwork policy's arm: Square for a podcast feed
(`FeedKind::Audio`), Landscape otherwise. No `overview`, no `art`, and no
Workspace ride here: a feed entry has no artwork source and no description,
so the Hero paints its placeholder in the reserved shape.

`GuiMusicList` carries `lib_idx` (the Grouped Music library index the
collection commands name), `collections` (the destination's collection
dropdown frame: All, every group in the group level's order, and the Groups
noun), `collection_names` (the checked groups' names the list
header and the Library Status bar's location read), `loading`, `failure`
(the failure line naming the selected groups whose album list failed), and
`artists` (`artist_key`, `name`, `sort_letter`, and `albums`).
`GuiMusicDetail` carries
`hero`, `art`, and `playing_track_id` (the playhead's Emby track id, from the
Queue panel's own authority, which the tracklist's playing row paints with a
play glyph beside the title, as the Queue's now-playing row does; the library
tree shows no playing marker). An album carries `album_id`,
`name`, `year`, and its `tracks` and `total` when the cache holds them; a
track carries `track_id`, `name`, `number`, and `duration`. `GuiMusicHeroFrame`
carries `artist`, `collections`, `album_count`, `artist_tracks`, the picked
`album`, and its `menu` entries with `label`, `key_hint`, and `index` (the
entry's index in the shared builder's order, so a window that paints only
some entries still executes the one it painted). The `rowMenu` section
(`GuiLibraryRowMenuFrame`) carries the row the `rowMenu` command named, with
the TUI menu builder's full entries for it, which the right-click item menu
paints. The Hero's art (`art`) is a `GuiArtRef`: the cached artwork as an `image`
URL, the bundled placeholder as a `placeholder` URL, or `Loading` with no URL.
The core never re-encodes artwork: the image fetch's 1280-pixel cap bounds the
cached bytes (`src/app/infra/image_fetch/protocol.rs`), the `mbv-art` scheme
answers them unchanged, and the window's `object-fit: cover` crops the Hero's
art. The window fetches no Library image: every load goes through the
`mbv-art` scheme to the core's image cache.

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

`library(GuiLibraryCommand)` is the fourth `CoreLink` method (`src/window/channel.rs`). It
only pushes `CoreInput::Library` into the core channel, as the other commands
do. `src/bridge/library.rs` translates the wire variants and
`GuiCore::apply_gui_library_command` (`src/app/gui_library.rs`) applies each
one
through the existing `App` action, so the TUI's own paths do not change. The
variants are `openDestination` (`position`), `collectionToggle` (`target`,
`key`), `collectionAll` (`target`), `listSort` (`target`, `sort`),
`showsSetView` (`libIdx`, `view`), `showDetail` (`itemId`), `openAlbum`
(`albumId`),
`play` (`itemId`), `playNext` (`itemId`), `addToQueue` (`itemId`),
`shuffleArtist` (`albumIds`), `menuEntry` (`row`, `index`), `rowMenu`
(`row`), `rowsInView` (`scope`, `ids`), and the Home commands — `homeSelect`, `homePlay`,
`homePlayNext`, and `homeQueue` (each carrying `row`, the
`GuiHomeRowId` the frame painted). Item
identities are stable Service ids, never row positions, so a command built
from a stale frame addresses the item that frame painted or nothing. A
`rowMenu` and its `menuEntry` carry the same `GuiLibraryMenuRow`: a
destination row's Service id, or a Home row's `GuiHomeRowId`, because a Home
row's menu resolves through the Home sources (the Continue Watching resume
path) rather than a destination store. The core resolves that row against
the active destination first and then every store, so an id two
destinations happen to share resolves to the list on screen. The
Home row id, not a Music item id, is
what `App::home_stable_target` resolves: `homePlay` runs the Home flat
list's play path (`App::home_play_target`, whose resume behaviour the row's
Continue Watching membership decides), and `homePlayNext` and `homeQueue`
both run its enqueue path (`App::home_enqueue_target`) — the Owner owns the
queue, so a Home command never mutates it directly, and Home's rows carry
no library the Music play-next admission resolves against. A stale frame's
row, one whose item the App's Home data no longer holds, logs and changes
nothing, as every item-named variant's stale arm does.

The `rowsInView` variant carries no action for an item: the window reports the
ids of the rows it paints, and the core stores the set for that scope, stamps
its navigation clock, and queues the in-view rows' row art once navigation is
idle (the Library row art flow above). It is the one Library command the window
sends with no press behind it.

The collection commands serve every destination that has a dropdown, and
their `target` is a `GuiCollectionTarget`: the Grouped Music library, a
Podcasts library, or a feed-group Emby library at its catalog index, or the
Feeds tab. `collectionToggle` checks or unchecks the collection whose stable
Service id `key` names, and a selection that ends with none checked falls
back to All, and `collectionAll` selects All. Each command stores the
selection and then starts the loads the new selection needs through the
destination's own loaders, so one dictionary of two commands serves all four
destinations. A `key` the destination's catalog no longer holds, or a target
that names no presented destination, logs and changes nothing.

The sort command `listSort` names the flat list by its `GuiSortTarget` — the
plain Emby library, Audiobooks, Podcasts, the Feeds tab, or the Shows New view
on a TV library — and the chosen `GuiListSort`, one of Title, Author, Newest,
or Oldest (gui-flat-list-sort design D5). The core stores it per destination
identity, re-projects the list, and ignores a target that names no presented
flat list, a Shows New target on a library that is not a TV library, and a
sort outside that target's choices. A press also returns the list's scroll to
its top and keeps the Hero's pick and the filter text (design D7).

The Shows view command `showsSetView` names the library by its `libIdx` and
one of All, New, or Upcoming. It is a view choice, not a collection command:
the core holds it in its own per-library map, saves it in the launch file's
`shows_view` field, and never routes it through `apply_collection`. A press
selects the choice and closes the dropdown; Upcoming rows take no select,
play, or enqueue, so the Hero keeps its item.

The Shows commands name a Shows library by its `libIdx` and its content by
the stable Service id the frame painted: `showsSelect`
(`libIdx`, `row`) selects the series the Hero shows, which also starts its
detail load, `showsSelectSeason` (`libIdx`, `seasonId`) moves the Workspace to
that season's episodes, and `showsPlay`, `showsPlayNext`, and `showsQueue`
(`libIdx`, `itemId`) run the shared library play, play-next, and enqueue
paths on an episode. The GUI Shows destination paints the series list alone
(design D8), so no view-mode command rides: the TUI keeps its own modes. The
Owner owns the queue, so no Shows command mutates
it directly, and a row, season, or episode id the active list no longer holds
logs and changes nothing.

The plain Emby-library commands name their library by its `libIdx` and its
content by the stable Service id the frame painted (row 8.1):
`embySelect` (`libIdx`, `row`) selects the row the Hero shows. The
feed-group mode's folder selection is the dropdown's own `collectionToggle`
and `collectionAll` commands over an `embyFeed` target, so no Emby-specific
feed-group command rides: the folder a press names is the stable root id
the frame carries, and a folder the root listing no longer holds logs and
changes nothing. `embyPlay` (`libIdx`, `itemId`) — the command Enter and a
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
by the window and does nothing for books. Every play and enqueue honours
`player.can_admit_audiobookshelf()`, and a refusal flashes the TUI's
owner-unavailable toast; the Owner owns the queue, so no Audiobooks command
mutates it directly, and a book or chapter id the Service data no longer
holds logs and changes nothing.

The Podcasts commands name their library by its `libIdx` and an episode by the
opaque row id the frame carried (row 10.1): `podcastsSelect` (`libIdx`, `row`)
selects the episode the Hero shows; the show selection is the dropdown's own
`collectionToggle` and `collectionAll` commands over a `podcasts` target, and
the list's order is `listSort` over the same target; `podcastsPlay`
(`libIdx`, `itemId`) — the command Enter and a double-click both send —
replaces the queue with the episode
through the TUI's episode play path; and `podcastsQueue` (`libIdx`, `itemId`)
appends it at the tail. There is no play-next command: `n` is consumed by the
window and does nothing for podcast episodes. Every play and enqueue
honours `player.can_admit_audiobookshelf()`, and a refusal flashes the TUI's
owner-unavailable toast; the Owner owns the queue, so no Podcasts command
mutates it directly. A row id of no valid shape, or an episode the per-show
cache no longer holds, logs and changes nothing.

The Feeds commands name an entry by the opaque row id the frame carried and
carry no library index (row 11.1): the subscription selection is the
dropdown's own `collectionToggle` and `collectionAll` commands over the
`feeds` target, the entry order is `listSort` over the same target, and a
subscription the tab no longer holds logs and changes nothing. `feedsSelect`
(`row`) selects the entry the Hero shows.
`feedsPlay` (`itemId`) — the command Enter and a double-click both send —
replaces the queue through the TUI's `play_feed_entry`, and `feedsQueue`
(`itemId`) appends through `enqueue_feed_entry`; both flash the TUI's
"Feed entry has no playable source" toast for an entry with no enclosure and
no link. There is no play-next command: `n` is consumed by the window and
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

The `play` and
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

`src/window/input.rs` forwards a keydown only after the window's key router, and it
never interprets the key itself. The router answers one of three outcomes
(`KeyOutcome`): `core` sends the key to the `key` method with the panel
target the pointer resolved and its default
action prevented, `handled` means the window content took the key so the core
receives none, and `text` means an open text field takes it, so the browser's
own default types it. `src/window/app.rs` installs `routeLibraryKey`
(`src/window/library/keys.rs`) as that router, so the Library's handler runs
before the forwarder (design D3).

While the frame carries `settings`, `routeLibraryKey` hands the key to
`routeSettingsKey` (`src/window/settings/keys.rs`) after the overlay, prefix,
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
   playback-target chooser, the Queue chooser, the confirmation modal, the
   daemon-lost recovery dialog) or prefix mode is armed, the key goes to the
   core with the target the pointer resolved. Both facts are in the frame. The Queue filter's text
   entry is the core's too, so every key is the core's while it holds the
   keyboard, with the `Queue` target wherever the pointer is — the keys the
   filter does not consume still reach the Queue leaf.
2. The `Ctrl+K` chord arms the Search field: the top bar's Search
   control paints in every screen but Settings — above the Queue, the
   Library panel, and the Playlists screen — so the chord arms the field
   wherever a mounted element paints it; a collapsed-and-closed control
   holds no field, so the chord opens its panel there instead, and on the
   Settings screen, where no control mounts, it keeps its core effect
   (design D9 of `move-playback-header-into-queue-footer`).
3. While a text field holds the keyboard, the field takes the
   key: the Search field first — its hold is DOM focus, which it keeps
   while it paints on any screen but Settings; Esc closes its
   results, the arrow keys move the highlight, and Enter activates the
   marked one — then the `/` filter, whose Esc closes it and Enter
   accepts it. The collection dropdown's own filter takes the typed keys,
   Backspace, and Esc while the dropdown is open, wherever the pointer is;
   a check change leaves it open.
4. Esc closes what the window opened, wherever the pointer is: the
   right-click row menu, the picked item's menu, the Narrow arrangement's
   Hero overlay, the collection dropdown, then the Search results
   dropdown. With nothing open, Esc stays the core's key.
5. A digit `1` to `7` opens the destination tab at its place after Home.
6. While the pointer rests over the Library column, the Library keys run on
   the pointer target (ADR 0040): `/` opens the list filter, Right and Left
   open or close the hovered tree node, Enter plays the hovered row's item,
   `n` plays it next, `a` adds it to the queue, and the arrow, Page Up, Page
   Down, Home, and End keys scroll the list the pointer rests on. With no
   hovered row, the keys that act on one are consumed and do nothing. Space
   keeps its core effect: no collection dropdown control holds a keyboard
   focus, so the dropdown answers a press only.
7. Every other key goes to the core's keyboard router with the target the
   pointer resolved: a key over the Queue list box names the `Queue`
   target, and every other key names none. So `[` and `]` seek, and the
   GUI-only chords `z`, `q`, and `c` and every `[keys]` binding keep their
   effect.

With the pointer over the Queue list box, no Library key runs, so `/` opens
the Queue filter and `a` opens the audio popover as before.

### The arrangement

`libraryArrangement` (`src/window/arrangement.rs`) picks the
arrangement from the measured Library column width — the window width minus
the Queue panel's current painted width, 400 px expanded, 72 px in the rail,
or the width a drag gave it — with the canvas sizes as thresholds. Below
480 px is Mini, from 880 px with at least 600 px of height is Wide, and
between them is Narrow. The thresholds are window constants beside the
Queue panel's state constants, and the core does not classify the size. Mini
shows one column at a time (`move-playback-header-into-queue-footer` D8):
the Queue panel first, and the global header's destination press switches
the view. While the Queue shows, a press on a destination tab makes that
destination active and shows the Library in place of the Queue; while the
Library shows, a press on the already active destination's own control — its
tab, or the Home icon while the Home content is present — puts the Queue
back, and a press on a different destination keeps the Library showing.
Outside Mini the view is unaffected: every other arrangement paints both
columns. Narrow mounts both panels, and
the row menu's View Detail entry opens the Hero as an overlay over the list;
a press or a pick never does. Wide mounts both
panels with the Hero beside the list — the list at the left edge of the
Library panel, the Hero pane to its right. The Queue panel keeps its four states
inside every arrangement, and a resize across a threshold moves no active
row, collection selection, or picked item, because those live in the window
content and the core.

Both columns are resizable (`src/window/resize.rs`). A drag on either
divider (`src/window/components/panel_divider.rs`) reports the sized column's new width in
pixels — the Queue panel's column, or the Wide arrangement's list column —
and the window content keeps it for the session. A width survives tab and
breakpoint changes because the store is module state, and every drag is saved
to `gui-panel-widths.toml` in the state directory, so the next launch
reads the widths back; a corrupt value falls back to the defaults. The list
column defaults to 368 px, the canvas's 320 px index 15 % wider. The Queue
column holds between its 400 px artboard and the lesser of its 800 px cap
and the width that leaves the Library column its 480 px Mini threshold, and
the list column holds between
320 px and 640 px, short enough to leave the Hero pane its minimum.
`panelWidths` resolves the two painted widths and the Library column they
leave, and the arrangement classifies that same value, so a drag paints the
geometry the thresholds measure and the two never disagree. A divider takes
no width in the row: its 6 px grab band overlays the seam through negative
margins, so the columns meet directly. A double-click returns a divider to
its default. The
dividers are the window content's: they report no region to the core, which
needs none, because the Queue panel's own resize observer re-reports the
regions a drag moved.

While the Settings screen is open it replaces the two columns
(add-gui-settings-screen design D11): the sidebar sits at the left window
edge and paints no divider; the shown section's content takes the rest of
the window, and the status line closes the screen with the frame's `status`
facts (the prefix-armed state); toasts show as floating cards over the
screen (`gui-toasts`). Below the Mini threshold
— `libraryArrangement` measured on the window width minus the sidebar's
column — only the pane that holds the keyboard shows: the sidebar until a
section opens, then its content. A resize across the threshold keeps the
open section, which the core owns.

Panel focus stays in the core's `FocusStack`. The window carries no copy of
it: `CoreThread::route_key` sets it per key from the key's target — a
`Queue` target sets `Queue`, no target sets `Library` — and each
`GuiLibraryCommand` the core applies sets Library Panel focus first (ADR
0040). The GUI's `App` applies neither the TUI's terminal-width Mini check
nor its hidden empty queue column.

### The launch location

The GUI keeps its own launch location (design D9). Its `App` starts with
`LaunchRestore::Done`, so the TUI launch state never moves the GUI's tab,
and the GUI never writes the TUI file. The file also carries a separate
`shows_view` field for the selected Shows destination view; an older file
without the field loads as All. The GUI file
(`mbv_config::load_gui_launch_state` and `save_gui_launch_state`) holds the
destination, one entry per collection destination whose selection the
session changed: the checked Music groups, Podcast shows, Feeds subscriptions,
or Emby feed-group folders, and the album each Music group last showed; and
one entry per flat list whose sort the session changed, keyed by destination
identity (gui-flat-list-sort design D10). Each entry names its collection
`target`, so saved ids restore only to their own dropdown; an entry written before the
target field loads as Music. The core reads it once before the first step,
saves it at orderly exit before `client_exit_teardown`, and never while the
window is open.

The restore runs in two phases (`GuiLaunchRestore`,
`CoreThread::restore_launch_location`). Every start opens on Home: the first
phase opens no destination and ignores the saved one, and only waits until a
presented destination exists, so the saved selections have lists to land in.
A start with no saved location — or one that fails to load — restores the
same way (`mbv_config::first_launch`), with no saved collection choice. The
second phase applies the saved collection choice once the destination's
collection catalog lands, and applies the saved `shows_view` to every
presented Shows destination: it matches the saved destination and target,
retains only the saved ids the catalog still holds, installs the checked set
through the shared collection path, and installs each saved sort the session
has not set itself. A saved collection entry that carries an old date
order and names a destination with no saved sort opens that destination on
Newest or Oldest. An emptied set, a saved All, or no saved entry leaves All
selected. The
restore applies to a destination that holds no choice and never clobbers one
the session already made. A mid-restore tab switch abandons the saved sets rather than landing
them under another destination. The restore is idempotent, so the catalog and
library drains call it whenever their content can complete a phase. The saved
picks restore with the group list, and a group with no pick heroes its first
album (design D13).

## The Settings screen

`settings(GuiSettingsCommand)` is the fifth `CoreLink` method (`src/window/channel.rs`). It
only pushes `CoreInput::Settings` into the core channel, as the other commands
do. `src/bridge/settings.rs` translates the wire variants and
`GuiCore::apply_gui_settings_command` (`src/app/gui_settings/command.rs`) applies
each one through the existing `App` method, so the TUI's own paths do not
change. The variants are `open`, `close`, `selectSection` (`section`), `activate`
(`setting`, the `SettingKey` name), `setValue` (`setting`, `value`), `applyChoices` (`kind`, `selected`),
`assignRoute` (`library`, `endpoint` or none), `embySetup` (`serverUrl`,
`username`, `password`), `audiobookshelfSetup` (`serverUrl`, `apiKey`),
`serviceAction` (`action`), `cancelSetup` (`service`), `feedAdd` (`name`,
`url`, `kind`), `feedEdit` (`url`, `name`, `kind`), `feedRemove` (`url`),
`keysCapture` (`target`, `key` as a `DomKey`), `keysReset` (`action`),
`keysClear` (`target`), `selectTarget` (`target`), and `resetUiState`. Identities are stable — a `SettingKey` name, a playlist id, a
feed URL, a chord — never row positions, so a command built from a stale frame
addresses what that frame painted or nothing, and a `setting` name this build
declares no key for logs and changes nothing. `src/bridge/keys.rs` translates a
`keysCapture` press to the neutral `KeyEvent` the router normalizes, so the
window never forms a chord string and the two Clients cannot disagree on one
chord's spelling.

The Playlists screen is not a Settings section. It is its own main-UI screen:
it takes the Library panel's slot under the top bar, with the Queue to its right.
The saved playlists paint as a tree (one open playlist at a time, as the core holds
one), and the hero beside the tree is the picked row's control set — Play, Shuffle,
Add, Rename, Refresh, and Delete for a playlist; Play, Shuffle, and Add for an
item. A right-click on a row opens the same actions as a menu. The core owns
whether it is open (`GuiCore::gui_playlists`, `src/app/gui_playlists.rs`) and sends
the `playlists` section (`GuiPlaylistsFrame`); the window owns the pick, the
menu, and the rename draft. Its `playlists` command carries `toggle`, `close`,
`open` (`id`), `collapse`, `refresh` (`id` or none), `action` (`id` and `play`,
`shuffle`, or `enqueue`), `itemAction` (`id`, the item `id`, and the action),
`rename` (`id`, `name`), and `delete` (`id`). Opening one of the two screens
closes the other, and a press on a destination tab closes the Playlists screen.

The Client core owns whether the screen is open and which section it shows
(`App::gui_settings`, `src/app/gui_settings.rs`); the window content owns its
interaction state — the sidebar cursor, the keyboard pane, each list's cursor
and scroll, the form drafts, the capture, and the Mini pane (ADR 0037). The
frame's section is `GuiSettingsFrame`: the shown `section`, the `sections` the
sidebar lists, and `content`, a tagged union with `rows` (an F2 page's rows),
`services`, `feeds`, `keys`, and `sessions`. Each F2 row carries
its stable setting name, its label, its kind (toggle, cycle, pick with its value list, choice list,
action, or info), its value text, its toggle flag, and its choices once the section's
load lands. The `info` kind is the read-only row: the Session section's
"This client" and "Owner" version rows use it. An `info` row paints a label
and a value with no control. Its value does not come from configuration;
`App::version_row_value` formats it, so both Clients show the same text.
An Enter or a click on an `info` row sends no `settings` command. The
Services content carries each Service's state and the actions
the TUI offers for it, with the open setup form's progress and error; the Keys
content the prefix chord, one row per `AllClients` action, and the last
refusal; the Sessions content the same playback-target choices the footer's
chooser lists; and the Playlists content the saved list, the open playlist, and
the loading flags. No variant carries a password, an API key, or any other
Service credential. The `settings` section is absent while the screen is
closed,
so the window mounts the screen only for a batch that carries it.

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

The `panel` section carries the UI-reset counter `ui_reset_epoch` (design
D10): the window
resets its Library interaction stores and its panel widths when the value
changes, and the TUI's own reset does not move it.

The window-content half lives in `src/window/settings/` (design D11):
`settings/state.rs` holds the screen store — the keyboard pane, the sidebar and
content cursors, the Mini pane (which is the keyboard pane), the form
drafts, the choice marks, and the capture — and `settings/entries.rs` builds each
section's flat list of activatable controls, which `settings/keys.rs` routes the
screen keys over. `src/window/components/settings_*.rs` holds the screen,
the sidebar, one component per section kind, and the status line, and
`src/window/app.rs` mounts `SettingsScreen` in place of the two columns while
`frame.settings` is present. Commands go through `CoreLink::settings`.

The screen paints the design canvas `canvas/Settings.dc.html` of the
`add-gui-settings-screen` change (design D13). The pointer is the primary
path and keys give parity (design D11, *Mouse-centric*). The global header's
Lucide `Settings` button, at the bar's right end, sends `open`, which opens
the screen at Services and does nothing while it is open. The sidebar's
`ArrowLeft` Back button sends `close`. The sidebar is a fixed 260 px
(`SETTINGS_SIDEBAR_WIDTH_PX`), independent of the Queue column, and Mini
measures the window width minus it. The sidebar's rows carry one Lucide icon
each, and the Services section is labelled Sources (a UI label only; the
code and the domain keep Service). Every section paints only through the
shared parts in `src/window/components/`: `SettingsCard`,
`SettingsRow`, `SettingsSwitch` (every toggle), `SettingsButton` (secondary,
primary, destructive, ghost), `SettingsCheckboxGrid` (the choice lists, with
their Apply), `SettingsStatePill` (a Service's state), and
`SettingsPickButton`. The pick button serves Subtitle mode, Subtitle
language, and Audio language: it opens a value menu in the row menu's chrome,
which the Settings key router owns while it is open, and an entry sends
`setValue`. `App::set_settings_value` sets the value with the cycle handler's
player update and deferred save, and the TUI's cycle calls it with its next
value, so the two Clients share one effect. The value lists come from
`mbv-ui-model` (`SUBTITLE_MODES`, `lang_cycle`), and the empty language
paints as Any.

The Tauri command grant rule no longer applies. Under Tauri, every command had
to be declared in the app manifest and granted in the capability file, or the
core never saw it. The window now calls `CoreLink` methods in the same process,
so no command registry and no grant exist.

## The attach states

Before the attach there is no `App`, so `src/attach.rs` (`AttachCampaign`)
owns the pre-attach phase on the core thread. It emits `GuiAttachState`
states — the two refusal texts, the waiting state while an Owner shuts
down, the version-or-wire mismatch, and the attach-failed text — the same
texts `startup::run_local_instance` gives the TUI Client. The attach gate
compares the application version and the wire identity from the Owner's
hello with this Client's; a different build identity alone no longer
refuses, and the build identity is display only (the Settings rows show
it). The refusal is `RemotePlayerErrorKind::OwnerMismatch`, code
`remote-player.owner_mismatch`. The mismatch state
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
window, and the Client core supplies only its data and actions: the GUI
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
2. **Carry it in a section.** Add a DTO under `src/bridge/frame/`,
   built from the shared projection or the adopted Surface state, a field on
   `GuiSectionBatch`, a `Section` variant, and — for a gated section — its
   mark row (`src/sections/marks.rs`) or its mark source. Keep text
glyph-free.
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
6. **Paint it.** Add a component under `src/window/components/` that
   paints the section, reports its list capacities and painted
   regions through `LayoutReporter`, retracts on unmount, and mounts in
   `src/window/app.rs`. Icons are Lucide only. Every control a pointer can reach
   must keep a key chord (the `gui-client` spec: every action is reachable
   from the keyboard).
7. **Prove the contract.** Rust side, a nextest in `crates/mbv-gui/tests/`
   for a mapping that is new (region-to-`Msg`, capacity applied). Window,
   a headless test asserting the painted structure, not appearance.

## Tests and checks

The window tests sit beside their modules in `src/window/**/tests.rs`. Each
test is stateless: it builds its input, calls once, and asserts on the return
value or the painted output. A component test renders a fixture batch through
`DioxusDocument` and asserts on what it paints. No test opens a window, sleeps,
or reaches an Owner or a network. The Rust tests in `crates/mbv-gui/tests/`
mock the channel, the batch sink, and the Owner steps. Live behaviour in the
real window is the user's manual check.

The crate's logs go to `mbv-gui.log` in the mbv state directory, not the
TUI's `mbv.log`.

## Blitz gaps

Blitz is pre-release, and its gaps shape the window (ADR 0039). Each gap has
one handling:

| Gap | Where | Handling |
| --- | --- | --- |
| No `text-overflow: ellipsis` | boxes that need an ellipsis | `Truncate` (`src/window/truncate.rs`) measures the text and cuts it with `…` |
| `:has()` is not a valid selector; one invalid selector drops its whole selector list | the shared hover rule in `assets/app.css` | Never use `:has()`. Mark the element (`data-hover-frame`) and select the marker. `a_filled_button_paints_the_hover_veil_under_the_pointer` guards the hover rule. |
| An inset `box-shadow` with a spread past half the box paints nothing (the renderer takes `rect.abs()` of the inverted hole) | the hover veil | The veil is a one-colour `linear-gradient` in `background-image` with `!important`, not a `box-shadow`. |
| No container queries (`cq*` units) | the player stage's control-row scale | The window measures the stage and sets `--player-scale` (design D3 of `gui-art-player-overlay`). |
| An animation repaints the whole window | `animate-spin`, `transition` classes | Accepted. They run only while shown. |
| No `NetHandler` error callback | artwork misses | The net provider sends `ArtEvent::Failed` (`src/window/net.rs`) |
| Transform painting | the scrubber thumb's `translate` | Accepted. |
| Slow Wayland resize, and surface timeouts | every resize | Accepted. |
| No GPU: surface creation panics | hosts with no Vulkan or GL | Accepted. The packages depend on the Vulkan loader. |
| Some DOM event kinds panic | none | No handler uses them (`src/window/input.rs`). |
| No reload of the window content | dev reload | Reload scenarios no longer exist. |

## Dependency notes

- `calloop` 0.14.5 depends on `tracing` with `tracing`'s `log` feature, so its
  tracing events also reach `log` for the whole workspace.
- `unicode-segmentation` 1.13 is a dependency of `mbv-gui` for the grapheme cut
  of the ellipsis (`src/window/truncate.rs`).
- `time` and `num-traits` are direct `mbv-gui` dependencies. They were behind the
  `blitz` feature while the port ran, and they are not in the
  dependency list of the port's design.

### Library Hero and list shared pieces

Invariant 30 (`docs/invariants/30-shared-ui-is-never-bent-to-fit-a-consumer.md`)
governs these. `LibraryHeroFrame` paints the Hero for Music, Shows, Podcasts,
Audiobooks, Feeds, and Movies: a required header band (the largest unit's
kicker, title, and plain facts — a movie is its own largest unit), the detail
block (art, selected unit, actions, overview), and the list band (the movie's
credits table and the IMDb button).
`LibraryWorkspaceTable` paints the list band's rows,
`LibraryHeroActions` the Play, Play next, and Add to queue buttons,
`LibraryDropdown` the Shows Workspace's season dropdown (a radio row per
season and a filter field, one open state in `LibraryUiState.shows_seasons`),
`LibraryItemRow` the
top-level rows of every list, `LibraryLetterHeading` the letter and section
headings, and `LibraryVirtualList` the scroller of every list. Home still
paints `LibraryHeroSummary`. The episode row state is one module
(`src/window/library/episode.rs`): `is_played_episode` and
`episode_progress` read the played state and the resume percent an episode
row and a Hero paint, whatever destination carries the row, so no other
destination's Hero imports Shows logic.

### The top-level Library row and its art

`LibraryItemRow`
(`crates/mbv-gui/src/window/components/library_item_row.rs`) is the one
top-level row part of every destination (`gui-delineated-rows` design D1, D2):
Home, the Emby lists, the Music artist rows, the Shows series rows, the New and
Upcoming rows, Audiobooks, Podcasts, and Feeds. A destination supplies its hook
prefix (`data-library-<attr>-*`), its text, its thumb, and its handlers, and
never restates the look. The row is 56 px high with square corners, an 8 px
inline padding, a 12 px gap, and a 1 px `--canvas-border` divider under it;
`LibraryVirtualList` paints the matching rule over the list's first row, so the
list frame has one top rule in every destination.

The thumb is a 64 × 36 box on `--canvas-well`: a landscape or portrait image
cover-fills it, a square image shows whole at the box's height, and a row with
no image paints the media kind's `EighthNotes` or `MonitorPlay` icon on
`--canvas-muted`. The row's resume percent paints in 3 px along the box's
bottom edge, on `--canvas-thumb-track` under `--canvas-partial`, and no row
paints a progress run under itself. A top-level tree node — a Music artist row
or a Shows series row — also names its open state (`gui-tree-double-click`
design D1 to D3). A press on a tree parent row picks it and opens or closes
nothing; its double-click opens the row when it is closed and closes it when
it is open, and plays nothing. No tree row paints an open-or-closed glyph or
keeps a spacer where one stood: the nested rows' indent comes only from the
row inset (Music artist 8 px, album 56 px, track 68 px; Shows series 8 px,
season 56 px, episode 68 px; Playlists playlist 8 px, item 32 px). Every open
tree parent that shows child rows hangs a 1 px guide under its leading element
through its subtree, painted per row by the shared `TreeGuide` part
(`crates/mbv-gui/src/window/components/tree_guide.rs`), so the guide stays
whole while only the rows in view paint. A top-level parent's guide paints in
`--canvas-tree-guide` and a nested parent's guide in
`--canvas-tree-guide-nested`. The part carries the row's `aria-expanded`, and
while the pointer rests on the row it
shows the shared `TreeHint`
(`crates/mbv-gui/src/window/components/tree_hint.rs`) — `Double-click to
expand` when the row is closed, `Double-click to collapse` when it is open —
as a tooltip card under the row's bottom edge, through the row's `tree-parent`
class and the `.tree-parent:hover` rule in `assets/app.css`. The row is the
card's positioning box and does not clip it, and the row's trailing text stays
visible. The rule is one for
every GUI tree parent: a Music album, a Shows season, and a Playlists
playlist paint their own rows and take the same press, the same double-click,
the same `aria-expanded`, and the same hint, while a leaf row — a track, an
episode, or a playlist item — takes none of them. A double-click on a leaf
row still plays the row's item, as its Enter play does. Nested rows keep
their own painters and heights: a Music album or track, and a Shows season
or episode.

Every row frame carries `GuiRowArtFrame` — the cached image and the fit its own
pixels ask for, a fetch in flight, a source that answered with no image, or a
kind with no art source — and `GuiRowMediaKindFrame` for the icon. The core
fills both from the item's Hero artwork source (`gui-delineated-rows` design
D5), so a row shows the image its Hero shows, and resolves them from the cache
on each projection without a fetch.

The list reports the rows it paints through
`GuiLibraryCommand::RowsInView { scope, ids }`
(`crates/mbv-gui/src/window/library/list/geometry.rs`, `rows_in_view_ids`): the
painted
window plus four rows of overscan either side, sent only when the set changes.
`GuiRowArtScope` names the list the ids belong to — Home, Emby, Music,
ShowsSeries, ShowsNew, ShowsUpcoming, Podcasts, Audiobooks, or Feeds — and a
Shows list reports the scope of the view it paints. The ids are the rows'
stable ids and never coordinates; a nested row names none, and the Music artist
rows report the core's own `artist:` and `artist-name:` encoding. The core
stores the set per scope, stamps its navigation-idle clock, and queues the
in-view rows' `Row` fetches once navigation is idle
(`src/app/gui_library/rows_in_view.rs`). Row cache keys carry the Service's
change value (`row:{item_id}:{type}:{tag}`, and
`row:abs:{server_hash}:{library_item_id}:{updated_at}`), the cache keeps its own
512-entry row LRU, and row fetches take at most 4 of the 6 fetch slots, so a
Hero fetch always has one.
