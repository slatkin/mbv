# mbv

A terminal client for Emby, Audiobookshelf, and Feeds that browses catalogs and
plays media. Playback may run inside the terminal process itself, or be hosted
by a background process on the same machine so it survives the terminal closing.

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
refused). Planned Client fall-through for explicitly requested non-audio items
is tracked in issue #431 and ADR 0017.
_Avoid_: audio daemon, headless audio owner, mbvd audio mode

**Playback run**:
The local mpv playback loop — one per mpv invocation, owned by a Player owner.
Distinct from Session (the Emby-tracked record that exists independently of
mbv) and from an Audiobookshelf playback session. The owner's canonical queue
remains authoritative whether mpv mirrors it eagerly or materializes only its
active file (active-file projection is used once a lifecycle-backed source such
as an Audiobookshelf episode or book enters the run).
_Avoid_: session, playback session

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
never edits anything on the server. Applies the same way whether playback ran
in this process, on a Local daemon, or on another device's Session. Addresses
canonical slot identity; removes only the consumed occurrence.
_Avoid_: auto-remove, playlist consume, consume-and-save

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

## Browsing and tabs

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
An optional RSS feed URL displayed in the playback panel when idle. Rotates
through items on a configurable interval.
_Avoid_: idle ticker, background feed, screensaver feed

**Playback target**:
Where explicit playback actions are sent: local in-process Player, directly
controlled remote Player owner (via ctrl), or Emby session (via observed remote
playback). Resolved per action from queue scope, active route, and attachment.
_Avoid_: play target, output target, active player

## Presentation

**Panel focus**:
Which of Library or Queue currently receives navigation keys. Independent of
Panel mode.
_Avoid_: active panel, focused panel, pane focus

**Panel mode**:
The layout mode cycled with `x`: Both (Library and Queue visible), LibraryOnly,
or QueueOnly. LibraryOnly forces Panel focus to Library; QueueOnly forces it to
Queue.
_Avoid_: layout mode, view mode, panel state

**Wide mode / Narrow mode**:
The right panel's responsive width states, chosen from the width available to
that panel. Narrow is not a distinct arrangement: it is always hero-on-top with
a single-column list. Wide uses either Hero-on-top or Hero-on-left, fixed per
surface. Distinct from Panel mode, which is app-wide.
_Avoid_: responsive mode, view mode, layout mode, breakpoint mode

**Hero-on-top**:
The wide arrangement used by movies, shows, podcasts, feeds, and home videos:
hero above the list, list in two columns.
_Avoid_: stacked, two-column mode, dual column

**Hero-on-left**:
The wide arrangement used by Home, music, and audiobooks: hero beside the list,
list in a single column.
_Avoid_: split, side-by-side, hero-on-side

**Search sidebar**:
The global cross-library search surface filtering to navigable media types
(Series, Episode, Season, Movie, Audio, MusicAlbum, MusicArtist), with optional
type filter pill and per-query result deduplication.
_Avoid_: library search, global search (bare), omnibox

**Watched filter**:
The All / Watched / Unwatched selector in the Feeds tab (`w` key). Filters feed
entries by their played flag. Audiobookshelf podcast browsing has an analogous
All / Played / Unplayed episode filter.
_Avoid_: played filter, hide watched, unwatched filter

## Audiobookshelf

**Audiobookshelf library**:
One Audiobookshelf library exposed as a peer tab, resolved once into a
podcast kind or a book kind at tab selection. Book and podcast libraries
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
URL as `feed_id` — the stable identity used by the shared feed-entry store.
Each entry also carries roaming `position_ticks` and `played` fields hydrated
from the shared feed-entry store on refresh and updated on playback lifecycle
events (stop, pause, seek, EOF). Entries never report progress to Emby. With no
shared-data daemon, entries degrade to zero position and unplayed without
blocking browsing or playback. Queued FeedEntries are owned snapshots: deleting
the subscription leaves them playable for the lifetime of the Bound queue.
_Avoid_: episode, post, feed item, rss item

## Shared data and roaming

**Shared data**:
The optional, opt-in durable roaming of mbv-owned state through the packaged
mbvd's redb database. Hosting is enabled on the daemon side; use is enabled by
an explicit endpoint on each client. Shared-data endpoint is independent of
ctrl and library routes. Isolated per Emby user, transport limited to
loopback/private-network TCP or Unix socket (WAN rejected before any credential
is sent), and optional TLS where the client validates the certificate before
sending its Emby token.
_Avoid_: sync, cloud sync, roaming service, shared state (bare)

**Shared document / Roaming document**:
One of the four revisioned documents roamed per user: Queue state, Library
position state, Last remote connection, and Roaming settings. Each has an
independent monotonic revision; writes use compare-and-swap — stale expected
revisions are rejected and the winner is adopted with a toast.
_Avoid_: shared file, roaming file, synced document

**Roaming settings**:
The exactly two settings that roam across machines: `auto_reconnect` and
`library_routes`. Stored in the shared database and mirrored locally via
`roaming_settings.json`; never written to `config.toml`. When shared or its
local mirror is active, shared values override explicit local config values
with a once-per-connection conflict log.
_Avoid_: roaming config, synced settings, shared library routes

**Feed entry state**:
Per-entry playback state keyed by `(user_id, feed_id, entry_guid)` in the
shared-database `feed_entry_state` table. Each row holds `position_ticks` and
`played`. Writes are last-write-wins (no CAS); reads support prefix scan by
`feed_id`. Negotiated as an additive `shared-mbv-feed-entry-state-v1`
capability; older daemons without it degrade to local-only feed state.
_Avoid_: feed progress, episode state, feed resume state

**Shared-data fallback / Local mirror**:
The local-filesystem mirrors of shared documents (and feed entry table) used
when shared data is unavailable. A client restores from local state, shows one
fallback toast, and retries with bounded exponential backoff. On reconnect,
existing shared documents replace divergent fallback values without prompting;
an absent shared document is first-writer initialized from local state.
Database open/corruption/serialization/disk-full failures disable hosting or
the operation without stopping playback or corrupting committed data.
_Avoid_: offline cache, local backup, sync fallback

## Remote sessions

A client can also reach *another* device's playback, discovered through Emby
rather than through this project's own local-daemon substrate. This is a
distinct relationship from Attach above, even though both involve one process
reaching a Player owner over a socket.

**Session**:
An Emby-tracked record that some device is playing something. Exists
independently of mbv — including for non-mbv devices — and is what the
Sessions panel lists. Emby Sessions may advertise private-LAN ctrl and shared-
data ports in `supported_commands` (`mbv-direct-tcp-port`, `mbv-shared-data-
tcp-port`).
_Avoid_: connection, stream, remote instance

**Session watch**:
A client observing another device's Session read-only — position and title
only, no queue control. The fallback when Direct remote control to that
device isn't available.
_Avoid_: attach, session attach, monitor, remote session (bare)

**Direct remote control**:
A client has its own control-socket connection to another device's Player
owner, giving the same queue management as a local session — reorder,
remove, play next, all of it. This is what the aqua queue-scope pill
indicates.
_Avoid_: green pill, remote takeover, queue management (alone)

**Queue scope**:
Local or Remote — whether the queue on the controlling terminal's local side or
the directly controlled remote Player owner's queue is currently shown in the
queue panel. Exists during Sessions-panel Direct remote control and explicit
remote daemon attachment, not Session watch or a Library route. Remote scope is
only selectable when a direct remote queue exists; otherwise Local is forced.
_Avoid_: split view, pill state

**Library route**:
A persistent per-library assignment sending that library's playback to a
chosen device, set from the library-routing picker rather than the Sessions
panel. Stored as `lowercased name -> tcp://host:port` endpoint (device name is
transient in the picker, immediately resolved to an endpoint before persisting).
Independent of Session watch / Direct remote control — connecting via the
Sessions panel tears down an active route first. F2 Settings manages routes;
hand-editing `config.toml` is supported. Malformed non-`tcp://` values are
logged and skipped.
_Avoid_: routing, daemon route

**Home daemon**:
The Local daemon a stay-alive client falls back to once Direct remote
control or a Library route ends. A bare-mode client has no home daemon;
ending remote control there resumes its own in-process Player directly.
_Avoid_: home base, origin daemon
