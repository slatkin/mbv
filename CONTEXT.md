# mbv

A terminal client for Emby that browses a library and plays media. Playback may
run inside the terminal process itself, or be hosted by a background process on
the same machine so it survives the terminal closing.

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
becomes available.
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
Changing a remote Service to a different server. It invalidates all queued and
persisted state belonging to the previous server.
_Avoid_: reconnect, migration, account switch

**Service removal**:
Deleting a Remote Service's setup, credential, and local state and returning it
to Not configured.
_Avoid_: logout, disconnect, disable

**Service credential**:
A secret issued by Emby or Audiobookshelf that authorizes mbv to that Service.
It belongs only to that Service.
_Avoid_: account credential, mbv token, control token

**Control credential**:
An mbv-owned secret used by a Local daemon to admit Clients independently of all
Service credentials. It grants control access, not an identity or login;
packaged mbvd does not use this mechanism yet.
_Avoid_: Emby token, API key, login token

**Service-owned state**:
Local state whose remote-native identity is meaningful only for one Remote
Service setup. Authentication repair preserves it; Service replacement or
removal invalidates it.
_Avoid_: provider cache, account state

## Playback ownership

**Player owner**:
The single process on a machine that holds the audio device, Bound queue, and
Service-specific playback lifecycle. Exactly one exists per user at a time.
_Avoid_: instance, master, host

**Bare mode**:
The default presentation, where one process is both the terminal UI and the
Player owner. Closing it stops playback.
_Avoid_: foreground mode, standalone, normal mode

**Stay-alive**:
The mode in which playback is hosted by a local daemon rather than the terminal
UI, so playback continues after every terminal window closes.
_Avoid_: daemon mode, background mode, alive mode, persistent mode

**Playback run**:
The local mpv playback loop — one per mpv invocation, owned by a Player owner.
Distinct from Session (the Emby-tracked record that exists independently of
mbv) and from an Audiobookshelf playback session. The owner's canonical queue
remains authoritative whether mpv mirrors it or materializes only its active
file; the run holds mpv and per-item lifecycle state.
_Avoid_: session, playback session

## Processes

**Local daemon**:
The Player owner in stay-alive mode: a user-owned background process on the same
machine as its clients, holding no terminal. One exists per user.
_Avoid_: relay, backend, server, session host

**mbvd**:
The separately packaged daemon, run as a system service, with its own
configuration, state, and socket. A different product surface from the local
daemon, never started by a terminal UI.
_Avoid_: system daemon, the daemon

**Client**:
A terminal UI that reaches an out-of-process Player owner over ctrl. Attachment
does not log it into the owner or establish a Service identity. Any number may
attach at once, and each is disposable.
_Avoid_: thin client, terminal client, viewer, attachment

**Tray**:
The desktop status icon belonging to the Player owner, giving playback controls
and a stop action while no client is on screen. Only present in stay-alive mode.
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
_Avoid_: reattach, connect, resume, take over

## Queue

**Queue slot**:
One independently addressable occurrence of a QueueItem in a canonical queue.
Two slots may contain the same content while retaining distinct slot identities.
_Avoid_: queue item, content ID, playlist index

**Content identity**:
The Service-qualified identity of media content, distinct from the identity of
each Queue slot containing it.
_Avoid_: item ID, queue ID, slot ID

**Consume**:
Removal of an item from the queue once it finishes playing, as in ncmpcpp.
Purely a queue operation — it says nothing about where the queue came from and
never edits anything on the server. Applies the same way whether playback ran
in this process, on a Local daemon, or on another device's Session.
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
owner's rules — an audio-only owner does not hold items it cannot play. Bound
does not mean playing: a stopped owner still holds its queue, and queues can be
Bound to two owners at once while only one of them plays.
_Avoid_: active queue, live queue, running queue, attached queue

**Unplayable item**:
An item a Player owner lacks the capability to play, because of media kind,
required Service availability, or playback support. It never enters that
owner's queue: a controlling Client strips it before submitting, and the owner
discards any that reach it regardless.
_Avoid_: rejected item, filtered item, invalid item, blocked item

**EmbyItem**:
The queue's Emby-side item type — the serialized record of an Emby library
item. Renamed from MediaItem; the rename is wire-invisible because serde
field names are unchanged. Positions for EmbyItems report to the Emby API.
_Avoid_: MediaItem, media item, emby entry

**QueueItem**:
The queue's media snapshot — an EmbyItem, FeedEntry, or
AudiobookshelfQueueItem. Generic queue operations use shared presentation and
identity behavior; Service-specific admission, source preparation, lifecycle,
progress, and cleanup remain explicit boundaries.
_Avoid_: queue entry, playable, mixed item

## Audiobookshelf

**Downloaded podcast episode**:
An Audiobookshelf podcast episode available as downloaded media, identified by
its `libraryItemId` and `episodeId`. It is distinct from an RSS FeedEntry and
from a remote podcast episode Audiobookshelf has not downloaded.
_Avoid_: feed episode, podcast item, track

**AudiobookshelfQueueItem**:
The QueueItem snapshot of a downloaded podcast episode. It carries content
identity, presentation, progress, completion, and Service-scoped artwork
identity, but no credential, server URL, playback-session ID, resolved source,
or request headers.
_Avoid_: Audiobookshelf episode, ABS item, feed entry

**Audiobookshelf playback session**:
Ephemeral Audiobookshelf lifecycle state opened to resolve and play one episode
and synchronize its progress. It is neither an Emby Session nor an mbv Playback
run.
_Avoid_: session, playback run, Emby session

## Feeds

**FeedSubscription**:
A user's subscription to one RSS/Atom feed: display name, URL, and FeedKind.
Stored per-user in local `config.toml`; fetched entries and fetch metadata are
not persisted. Editing never changes the URL — a changed URL is a new
subscription.
_Avoid_: feed config, channel, subscription config

**FeedKind**:
The Audio | Video classification of a FeedSubscription. Inferred from
enclosure MIME types when available (mixed or absent defaults to Video) and
editable by the user. It governs queue admission for entries that carry no
MIME type of their own.
_Avoid_: feed type, media type, category

**FeedEntry**:
One parsed item from a subscribed feed. Identity is guid, else enclosure-URL
hash, else title+pub-date hash. Carries enclosure URL, link, pub_date,
duration in Emby ticks, description, and the normalized subscription URL as
`feed_id`. Each entry also carries roaming `position_ticks` and `played`
fields hydrated from the shared feed-entry store on refresh and updated on
playback lifecycle events (stop, pause, seek, EOF). Entries never report
progress to Emby. With no shared-data daemon, entries degrade to zero
position and unplayed without blocking browsing or playback. Queued
FeedEntries are owned snapshots: deleting the subscription leaves them
playable for the lifetime of the Bound queue.
_Avoid_: episode, post, feed item, rss item

## Remote sessions

A client can also reach *another* device's playback, discovered through Emby
rather than through this project's own local-daemon substrate. This is a
distinct relationship from Attach above, even though both involve one process
reaching a Player owner over a socket.

**Session**:
An Emby-tracked record that some device is playing something. Exists
independently of mbv — including for non-mbv devices — and is what the
Sessions panel lists.
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
remote daemon attachment, not Session watch or a Library route.
_Avoid_: split view, pill state

**Library route**:
A persistent per-library assignment sending that library's playback to a
chosen device, set from the library-routing picker rather than the Sessions
panel. Independent of Session watch / Direct remote control — connecting via
the Sessions panel tears down an active route first.
_Avoid_: routing, daemon route

**Home daemon**:
The Local daemon a stay-alive client falls back to once Direct remote
control or a Library route ends. A bare-mode client has no home daemon;
ending remote control there resumes its own in-process Player directly.
_Avoid_: home base, origin daemon
