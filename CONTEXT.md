# mbv

A terminal client for Emby, Audiobookshelf, and Feeds that browses catalogs and
plays media. Playback may run inside the terminal process itself, or be hosted by an
out-of-process Player owner — the Local daemon on the same machine — so it survives
the terminal closing.

## Services

**Service**:
One of mbv's singleton media integrations: Emby, Audiobookshelf, or Feeds. Each
kind exists at most once within mbv; Feeds is always present even when it has no
subscriptions.
_Avoid_: account, provider, backend

**Remote Service**:
An Emby or Audiobookshelf Service reached at a configured server and authorized
with its own Service credential.
_Avoid_: account, remote provider, backend

**Service setup**:
Establishing a Remote Service by successfully validating its server and Service
credential. mbv itself never requires Service setup before it can start.
_Avoid_: app login, account creation, onboarding

**Service-independent startup**:
The guarantee that mbv enters its TUI before any Remote Service authenticates or
becomes available. Each configured Remote Service initializes independently after
the first frame; one Service's failure does not delay the others. Feeds needs no
Remote Service.
_Avoid_: no-auth mode, offline mode, provider mode

**Services view**:
The Settings surface for setting up remote Services and managing feed
subscriptions. mbv opens it initially when no Remote Service is configured and
Feeds has no subscriptions.
_Avoid_: login screen, setup wizard, authentication gate

**Service state**:
The current availability of a remote Service: Not configured, Connecting,
Ready, Needs authentication, or Unavailable. Unavailable preserves credentials;
Needs authentication means the remote Service rejected them.
_Avoid_: login state, account state, online status

**Service replacement**:
Changing a remote Service to a different server. It invalidates queued and
persisted state whose identity belongs only to that previous setup; unrelated
Service state (e.g. Feed entries, other Service's items) is preserved in mixed
queues.
_Avoid_: reconnect, migration, account switch

**Service removal**:
Deleting a Remote Service's setup, credential, and local state and returning it
to Not configured. Only that Service's owned state is purged; other Services'
queued and persisted media remain.
_Avoid_: logout, disconnect, disable

**Service credential**:
A secret issued by Emby or Audiobookshelf that authorizes mbv to that Service.
It belongs only to that Service. Stored in a per-Service mode-0600 secret file,
never in `config.toml`.
_Avoid_: account credential, mbv token, control token

**Control credential**:
An mbv-owned secret used by a Local daemon to admit Clients independently of all
Service credentials. It grants control access, not an identity or login;
packaged mbvd does not use this mechanism yet — it currently still uses legacy
Emby-token ctrl authentication and will migrate to filesystem/trusted-LAN
authorization as part of issue #523.
_Avoid_: Emby token, API key, login token

**Service-owned state**:
Local state whose remote-native identity is meaningful only for one Remote
Service setup. Authentication repair preserves it; Service replacement or
removal invalidates only that Service's owned state, not unrelated Services or
Feeds.
_Avoid_: provider cache, account state

**Setup generation**:
A per-Service monotonic counter that guards stale asynchronous setup completions.
Every replace/retry/setup attempt bumps it; a completion is applied only if its
generation matches the current runtime.
_Avoid_: setup version, auth generation, connection ID

**Owner admission / Service eligibility**:
The rule by which a Player owner decides whether a QueueItem may enter its Bound
queue. It evaluates media kind (audio vs video), whether the required Remote
Service setup and credential are loaded in that owner process, and whether ctrl
peers negotiated transport for that item kind. Bare mode may admit Emby, Feed,
and Audiobookshelf items when their Services are Ready; Local daemon and packaged
mbvd currently admit Emby and Feed (audio-only owners admit only the audio
subset of a mixed submission); Audiobookshelf daemon admission is tracked in
milestone #524.
_Avoid_: owner capability, queue capability, supported kinds

## Playback ownership

**Player owner**:
The single process on a machine that holds the audio device, Bound queue, and
Service-specific playback lifecycle. Exactly one exists per user at a time.
Different owner kinds have different Service eligibility.
_Avoid_: instance, master, host

**Out-of-process owner**:
A Player owner running outside the terminal application's own process — the Local
daemon or packaged mbvd — reached over ctrl. Out-of-process says which process holds
playback, not which machine: a Local daemon is always on this machine and still
classifies as on-this-machine; only TCP or Unix daemon endpoints point elsewhere
(`player-target-locality`). Bare mode is never out-of-process.
_Avoid_: remote owner (bare), background owner, external player

**Bare mode**:
The default presentation, where one process is both the terminal UI and the
Player owner. Closing it stops playback. Bare mode is currently the only owner
eligible for Audiobookshelf podcast and book playback.
_Avoid_: foreground mode, standalone, normal mode

**Stay-alive**:
The mode in which playback is hosted by a Local daemon rather than the terminal
UI, so playback continues after every terminal window closes. The Local daemon
is the Player owner; Clients are disposable UIs that attach to it.
_Avoid_: daemon mode, background mode, alive mode, persistent mode

**Audio-only owner**:
A Player owner configured with `--audio-only` (packaged mbvd ships this way)
that can only play audio. It never holds a video item; a mixed submission that
contains audio is accepted minus the non-audio items (wholly non-audio remains
refused). When a Client explicitly plays a video through an eligible ctrl
attachment or controlled Emby session, mbv prompts with the owner and selection
named; confirmation stops the owner, ends the attachment, and plays locally,
while a decline changes nothing. The Local stay-alive daemon is never audio-only.
See `openspec/changes/play-locally-when-owner-cannot` for this shipped behavior.
_Avoid_: audio daemon, headless audio owner, mbvd audio mode

**Playback run**:
The local mpv playback loop — one per mpv invocation, owned by a Player owner.
Distinct from Session (the Emby-tracked record that exists independently of
mbv) and from an Audiobookshelf playback session. The owner's canonical queue
remains authoritative whether mpv mirrors it eagerly or materializes only its
active file (active-file projection is used once a lifecycle-backed source such
as an Audiobookshelf episode or book enters the run).
_Avoid_: session, playback session

**Clocked audio output**:
Packaged mbvd's inherited Playback-run output: mpv's `audio-device` property is
bound to a real ALSA endpoint, so hardware paces playback. Distinct from
selecting the legacy PCM pipe output, which writes untimed PCM through mpv's
`ao=pcm` file writer into a FIFO for an external consumer such as Snapserver.
_Avoid_: ALSA mode, direct audio, hardware output

**mpv script set**:
The entry script `mbv.lua` and its sibling Lua fragments that build mpv's on-screen
control overlay (OSC) and Next-Up prompt. Exactly one copy is live per playback run:
the copy belonging to the running build, never a leftover from the removed installer,
a previous version, or another build. Distinct from mpv's own configuration and user
scripts, which mbv never reads or edits.
_Avoid_: OSC bundle, overlay bundle, Lua scripts (bare), script folder

**Resolved script path**:
The one script-set path handed to mpv, chosen by a single rule: the running build's
checkout copy (`<checkout>/scripts/mbv.lua`) when it exists, otherwise the installed
copy at `/usr/share/mbv/scripts/mbv.lua`. The removed installer's user-directory path
is never a candidate; an unused copy there is named in a startup warning and left
untouched. The font directory resolves under the same rule, so scripts and fonts
always come from the same source. Startup reports the resolved path.
_Avoid_: script source (bare), active script, script lookup

## Processes

**Local daemon**:
The Player owner in stay-alive mode: a user-owned background process on the same
machine as its clients, holding no terminal. One exists per user. It starts
without authenticating any Remote Service (Service-independent Local daemon per
ADR 0018) and is stopped only by explicit lifecycle request when stay-alive is
off.
_Avoid_: relay, backend, server, session host

**mbvd**:
The separately packaged daemon, run as a system service, with its own
configuration, state, and socket. A different product surface from the local
daemon, never started by a terminal UI. On `main` it is still Emby-gated: it
constructs `EmbyClient` unconditionally, requires cached credentials to start,
and uses legacy Emby-token ctrl authentication. Service-independent startup
(zero Services), Feed playback without Emby, optional Emby runtime, filesystem/
trusted-LAN authorized ctrl, and `mbvd --connect emby` administration are
implemented in open PR #529 tracking issue #523 — do not describe them as
landed on `main`.
_Avoid_: system daemon, the daemon

**Client**:
A terminal UI that reaches an out-of-process Player owner over ctrl. Attachment
does not log it into the owner or establish a Service identity. Any number may
attach at once, and each is disposable.
_Avoid_: thin client, terminal client, viewer, attachment

**Tray**:
The desktop status icon belonging to the Player owner, giving playback controls
and a stop action while no client is on screen. Only present when the owner
enables it; for the Local daemon this means stay-alive mode.
_Avoid_: systray, status icon, indicator

**Daemon endpoint**:
The address form used to reach any Player owner's control socket: either
Local (this machine's own Local daemon) or a network address (Unix or TCP)
pointing at another Local daemon or an mbvd.
_Avoid_: connection string, remote address, socket path

## Continuity

**Playback continuity**:
The guarantee stay-alive makes: what is playing, the queue, and position survive
every client closing and reopening.
_Avoid_: persistence, session continuity

**Session continuity**:
Preservation of a client's on-screen state — cursor, scroll, open overlays,
search — across a close and reopen. Deliberately *not* offered; only playback
continuity is.
_Avoid_: terminal continuity, UI state

**Attach**:
A Client establishing a ctrl connection to an existing Player owner. It is a
control relationship, not login or Service setup; its handshake may present a
Control or legacy Service credential for admission without establishing Client
identity. Several Clients may attach at once without displacing one another.
Connecting never evicts existing clients (multi-connection model, ADR 0014).
_Avoid_: reattach, connect, resume, take over

## Queue

**Queue slot**:
One independently addressable occurrence of a QueueItem in a canonical queue.
Two slots may contain the same content while retaining distinct slot identities.
Slot identity is stable across moves; content identity is not.
_Avoid_: queue item, content ID, playlist index

**Content identity**:
The Service-qualified identity of media content, distinct from the identity of
each Queue slot containing it. Provider-qualified: Emby ID, Feed guid, or
Audiobookshelf (libraryItemId + episodeId) tuple.
_Avoid_: item ID, queue ID, slot ID

**Queue source**:
The origin recorded for a queue: Playlist (with optional id/name), Album,
Series, Shuffle, Remote, Collection (with collection type), or Unknown.
Preserved across restore and used for UI display and save-on-consume decisions.
_Avoid_: queue origin, queue type, source type

**Consume**:
Removal of an item from the queue once it finishes playing, as in ncmpcpp.
Purely a queue operation — it says nothing about where the queue came from and
never edits anything on the server. Driven only by the authoritative Player
owner's playback lifecycle (a local in-process Player, a Local daemon, or a
directly controlled remote Player owner); a Session watch of another device's
generic Emby Session never consumes, because that observation carries no mbv
queue authority. Addresses canonical slot identity; removes only the consumed
occurrence.
_Avoid_: auto-remove, playlist consume, consume-and-save, remote consume

**Save on consume**:
The separate, opt-in behaviour of writing the shortened queue back to the Emby
playlist it was loaded from. Only meaningful for a queue that is a saved
playlist; Consume happens with or without it.
_Avoid_: autosave, consume persistence, playlist sync

**Composed**:
The stage in which a queue is held in a client's UI and no Player owner holds
it. Editing one has no playback consequence, so it doubles as a staging area —
build it now, play it later. Not every queue is Composed first; playback
started from Emby reaches an owner without passing through a UI.
_Avoid_: draft, staging queue, pending queue, unplayed queue

**Bound**:
The stage in which a Player owner holds a queue. Its contents answer to that
owner's rules — an audio-only owner does not hold items it cannot play; an owner
without loaded Audiobookshelf setup does not hold Audiobookshelf items. Bound
does not mean playing: a stopped owner still holds its queue, and queues can be
Bound to two owners at once while only one of them plays.
_Avoid_: active queue, live queue, running queue, attached queue

**Unplayable item**:
An item a Player owner lacks the capability to play, because of media kind,
required Service availability, or playback support. It never enters that
owner's queue: a controlling Client strips it before submitting, and the owner
discards any that reach it regardless. Wholly non-audio submissions to an
audio-only owner surface a structured rejection.
_Avoid_: rejected item, filtered item, invalid item, blocked item

**EmbyItem**:
The queue's Emby-side item type — the serialized record of an Emby library
item. Renamed from MediaItem; the rename is wire-invisible because serde
field names are unchanged. Positions for EmbyItems report to the Emby API.
_Avoid_: MediaItem, media item, emby entry

**QueueItem**:
The queue's media snapshot — an EmbyItem, FeedEntry, AudiobookshelfQueueItem,
or AudiobookshelfBookQueueItem. Generic queue operations use shared presentation
and identity behavior; Service-specific admission, source preparation,
lifecycle, progress, and cleanup remain explicit boundaries. Persistence
round-trips tagged QueueItem values; legacy untagged Emby-only payloads remain
readable.
_Avoid_: queue entry, playable, mixed item

**Playback resume**:
The rule deciding whether a previously watched entry should resume or start over.
When position exceeds the 6% threshold of runtime (or runtime is unknown and
position > 0), resume starts from that position; otherwise from zero. Applies
to Emby items, Feed entries, and Audiobookshelf episodes using the same
threshold.
_Avoid_: continue threshold, resume percent, watched threshold

**Playhead**:
The shell's single reconciled answer to where playback is: the queue scope that
is playing, the active slot within it, and whether that slot is confirmed by the
Player owner or an optimistic prediction not yet acknowledged. A prediction
records its reason — a queue edit relocated the still-playing item, or a
different item was selected to play. Reconciled in one tick-phase step against
owner status, never during paint; playback position itself stays a live read.
_Avoid_: pending active index, cursor push, now-playing index, active_idx

## Browsing and tabs

**MediaList**:
The shared provider-neutral owner for one logical media-row flow. It owns rows,
stable-target selection, cursor/scroll, row-local behavior, and retained
geometry. It is embedded in the destination's `LibraryPanel` slot; it is never
mounted, focused, subscribed, or given a ComponentId. The Library panel owns
the skeleton and slot; the destination retains Service content and typed
translation.
_Avoid_: generic list, generic media list, two-column list

**Multi-selection**:
The set of rows a user has picked in one MediaList for a bulk action, built by
Ctrl+Click, Shift+Click, or keyboard Visual mode (`V`), keyed by stable
targets and held by the MediaList beside its cursor. A non-empty
multi-selection is Visual mode. "Selection" alone still means the cursor row.
_Avoid_: marked rows, selection (for the set), checked items

**WideMediaList**:
The provider-neutral, one-column fixed-row TuiRealm Component over a
`MediaList<Target>`. It is the canonical control for Library browser rails,
Workspace lists, and Queue rows in every Panel mode; it never chooses a Service
or destination. The Library panel owns placement and supplies the current row
flow rectangle, while the destination retains Service content and typed
translation.
_Avoid_: generic list, two-column list, Inline Search

**Media-list row**:
The one painted fixed-height row of a `MediaList` flow in every Panel mode. Its
left-aligned metadata slot carries one closed role: a release year in the
green metadata role or a progress badge in the FOAM one; its right-aligned
duration slot is green.
_Avoid_: wide media row, wide_media_row

**Group heading**:
The non-selectable Heading row labelling a group of media-list Item rows (artist, feed age bucket, letter or surname bucket, season). Painted bold in the FOAM metadata role; unlike a media-list Item row it keeps the surface fill and never paints the selected-row bar. It is a visual label, never a selection or action target, and it takes its own place in the zebra alternation.
_Avoid_: artist header, group title, section label

**Inline Search**:
A library-scoped search capability embedded in the selected searchable Emby destination. The destination owns the local search control, session, query, result selection, painting, and keyboard/mouse interpretation; the shell owns full-library fetches, recursive album indexing, stale-completion guards, navigation effects, and activation effects. EmbyLibraryContent, MusicContent, or TvContent is the sole owner and painter for the current presentation; TV transfers one snapshot between Narrow and Wide, while an ordinary tab change dismisses search. It is distinct from the cross-library **Search sidebar**.
_Avoid_: global search, Search sidebar, search overlay

**Tab selection**:
The single selected destination in the left panel: Home, EmbyLibrary(index),
AudiobookshelfLibrary(index), or Feeds. Tab positions are count-aware
(Home is 0, Emby N libraries occupy 1..N, Audiobookshelf M libraries follow,
Feeds is always last), preventing Emby and Audiobookshelf sharing the same
numeric position.
_Avoid_: library tab, browse tab, panel tab

**Service browse dispatch / Browse target**:
The exhaustive boundary that maps each left-panel action (keys, mouse, refresh,
rendering, help, context menu) to exactly one of Home, Emby, Audiobookshelf, or
Feeds. Emby-only handlers receive an explicitly selected Emby library rather
than inferring one; there is no "all other tabs are Emby" fall-through. Provider
browse models remain separate and meet only at QueueItem construction and
owner admission (ADR 0018).
_Avoid_: library routing (for this), provider dispatch, generic browse

**Browse level**:
One level in an Emby library navigation stack: parent ID, title, items, total
count, cursor, scroll, optional item-type filter, unplayed flag, sort
criteria, optional letter-range pill, and optional music grouping. Levels are
stacked when drilling into folders, seasons, or shows.
_Avoid_: library level, folder level, nav level

**Surname bucket**:
One fixed alphabetical range of author surnames used as a selectable pill in
an Audiobookshelf book browser, such as A-C or V-Z. Empty ranges are omitted
from the pill row.
_Avoid_: author group (the bucket is a range, not one author)

**Home view**:
The Home tab content: Continue Watching across libraries and per-library Latest
additions. Each section maintains its own cursor and scroll.
_Avoid_: home screen, dashboard, landing

**Library position**:
The saved per-library drill depth, focused item, cursor index, sort, letter
filter, and for feed-view libraries selected group and video cursor/scroll.
Restored across restarts; sticky across launches.
_Avoid_: browse position, library state, saved position

**Idle feed**:
An optional RSS feed URL whose current item title is displayed in the playback
panel when idle and that panel is rendered. In queue-only idle, the playback
panel is hidden, so the title is not displayed. Rotates through items on a
configurable interval.
_Avoid_: idle ticker, RSS ticker, background feed, screensaver feed

**Idle collapse**:
The shell's Queue-visible behavior when fully idle: it removes the card from the
queue visual slot so the Queue list occupies those rows. In queue-only mode it
also removes the in-queue playback panel. Paused playback remains visible, and
a connected-but-not-playing remote session or cast keeps its playback panel.
_Avoid_: idle hide, collapsed queue, empty player

**Playback target**:
Where explicit playback actions are sent: local in-process Player, directly
controlled remote Player owner (via ctrl), observed Emby Session (via
supported remote transport commands only), or an attached cast receiver.
Resolved per action from queue scope, active route, and attachment. Only a
Player owner target carries queue authority; sending to an observed Emby
Session dispatches transport actions without conferring queue or occurrence
identity on that Session.
_Avoid_: play target, output target, active player

## Panels and surfaces

**Panel**:
A root-composed region with its own placement, surface fill, layout, and
Interactive Component. Panels are Tab, Library, Library playback, Queue, Queue
playback, Status bar, and the pane boundaries. **Panel focus** still means which
of Library or Queue receives navigation keys; it is independent of Panel mode.
_Avoid_: active panel, focused panel, pane focus

**Panel mode**:
The app-wide layout state, one of Mini, Narrow, or Wide:
- **Mini**: only one of Library or Queue is visible, per Panel focus. Reached
  either by explicit `x` toggle at any terminal width, or forced when terminal
  width is below the mini-view threshold. Both paths are the same state —
  Mini names "only one panel is showing," not the reason it's showing.
- **Narrow**: both panels visible; a hero-bearing Library panel uses the
  standard fixed-row browser and opens a Library Hero overlay on demand.
- **Wide**: both panels visible; a hero-bearing Library panel uses Wide hero
  when the shared width and minimum-height conditions are met.
_Avoid_: layout mode, view mode, panel state, responsive mode, breakpoint mode

**Zebra stripe**:
The alternating-row secondary background on Wide media lists. Grouping Heading rows and Spacer rows
keep the surface fill outside the sequence, and each group's member rows alternate from the secondary
fill at the group's first member, so a stripe follows a row's position in its group rather than the
screen row it lands on.
A Spacer row between groups sits outside the sequence and keeps the surface fill; a list with no
Heading above a row counts from its first row, which is unstriped. A selected row paints the
selected-row bar instead of its stripe.
_Avoid_: alternating row, striped background, row banding

**Selected-row bar**:
The unconditional selected-row treatment of every canonical media list: while the list holds focus the
whole row paints the `#2d353b` bar fill across its full width — overriding its zebra stripe, its
two-column gutters, and its owning surface — while every span keeps the ordinary unselected
foreground role, so no title is bold and none takes an accent colour. A multi-selected row paints the
bar too, including while the list is unfocused; an unfocused list paints no bar for its cursor row.
_Avoid_: gutter accent, gutter-selected style, selection marker

**Title reveal**:
The closed title-reveal policy of one media list: `Always` (every row paints its full title, the
default) or `OnSelection` (a row outside the list's selection paints only its primary context text,
and the selected row paints the full context-and-title text, marqueed while the list is focused). The
destination that composes the list declares it once; the shared row painter applies it, so no
destination paints or branches its own rows. The podcast episode browser is the only `OnSelection`
list today: the list reads as its parent podcasts at rest and reveals the episode name where the user
is looking.
_Avoid_: hidden title, hover title, selected-only title, marquee mode

**Tab panel**:
The root-composed Panel that paints tab selection and overflow controls for the
Library column and owns their hit geometry.
_Avoid_: tab bar

**Status bar panel**:
The root-composed Panel that paints the status row and its volume, mute, and
remote controls when the Library column is visible.
_Avoid_: status bar

**Library panel**:
The root-composed Panel that owns the shared Wide and Narrow library skeleton.
Destinations supply typed Selector row, List controls row, list, Hero header,
and Workspace content; they do not place or paint those slots. It also owns the
Library Hero overlay lifecycle and its Library-confined placement and hit
geometry.
_Avoid_: destination panel, library screen

**Library Hero overlay**:
A Library-panel-local detail surface opened for a selected hero-bearing browser
row in non-Wide geometry. It is centered within and confined to 90% of the
visible Library pane; in non-Wide geometry that pane is the browser's inset list
box, so the Selector row and the spacer band below it stay outside both the
overlay frame and its dimmed backdrop. It reuses the shared Hero header,
overview (including cast and crew), artwork, and optional Workspace content,
while its provider-link row
remains plain text. It leaves a visible Queue independently operable. Library
focus can dismiss it with Esc or a click on the dimmed Library remainder;
changing destination dismisses it. It is distinct from application popups and
from the four anchored Sidebars.
_Avoid_: application popup, Sidebar, full-window overlay, separate detail block

**Library playback panel**:
The root-composed Panel that paints the playback transport in the Library
column when the Queue column is hidden.
_Avoid_: player strip

**Queue playback panel**:
The root-composed Panel in the Queue column that owns the playback status/target
header, visual slot, and transport when the Queue column is visible. Its header
remains while idle; its visual slot and transport collapse while idle.
_Avoid_: now-playing panel

**Selector row**:
The Library panel slot for one browse pill bar, such as a letter range, group,
bucket, section, or the Feeds watched-filter pills followed by feed groups.
_Avoid_: selector bar

**List controls row**:
The optional Library panel slot for secondary list controls, such as an Emby
home-video count. Feeds does not use this row.
_Avoid_: secondary selector

**Hero header**:
The Wide Library panel slot for selected-item facts and artwork. Its closed
content-derived shapes are **Landscape** (artwork above text), **Portrait** and
**Square** (text beside artwork). The panel derives the shape from the artwork
policy; a destination never chooses an arm.
_Avoid_: hero variant

**Workspace**:
The optional Wide Hero pane slot for one optional header row, one optional Selector row, and
one constituent-item
list, such as tracks, episodes, or chapters. The Library panel owns its box,
surface, and placement.
_Avoid_: detail workspace

**Wide hero**:
The sole Wide arrangement for hero-bearing browse surfaces: the single-column
Library browser and its pills occupy the left pane, and the selected-item hero
or provider-owned detail workspace occupies the right pane. It applies only when
the shared width breakpoint and minimum-height guard are satisfied; otherwise the
standard fixed-row browser remains active and detail opens through the Library
Hero overlay.
_Avoid_: separate detail block, split, side-by-side, hero-on-side, hero-on-left,
hero-on-right

**Hero pane**:
The `#333c43` `SURFACE_RESTING` fill of Wide hero's right pane — the
container surface itself, independent of what is painted inside it.
_Avoid_: recessed box, hero panel, detail panel

**Main content box**:
The `#2d353b` `SURFACE_BACKDROP` inset within a Hero pane or Library Hero
overlay, holding kind-dependent body content at one shared padding
value. It holds overview text and, for a Movie in the Wide Hero pane, the
Cast and crew table in the same box. Distinct from the Hero pane it sits
inside: the pane is the outer container fill, the box is the inner content
inset.
_Avoid_: overview box, recessed box

**Provider-link row**:
The Movie metadata row that joins the names of its declared provider links,
rendered after the release-date, runtime, and genre rows. Each name is plain
text unless the terminal declares hyperlink support and its URL is an `http` or
`https` URL with no control bytes; eligible names use an OSC 8 hyperlink while
remaining in the same row. The row remains plain text in the Library Hero
overlay.
_Avoid_: external-links row, link list, clickable links

**Cast and crew table**:
The Wide Hero pane's table of Movie people inside the Main content box, with
one name column and one shared role column, right-aligned at the box's right edge, and no header or
separator. It lists
every person the server reports: every Director in provider order, then every remaining person in
provider order regardless of type; nothing is capped. An empty role falls back to the
person's provider type. A blank row, then a line of ▁ block characters in sage green, separates the
overview text from the table's first row. The table starts at the box's first content row when there
is no overview, and the box scrolls
when the content exceeds its height.
_Avoid_: credits list, cast list, detail table

**Render Component**:
A `src/app/render/components/` unit that takes a typed content model plus a
`Rect` (and, for Ratatui, a `&mut Buffer`/`Frame`), paints, and computes its
own geometry within that `Rect`. Components consume semantic theme roles or
component style policies — never arbitrary `Color`/`Style` passed in from a
screen.
_Avoid_: component (bare), interactive component, screen, arrangement

**Interactive Component**:
A TuiRealm `AppComponent` under `src/app/components/` that owns one independently
routed surface's local presentation state, input interpretation, updates,
rendering, and geometry, and returns typed requests for work outside its authority.
_Avoid_: component (bare), controller, render component, widget

**Keyboard Router**:
The single keyboard routing authority, in the `UiRoot` Interactive Component
(ADR 0023). It observes every chord regardless of focus and resolves it against
ordered policy to one of ADR 0002's outcomes — `Command`, `Swallow`,
`FallThrough`, or a `Deferred` context-sensitive candidate — where `FallThrough`
lets the focused Interactive Component's own typed request stand. There is
exactly one.
_Avoid_: input handler, dispatcher, context stack, key policy (the policy is
data the router evaluates, not a second router)

**Arbitration fold**:
The one central, pure combination (`arbitrate_key`, ADR 0023 amendment) of the
Keyboard Router's outcome and the focused component's leaf disposition for the
same tick into a final disposition and the requests to dispatch. It owns no
policy and resolves no chord, so it is not a second Keyboard Router. It is the
only place a local claim suppresses a context-sensitive candidate, and the only
place deferred-candidate clocks advance or reset.
_Avoid_: second router, dispatcher, message filter

**Global chord**:
A key combination whose meaning does not depend on which surface is focused, and
which is therefore claimed by the Keyboard Router rather than interpreted by an
Interactive Component. A selection-dependent chord (`.` for the context menu) is
not global even though every destination binds it.
_Avoid_: global key, hotkey, shortcut (bare — a shortcut may be leaf-local)

**`component` state**:
The intermediate interactive-surface ledger state in which an Interactive
Component paints the surface while the shell still mirrors `App` state and/or
legacy input still forwards; `App` teardown remains pending group 5.
_Avoid_: migrated (until the mirror and legacy handler are removed)

**Arrangement**:
A `src/app/render/arrangements/` unit that takes a typed content model plus a
`Rect`, places one or more components, and owns breakpoints and rect
splitting. Arrangements sit between screens and components in the render
dependency order (`screens -> arrangements -> components`).
_Avoid_: layout (bare), component, screen

**Variant**:
A centrally owned, closed visual form whose arm is derived from content or
state available to every caller. A caller-selected arm with only one user is a
defect, not an endorsed pattern: conform that caller or broaden the shared
content type. The Hero header derives Landscape, Portrait, or Square from
artwork policy; the Library panel derives its Wide Hero arrangement from shared
geometry.
_Avoid_: mode (reserved for Panel mode), option

**Policy**:
A centrally owned rule that derives a closed style or behavior choice from
shared content or state. It is not a caller-selected escape hatch: a policy
value with one caller or one user is a defect to remove, not approved
vocabulary. Policies own their geometry and painting consequences.
_Avoid_: config, options, flags (bare)

**Bespoke surface**:
A named Interactive or Render Component needed only after the shared Panel,
slot, arrangement, and content vocabulary genuinely cannot express a surface.
It records the reason and has buffer coverage, but is not a caller-selected
variant arm, exemption, or second composition path.
_Avoid_: one-off, special case, exception

**Sidebar**:
An anchored, full-height destination rendered at a fixed width from one edge
of the window (code: `render_panel_shell_at`, whose rect is literally named
`sidebar`) — distinct from a **popup**, a centered dimmed-backdrop overlay
(code: `render_modal_frame`) such as the Feeds-management, multiselect,
library-routes, save-playlist, and confirm dialogs. The four sidebars: Search
sidebar, Settings sidebar, Sessions sidebar, Help sidebar. Feeds management is
a popup nested inside the Settings sidebar, not a sidebar itself — it only
exists while Settings is open.
_Avoid_: panel (reserved for Library/Queue), screen, overlay (bare), full-window destination

**Search sidebar**:
The global cross-library search surface filtering to navigable media types
(Series, Episode, Season, Movie, Audio, MusicAlbum, MusicArtist), with optional
type filter pill and per-query result deduplication.
_Avoid_: library search, global search (bare), omnibox

**Watched filter**:
The All / Played / Unplayed selector in the Feeds tab (`w` key). Filters feed
entries by their played flag. Audiobookshelf podcast browsing has an analogous
All / Played / Unplayed episode filter.
_Avoid_: played filter, hide watched, unwatched filter

## Audiobookshelf

**Podcast**:
An Audiobookshelf podcast show or one of its downloaded episodes. The word never means an Emby library: Emby has no podcast feature, and a Channel or `podcasts` collection library from an Emby addon receives only generic Emby handling with no podcast behavior.
_Avoid_: Emby podcast, Emby channel, podcast library (bare), podcast channel

**Audiobookshelf library**:
One Audiobookshelf library exposed as a peer tab, resolved once into a
podcast kind or a book kind at tab selection. Book and Audiobookshelf podcast libraries
interleave as peer tabs in the server's `/api/libraries` order, exactly as
Emby libraries do; no type-partitioning or reordering. Identity is Service
kind + library ID.
_Avoid_: ABS library, audiobookshelf collection, podcast library (as kind)

**Downloaded podcast episode**:
An Audiobookshelf podcast episode available as downloaded media, identified by
its `libraryItemId` and `episodeId`. It is distinct from an RSS FeedEntry and
from a remote podcast episode Audiobookshelf has not downloaded.
_Avoid_: feed episode, podcast item, track

**Audiobookshelf show**:
One podcast show (series) inside an Audiobookshelf podcast library, identified
by Service kind + `libraryItemId`. Holds title, author, cover path, and a
paged list of downloaded episodes.
_Avoid_: podcast, show item, ABS show

**Audiobookshelf book**:
One audiobook inside an Audiobookshelf book library, identified by Service
kind + `libraryItemId` only — books have no episode identity. Carries title,
the raw author credit (`author_display`) and its first-listed-author surname
sort key (`author_sort_key`, via `human_name`, falling back to the raw credit),
cover path, and a book-relative `chapters[]` / `audioFiles` detail. Queueing
projects the whole book as one item and one continuous mpv timeline across its
audio files; chapter rows seek absolutely against that merged timeline.
_Avoid_: audiobook item, book episode, track

**Audiobook chapter**:
One book-relative seekable range `{start, end}` in seconds across the whole
book timeline, as Audiobookshelf's `chapters[]` reports it (it may span audio
files). mbv renders each as a first-class row and issues one absolute seek to
`start` on the merged timeline on activation.
_Avoid_: track, segment, file part

**AudiobookshelfBookQueueItem**:
The QueueItem snapshot of a book: content identity, presentation, progress,
completion, and Service-scoped artwork identity keyed by `libraryItemId` only
— a sibling of `AudiobookshelfQueueItem`, never an `episode_id` optional. It
carries no credential, server URL, playback-session ID, resolved source, or
request headers, matching the episode item's redaction boundary.
_Avoid_: ABS book item, audiobook episode, book queue entry

**AudiobookshelfQueueItem**:
The QueueItem snapshot of a downloaded podcast episode. It carries content
identity, presentation, progress, completion, and Service-scoped artwork
identity, but no credential, server URL, playback-session ID, resolved source,
or request headers. Currently eligible only for bare-mode owners with
Audiobookshelf setup and credential (Local daemon and mbvd eligibility is
milestone #524 — issues #525-528).
_Avoid_: Audiobookshelf episode, ABS item, feed entry

**Audiobookshelf playback session**:
Ephemeral Audiobookshelf lifecycle state opened to resolve and play one episode
or one book and synchronize its progress. It is neither an Emby Session nor an
mbv Playback run. Created just in time for the active slot; close is bounded and
finalized before next session opens. Monotonic wall-clock listening time is
accumulated only while not paused. Episode sessions are keyed by
`libraryItemId` + `episodeId`; book sessions by `libraryItemId` only.
_Avoid_: session, playback run, Emby session

## Feeds

**Feeds Service / Feeds tab**:
mbv's built-in Feeds Service and its Feeds tab, which manage RSS/Atom
FeedSubscriptions and their FeedEntries. This is distinct from an Emby
homevideos feed view.
_Avoid_: feed view, Emby feed, homevideos feed

**Emby homevideos feed view**:
The grouped, feed-like browse surface of an Emby homevideos library, such as a
configured YouTube tab. It is distinct from the Feeds Service/tab;
`restore-feed-group-inline-expansion` concerns this Emby homevideos feed view,
not the Feeds Service tab.
_Avoid_: Feeds tab, Feeds Service, RSS subscription

**FeedSubscription**:
A user's subscription to one RSS/Atom feed: display name, URL, and FeedKind.
Stored per-user in local `config.toml` as `[[feeds]]`; fetched entries and
fetch metadata are not persisted. Editing never changes the URL — a changed URL
is a new subscription. YouTube channel URLs are normalized to RSS on subscribe.
_Avoid_: feed config, channel, subscription config

**FeedKind**:
The Audio | Video classification of a FeedSubscription. Inferred from
enclosure MIME types when available (mixed or absent defaults to Video) and
editable by the user. It governs queue admission for entries that carry no
MIME type of their own.
_Avoid_: feed type, media type, category

**FeedEntry**:
One parsed item from a subscribed feed. Identity is guid, else enclosure-URL
hash, else title+pub-date hash. Carries enclosure URL, link, mime type,
pub_date, duration in Emby ticks, description, and the normalized subscription
URL as `feed_id` — the stable identity used by local feed-entry state.
Each entry carries local `position_ticks` and `played` playback state, restored
from the local state directory and updated on playback lifecycle events (stop,
pause, seek, EOF). Entries never report progress to Emby. Queued FeedEntries are
owned snapshots: deleting the subscription leaves them playable for the lifetime
of the Bound queue.
_Avoid_: episode, post, feed item, rss item

## Remote sessions

A client can also reach *another* device's playback, discovered through Emby
rather than through this project's own local-daemon substrate. This is a
distinct relationship from Attach above, even though both involve one process
reaching a Player owner over a socket.

**Session**:
An Emby-tracked record that some device is playing something. Exists
independently of mbv — including for non-mbv devices — and is what the
Sessions sidebar lists. Emby Sessions may advertise a private-LAN control port
in `supported_commands` (`mbv-direct-tcp-port`).
_Avoid_: connection, stream, remote instance

**Session watch**:
A client observing another device's generic Emby Session read-only with respect
to mbv's queue: it may display directly observed position and title and may
dispatch supported remote transport commands (play, pause, seek, stop, next,
previous, volume, mute, stream selection), but it never mutates mbv queue
membership, ordering, slot snapshots, cursor, or playlist state, and it never
infers queue position, occurrence identity, completion, or consume from that
Session. The fallback when Direct remote control to that device isn't
available.
_Avoid_: attach, session attach, monitor, remote session (bare), tracked session, tracking

**Direct remote control**:
A client has its own control-socket connection to another device's Player
owner, giving the same queue management as a local session — reorder,
remove, play next, all of it. This is what the aqua queue-scope pill
indicates.
_Avoid_: green pill, remote takeover, queue management (alone)

**Queue scope**:
Local or Remote — whether the queue on the controlling terminal's local side or
the directly controlled remote Player owner's queue is currently shown in the
queue panel. Exists during Sessions-sidebar Direct remote control and explicit
remote daemon attachment, not Session watch or a Library route. Remote scope is
only selectable when a direct remote queue exists; otherwise Local is forced.
_Avoid_: split view, pill state

**Library route**:
A persistent per-library assignment sending that library's playback to a
chosen device, set from the library-routing picker rather than the Sessions
sidebar. Stored as `lowercased name -> tcp://host:port` endpoint (device name is
transient in the picker, immediately resolved to an endpoint before persisting).
Independent of Session watch / Direct remote control — connecting via the
Sessions sidebar tears down an active route first. F2 Settings manages routes;
hand-editing `config.toml` is supported. Malformed non-`tcp://` values are
logged and skipped.
_Avoid_: routing, daemon route

**Home daemon**:
The Local daemon a stay-alive client falls back to once Direct remote
control or a Library route ends. A bare-mode client has no home daemon;
ending remote control there resumes its own in-process Player directly.
_Avoid_: home base, origin daemon

## Cast

**Cast receiver**:
A Google Cast device discovered on the LAN via mDNS and presented as a row
in the F3 target panel alongside Emby sessions, identified durably by its
advertised identifier across address changes rather than a stored host/port.
Called a **cast target** once mbv is attached to it — the receiver in its
role as where playback actions are sent.
_Avoid_: chromecast device, TV, cast device

**Cast attachment**:
mbv's relationship to a cast receiver it controls: selecting one attaches
mbv without starting, stopping, or reconfiguring the local Player, and while
attached, playing a selection dispatches it to the receiver instead of local
playback. Held beside `connected_session_id`, independent of Emby session
state — both may be attached at once. Distinct from Attach above: it is not
a ctrl connection and creates no Client relationship, even though both name
"one process reaching a playback target." On launch, with `auto_reconnect`
enabled, mbv attaches to the receiver it was attached to at exit, restoring
control and displayed state from its reported status without dispatching
anything.
_Avoid_: attach (bare, without "cast"), cast session, cast connection

**Dispatch**:
Sending a played selection's items to a cast receiver's own media queue in
one act; mbv does not project, track, or reconcile that queue afterward, and
the receiver performs its own advancement between dispatched items.
Distinct from Bound (a Player owner holding mbv's own queue) — a receiver
owns what it plays independently of mbv's queue once dispatch completes.
_Avoid_: enqueue, cast queue, load (bare)

**Uncastable**:
An item mbv's queue holds that cannot be sent to a cast receiver — no
retrievable media URL, a credential that can only be carried in a request
header, or (for Audiobookshelf) a multi-file book whose position cannot be
represented on a receiver without corrupting its stored resume point.
Reported to the user by name and reason rather than substituted or dropped
silently. Distinct from Unplayable item, which never enters a Player
owner's queue at all.
_Avoid_: unplayable (cast), unsupported item, skip reason
