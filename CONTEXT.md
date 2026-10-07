# mbv

mbv is a terminal client for Emby, Audiobookshelf, and Feeds. It browses catalogs and plays media. Every local launch attaches to the per-user Owner process. That process is the only local Player-owner host. Stay-alive controls whether that process outlives its Clients.

## Services

Service:
One of the singleton media integrations in mbv. The three kinds are Emby, Audiobookshelf, and Feeds. Each kind exists at most one time in mbv. Feeds is always present, even with no subscriptions.
_Avoid_: account, provider, backend

Remote Service:
An Emby or Audiobookshelf Service at a configured server. It uses its own Service credential for authorization.
_Avoid_: account, remote provider, backend

Service setup:
The act that establishes a Remote Service. It validates the server and the Service credential with success. mbv starts without Service setup.
_Avoid_: app login, account creation, onboarding

Service-independent startup:
The guarantee that mbv shows its TUI before any Remote Service authenticates. Each configured Remote Service starts on its own after the first frame. Failure of one Service does not delay another Service. Feeds needs no Remote Service.
_Avoid_: no-auth mode, offline mode, provider mode

Services view:
The Settings surface for Remote Service setup and feed subscriptions. mbv opens it at start when no Remote Service is configured and Feeds has no subscriptions.
_Avoid_: login screen, setup wizard, authentication gate

Service state:
The current availability of a Remote Service. The five values are Not configured, Connecting, Ready, Needs authentication, and Unavailable. Unavailable keeps stored credentials. Needs authentication means the Remote Service rejected the credentials.
_Avoid_: login state, account state, online status

Service replacement:
The change of a Remote Service to a different server. It invalidates queued and stored state that belongs only to the prior setup. It preserves unrelated state in mixed queues, for example Feed entries and items of another Service.
_Avoid_: reconnect, migration, account switch

Service removal:
The deletion of a Remote Service setup, credential, and local state. The Service returns to Not configured. Only state owned by that Service is purged. Queued and stored media of other Services remain.
_Avoid_: logout, disconnect, disable

Service credential:
A secret from Emby or Audiobookshelf that authorizes mbv to that Service. It belongs only to that Service. mbv stores it in a per-Service secret file with mode 0600. It never lives in `config.toml`.
_Avoid_: account credential, mbv token, control token

Control credential:
A secret owned by mbv. The Owner process uses it to admit Clients. It works apart from all Service credentials. It grants control access. It grants no identity and no login. Packaged mbvd does not use it yet. It still uses legacy Emby-token ctrl authentication. It will migrate to filesystem and trusted-LAN authorization under issue #523.
_Avoid_: Emby token, API key, login token

Service-owned state:
Local state with a remote-native identity that is valid only for one Remote Service setup. Repair of authentication preserves it. Replacement or removal of the Service invalidates only state owned by that Service. It preserves unrelated Services and Feeds.
_Avoid_: provider cache, account state

Setup generation:
A per-Service counter that rises in order. It guards late async setup completions. Every replace, retry, and setup attempt raises it. A completion applies only when its generation matches the current run.
_Avoid_: setup version, auth generation, connection ID

Owner admission / Service eligibility:
The rule by which a Player owner accepts a QueueItem into its Bound queue. It tests media kind, that is audio against video. It tests whether the required Remote Service setup and credential are loaded in that owner process. It tests whether ctrl peers negotiated transport for that item kind. The local Owner process and packaged mbvd admit Emby and Feed today. Audio-only owners admit only the audio subset of a mixed submission. Audiobookshelf owner admission is tracked in milestone #524.
_Avoid_: owner capability, queue capability, supported kinds

## Playback ownership

Player owner:
The one process on a machine that holds the audio device. It holds the Bound queue and the Service playback lifecycle. Exactly one exists per user at one time. Different owner kinds admit different Services.
_Avoid_: instance, master, host

Out-of-process owner:
A Player owner outside the terminal application process. The Owner process and packaged mbvd are examples. The client reaches it over ctrl. The term names the process that holds playback. It does not name the machine. The Owner process is always on this machine and counts as on this machine. Only TCP or Unix endpoints point elsewhere (`player-target-locality`).
_Avoid_: remote owner (bare), background owner, external player

Bare mode:
Retired term. Every local terminal launch is a Client of the Owner process.
_Avoid_: foreground mode, standalone, normal mode, Bare mode

Stay-alive:
The lifetime policy for the Owner process. It decides whether the Owner process outlives its Clients. When enabled, the Owner process is a background process. It remains after the last Client closes. When disabled, no background process or service exists. The Owner process is part of the running mbv app and ends with it.
_Avoid_: daemon mode, background mode, alive mode, persistent mode

Audio-only owner:
A Player owner started with `--audio-only`. Packaged mbvd ships in this form. It plays only audio. It never holds a video item. It accepts a mixed submission without the non-audio items. It refuses a fully non-audio submission. When a Client plays a video through an eligible ctrl attachment or a controlled Emby session, mbv prompts with the owner and the selection named. Confirmation stops the owner, ends the attachment, and plays locally. Decline changes nothing. The local Owner process is never audio-only. The shipped behavior is described in `openspec/changes/archive/2026-09-16-play-locally-when-owner-cannot`.
_Avoid_: audio daemon, headless audio owner, mbvd audio mode

Playback run:
The local mpv playback loop. One run exists per mpv invocation. A Player owner owns it. It differs from Session. Session is the Emby-tracked record and exists apart from mbv. It differs from an Audiobookshelf playback session. The owner canonical queue stays authoritative. It stays authoritative whether mpv mirrors it at once or loads only the active file. Active-file projection applies once a lifecycle-backed source enters the run, for example an Audiobookshelf episode or book.
_Avoid_: session, playback session

Clocked audio output:
The inherited Playback-run output of packaged mbvd. The mpv `audio-device` property binds to a real ALSA endpoint. Hardware paces playback. It differs from legacy PCM pipe output. That output writes untimed PCM through the mpv `ao=pcm` file writer into a FIFO. An external consumer such as Snapserver reads the FIFO.
_Avoid_: ALSA mode, direct audio, hardware output

mpv script set:
The entry script `mbv.lua` and its sibling Lua fragments. They build the mpv on-screen control overlay (OSC) and the Next-Up prompt. Exactly one copy is live per playback run. It is the copy of the running build. It is never a leftover from the removed installer, a prior version, or another build. It differs from mpv configuration and user scripts. mbv never reads or edits those.
_Avoid_: OSC bundle, overlay bundle, Lua scripts (bare), script folder

Resolved script path:
The one script-set path handed to mpv. One rule selects it. Use the checkout copy of the running build (`<checkout>/scripts/mbv.lua`) when it exists. Use the installed copy at `/usr/share/mbv/scripts/mbv.lua` otherwise. The removed installer user-directory path is never a candidate. An unused copy there appears by name in a startup warning. It is left untouched. The font directory follows the same rule. Scripts and fonts always come from the same source. Startup reports the resolved path.
_Avoid_: script source (bare), active script, script lookup

## Processes

Owner process:
The part of the mbv app that is the Player owner for every terminal UI on this machine. Every such UI is a Client. The Owner process stays the local playback authority across route switches. With Stay-alive disabled, it is part of the running app. It is not a background process or service. With Stay-alive enabled, it is a background process. It survives its last Client.
_Avoid_: local daemon, home daemon, daemon, session, background service, relay, backend, server

Stay-alive process:
_Avoid_: Stay-alive process

mbvd:
The separately packaged headless server daemon. It runs as a system service, a systemd unit, often on a headless server with no desktop session. It has its own configuration, state, and socket. Users reach it from an already running mbv. They use the F3 Sessions sidebar. That path matches control of an Emby Session device (Direct remote control). It is never the path that starts mbv. It has no Tray, no pinned panel, and no other desktop or Client function. Its limits never limit a Local process. It is a different product surface from the Packaged Player owner. No terminal UI starts it. On `main` it is still Emby-gated. It constructs `EmbyClient` without condition, requires cached credentials at start, and uses legacy Emby-token ctrl authentication. Service-independent startup (zero Services), Feed playback without Emby, and optional Emby runtime live in open PR #529 for issue #523. Filesystem and trusted-LAN authorized ctrl and `mbvd --connect emby` administration also live there. Do not describe them as landed on `main`.
_Avoid_: system daemon, the daemon

Client:
A terminal UI attached to an out-of-process Player owner over ctrl. Local Clients attach to the Owner process. Attachment establishes no Service identity. Any number can attach when Stay-alive is enabled. With it disabled, admission is exclusive.
_Avoid_: thin client, terminal client, viewer, attachment

Local process:
Any mbv process on the user machine. It is a Client or the Owner process. Control of mbvd or another device from F3 happens inside an already running Local process. Its Owner process and Tray keep running. Desktop features belong to Local processes and never to mbvd. The Tray and the pinned panel are examples.
_Avoid_: client side (of mbvd), frontend, desktop daemon

Tray:
The desktop status icon shown by a Local process. It gives playback controls and a stop action while no Client is on screen. It is never the property of mbvd. mbvd is headless. For the local Owner process, it appears while Stay-alive is enabled or the `Show systray icon` preference is on. It follows those settings live.
_Avoid_: systray, status icon, indicator, mbvd tray

Player endpoint:
The address used to reach a Player owner control socket. Local means the Owner process on this machine. A network address points at a remote owner. A remote owner is another machine Player owner or an mbvd. The user reaches it from the F3 Sessions sidebar or a Library route of a running mbv. mbvd is a daemon. The Owner process is not a daemon, on this machine or any other.
_Avoid_: daemon endpoint, connection string, remote address, socket path

## Continuity

Playback continuity:
The guarantee that enabled Stay-alive gives. What plays, the queue, and position survive every Client close and reopen.
_Avoid_: persistence, session continuity

Session continuity:
Preservation of on-screen client state across close and reopen. Cursor, scroll, open overlays, and search are examples. mbv deliberately does not offer it. Only playback continuity exists.
_Avoid_: terminal continuity, UI state

TUI launch state:
The one exit snapshot that a completed TUI session leaves. It is the start location for the next launch. A TUI loads the saved snapshot one time at startup. It keeps its own launch-state changes in memory while it runs. It replaces the saved snapshot only as part of orderly exit. A UI-state reset clears it outside exit. Cursor, tab, pill, item, focus, refresh, and rendering activity never write it while open. When two TUIs diverge in memory and exit in sequence, the last completed exit wins. No Client identity, merge, or daemon synchronization applies. An attached Client restores the same bounded snapshot. The snapshot holds exactly four things. They are the selected tab and the selected main Selector pill for that tab. They are the selected library item for that pill and whether Library or Queue held Panel focus. The library item uses stable identities where the destination supplies them. It never uses presentation indices. The snapshot holds no state for unselected tabs. It holds no Queue selection and no nested Workspace selector such as a TV season. It holds no overlay or Sidebar and no search query or result. It holds no multi-selection or Visual mode, no scroll offset, and no other transient presentation state. It is a bounded launch location. It is not Session continuity. Unlike Library position, it records only the selected tab. It never records per-library state for unselected tabs.
_Avoid_: session state, saved session, launch preferences

UI-state reset:
The global F2 Settings action labelled `Reset UI State`. It returns all presentation and interaction state of the running TUI to defaults. It acts as a whole and not only on the focused Panel. Every retained destination returns to its root scope and default sort, filter, pill, selection, scroll, and expansion. Queue local cursor, marks, and scroll start again without change to scope, slots, or playback. Home is selected with default Panel mode, widths, artwork presentation, and startup focus rules. Transient overlays, Inline Search, multi-selection, Visual mode, drafts, and pending UI navigation are cleared. Media, credentials, settings, queue contents, playback, progress, and caches are untouched. The F5 data refresh is a different action. It refetches content. It is the one deliberate exception to TUI launch state as exit-only. It clears the saved launch snapshot and the per-library saved positions and presentation preferences that a launch restores. A Client started before the next orderly exit then starts from defaults.
_Avoid_: factory reset, clear cache, data reset, hard reset

Attach:
The act by which a Client establishes a ctrl connection to an existing Player owner. It is a control relation. It is not login or Service setup. Its handshake can present a Control or legacy Service credential for admission. It establishes no Client identity. Several Clients can attach at one time without displacement of each other. Connection never evicts existing clients (multi-connection model, ADR 0014).
_Avoid_: reattach, connect, resume, take over

## Queue

Queue slot:
One separately addressable occurrence of a QueueItem in a canonical queue. Two slots can hold the same content and keep distinct slot identities. Slot identity is stable across moves. Content identity is not.
_Avoid_: queue item, content ID, playlist index

Content identity:
The Service-qualified identity of media content. It differs from the identity of each Queue slot that holds it. It is an Emby ID, a Feed guid, or an Audiobookshelf tuple (`libraryItemId` plus `episodeId`).
_Avoid_: item ID, queue ID, slot ID

Queue source:
The recorded origin of a queue. The values are Playlist (with optional id and name), Album, Series, Shuffle, Remote, Collection (with collection type), or Unknown. It survives restore. It serves UI display and save-on-consume decisions.
_Avoid_: queue origin, queue type, source type

Consume:
Removal of an item from the queue after it finishes playing, as in ncmpcpp. It is only a queue operation. It says nothing about the queue origin. It never edits anything on the server. Only the authoritative Player owner playback lifecycle drives it. That owner is the local Owner process or a directly controlled remote Player owner. A Session watch of another device generic Emby Session never consumes. That observation carries no mbv queue authority. Consume addresses canonical slot identity. It removes only the consumed occurrence.
_Avoid_: auto-remove, playlist consume, consume-and-save, remote consume

Save on consume:
The separate opt-in behavior that writes the shortened queue back to the Emby playlist it came from. It is valid only for a queue that is a saved playlist. Consume happens with or without it.
_Avoid_: autosave, consume persistence, playlist sync

Composed:
The retired pre-owner-process stage. A client UI once held an editable queue before submission to a Player owner. Local Clients now display adopt-only queue snapshots.
_Avoid_: draft, staging queue, pending queue, unplayed queue

Bound:
The stage in which a Player owner holds a queue. Contents obey the rules of that owner. An audio-only owner does not hold items it cannot play. An owner without loaded Audiobookshelf setup does not hold Audiobookshelf items. Bound does not mean playing. A stopped owner still holds its queue. Queues can be Bound to two owners at one time while only one of them plays.
_Avoid_: active queue, live queue, running queue, attached queue

Owner-held queue and source:
The Owner process holds the only authoritative local Bound queue and Queue source and stores them. Attached Clients are readers. They display owner-accepted snapshots. They never stage, store, or seed a queue of their own. Packaged mbvd keeps its separate behavior.
_Avoid_: client-owned queue, thin-client seeding, staged local queue

QueueLineage:
An owner-minted identity with an accepted queue state. A source-only update (Save As) applies only to the lineage the owner held when requested. Clients echo owner-minted lineage and never authorize it themselves.
_Avoid_: generation stamp, client lineage, fence token

Queue epoch:
A Client-local counter (`QueueEpoch`). It advances on each Client-side queue replace, clear, or attach. It fences async completions against queue changes this Client made. Playlist saves and idle loads are examples. It is never sent over ctrl. It is captured with the owner lineage as a `QueueOrigin` when a request is made. The fence value shape then follows the owner.
_Avoid_: remote queue lineage, client lineage

Unplayable item:
An item that a Player owner lacks the capability to play. Cause can be media kind, required Service availability, or playback support. It never enters the queue of that owner. A controlling Client strips it before submission. The owner discards any that reach it in all cases. A fully non-audio submission to an audio-only owner produces a structured rejection.
_Avoid_: rejected item, filtered item, invalid item, blocked item

EmbyItem:
The Emby-side item type of the queue. It is the serialized record of an Emby library item. It was renamed from MediaItem. The rename is invisible on the wire because serde field names are unchanged. Positions for EmbyItems report to the Emby API.
_Avoid_: MediaItem, media item, emby entry

QueueItem:
The media snapshot of the queue. It is an EmbyItem, a FeedEntry, or an AudiobookshelfItem (`Episode(AudiobookshelfQueueItem)` or `Book(AudiobookshelfBookQueueItem)`). Generic queue operations use shared presentation and identity behavior. Service admission, source preparation, lifecycle, progress, and cleanup stay as explicit boundaries. Stored data round-trips tagged QueueItem values. Legacy untagged Emby-only payloads stay readable.
_Avoid_: queue entry, playable, mixed item

Transport command:
A command to the Player owner for transport control. `Step(Direction)` asks for an owner-resolved relative queue move. `Player(PlayerCommand)` carries a playback command such as pause, seek, or stop. A step is not a `PlayerCommand`. It resolves against the owner canonical queue.
_Avoid_: playback command, navigation command, transport action

Relative step:
A request to move to the next or previous Queue slot. The Player owner resolves it from its latest requested or observed target with `relative_step_target`. Unlike a jump that names a slot, a relative step derives its target at handling time. Same-direction requests can combine while a step transition is unsettled.
_Avoid_: next command, previous command, relative navigation

Playback resume:
The rule that decides whether a prior watched entry resumes or starts over. When position passes 1 percent of runtime, resume starts from that position. When runtime is unknown and position is above zero, resume also starts from that position. Otherwise it starts from zero. It applies to Emby items, Feed entries, and Audiobookshelf episodes with the same threshold. Music is fire-and-forget and never resumes. Tracks, albums, and artists are examples. `EmbyItem::is_music` and `QueueItem::is_music` items skip this rule in full. `MediaSemanticState::from_emby` and `from_queue_item` always derive `Ordinary` for them without regard to stored played or position facts. A finished or partly played track never dims and never shows a resume percentage.
_Avoid_: continue threshold, resume percent, watched threshold

Playhead:
The one reconciled answer of the shell to where playback is. It names the queue scope that plays. It names the active slot in it. It tells whether that slot is confirmed by the Player owner or is an optimistic prediction not yet acknowledged. A prediction records its reason. A queue edit moved the still-playing item, or a different item was selected to play. Reconciliation happens in one tick-phase step against owner status. It never happens during paint. Playback position itself stays a live read.
_Avoid_: pending active index, cursor push, now-playing index, active_idx

## Browsing and tabs

MediaList:
The shared provider-neutral owner for one logical media-row flow. It owns rows, stable-target selection, cursor and scroll, row-local behavior, and retained geometry. It is embedded in the destination `LibraryPanel` slot. It is never mounted, focused, subscribed, or given a ComponentId. The Library panel owns the skeleton and slot. The destination keeps Service content and typed translation.
_Avoid_: generic list, generic media list, two-column list

Row flow:
The one ordered row sequence that a list presents, in paint order. Row position, viewport offset, and point resolution all address this same sequence. List order is the order in which its rows paint. The shared `crates/mbv-components/src/list/` seam owns it. Cursor movement, viewport resolution, retained paint geometry, and multi-selection each have one shared implementation over it.
_Avoid_: item list, row list, flat list

Structural row:
A row that holds a position in a Row flow but carries no stable target. It can never be the selection. A Group heading or a Spacer is an example. The cursor moves between selectable rows and never rests on a structural row.
_Avoid_: non-selectable row (bare), filler row, separator row

Multi-selection:
The set of rows that a user picked in one MediaList for a bulk action. The user builds it with Ctrl plus Click, Shift plus Click, or keyboard Visual mode (`V`). It is keyed by stable targets. The MediaList holds it beside its cursor. A non-empty multi-selection is Visual mode. Selection alone still means the cursor row.
_Avoid_: marked rows, selection (for the set), checked items

Mark:
Membership of one stable target in a Multi-selection. The seam `MarkSelection` carrier owns the ordered marks of a list. Add, remove, toggle, and clear of marks are the operations on that carrier. A mark is multi-selection membership. It is not a row highlight. Selection alone continues to mean the cursor row.
_Avoid_: marked rows (for the membership), mark (for the cursor row)

WideMediaList:
The provider-neutral one-column fixed-row TuiRealm Component over a `MediaList<Target>`. It is the canonical control for Library browser rails, Workspace lists, and Queue rows in every Panel mode. It never selects a Service or destination. The Library panel owns placement and supplies the current row flow rectangle. The destination keeps Service content and typed translation.
_Avoid_: generic list, two-column list, Inline Search

TreeBrowser:
The one complete shared nesting browser Interactive Component (`crates/mbv-components/src/list/tree_browser/`). It is the nesting counterpart to the flat MediaList. Destinations supply only plain `TreeNode<Target>` values plus the closed per-node `TreeMarkPolicy`. They translate emitted stable-target intents. They supply no tree behavior of their own. Like MediaList it is embedded. It is never mounted, focused, subscribed, or given a ComponentId on its own. Its `Component::view` is the only interactive view entry point. It delegates painting to one shared destination-neutral Render Component under `crates/mbv-render/src/components/tree_browser.rs`.

TV show tree:
The show-mode TV browser. It is one shared TreeBrowser. It projects shows as selectable expandable roots, seasons as expandable children, and loaded episodes as leaf children. It uses the established show group headings and between-group spacers as structural rows. Show modes (`All` and the alphabet ranges) use it in every Panel mode. `Latest`, `Upcoming`, Inline Search, and the Hero season pills and episode Workspace stay flat and outside it. Inline tree episodes deliberately duplicate Workspace episode rows. Row identity is a closed `TvTreeTarget` that scopes seasons and episodes to their show and season. A season loads its children only when its branch expands.
_Avoid_: show browser (bare), series list, nested TV list

Media-list row:
The one painted fixed-height row of a `MediaList` flow in every Panel mode. Its left-aligned metadata slot carries a progress badge in the FOAM role. Its right-aligned gutter has one closed metadata role. It is a date or year in the green `STATUS_AVAILABLE` role, or a green duration on Queue rows.
_Avoid_: wide media row, wide_media_row, left-aligned year slot

Group heading:
The non-selectable Heading row that labels a group of media-list Item rows. Artist, feed age bucket, letter or surname bucket, and season are examples. It paints bold in the FOAM metadata role. Unlike a media-list Item row, it keeps the surface fill and never paints the selected-row bar. It is a visual label. It is never a selection or action target. It takes its own place in the zebra order.
_Avoid_: artist header, group title, section label

Artist root:
The selectable tree root row for one artist in the Grouped Music browser (`grouped-music-tree-browser`). It is focusable and expandable. It carries stable Service artist identity or a deterministic fallback. It is the source for artist detail, that is Hero and Workspace, artwork, and artist tracks. It is never played as a target itself. Actions on it walk its settled album leaves. It differs from Group heading. Group heading is a non-selectable visual label elsewhere in the canonical lists. Do not conflate the two. Do not call the artist root a group, heading, or section.
_Avoid_: artist heading, artist group, artist header, group heading (for the root)

Inline Search:
A library-scoped search capability inside the selected searchable Emby destination. The destination owns the local search control, session, query, result selection, painting, and keyboard and mouse interpretation. The shell owns full-library fetches, recursive album indexing, stale-completion guards, navigation effects, and activation effects. EmbyLibraryContent, MusicContent, or TvContent is the sole owner and painter for the current presentation. TV moves one snapshot between Narrow and Wide. An ordinary tab change ends search. It differs from the cross-library Search sidebar.
_Avoid_: global search, Search sidebar, search overlay

Tab selection:
The one selected destination in the left panel. It is Home, EmbyLibrary(index), AudiobookshelfLibrary(index), or Feeds. Tab positions are count-aware. Home is 0, Emby N libraries take 1 through N, Audiobookshelf M libraries follow, and Feeds is always last. This prevents Emby and Audiobookshelf from sharing the same numeric position.
_Avoid_: library tab, browse tab, panel tab

Service browse dispatch / Browse target:
The complete boundary that maps each left-panel action to exactly one of Home, Emby, Audiobookshelf, or Feeds. Keys, mouse, refresh, rendering, help, and context menu are examples. Emby-only handlers receive an explicitly selected Emby library and never infer one. No fall-through treats all other tabs as Emby. Provider browse models stay separate and meet only at QueueItem construction and owner admission (ADR 0018).
_Avoid_: library routing (for this), provider dispatch, generic browse

Browse level:
One level in an Emby library navigation stack. It holds parent ID, title, items, total count, cursor, scroll, optional item-type filter, unplayed flag, sort criteria, optional letter-range pill, and optional music grouping. Levels stack when the user drills into folders, seasons, or shows.
_Avoid_: library level, folder level, nav level

Surname bucket:
One fixed alphabetical range of author surnames as a selectable pill in an Audiobookshelf book browser, for example A-C or V-Z. Empty ranges are omitted from the pill row.
_Avoid_: author group (the bucket is a range, not one author)

Home view:
The Home destination content, with the tab labelled `Continue`. It is Continue Watching across libraries. Home has no Latest pills or rows and no Selector row or pill bar. Latest lives at each eligible destination. The section keeps its own cursor and scroll.
_Avoid_: home screen, dashboard, landing, Home tab label (the tab reads `Continue`)

Destination Latest mode:
The `Latest` pill and content mode at an eligible destination. Every visible Emby library except Music libraries, each Audiobookshelf podcast library, and the Feeds tab are eligible. It shows the newest additions of that destination from its own source, without Home duplicates. It follows `destination-latest-modes`.
_Avoid_: Home Latest, Latest section (for the destination mode)

Launch cutoff / launch window:
The pair of client-launch times that bound new content. The prior client-launch time is the launch cutoff. It is read and atomically replaced at startup. The current launch time is the other bound. An item is new in a destination Latest mode under two conditions. Its provider time falls strictly after the prior launch. Its provider time falls no later than the current launch. The first launch sets the baseline and marks nothing.
_Avoid_: session start, boot time, last-seen timestamp

New-content marker:
The Iris `•` after a destination Latest pill label. It appears when its source holds an item new in the launch window. Selection of the pill acknowledges the source of that destination. It clears the marker for the rest of the client run. Prior selection also clears it.
_Avoid_: unread dot, badge, unread count

Library position:
The stored per-library drill depth, focused item, cursor index, sort, letter filter, and for feed-view libraries selected group and video cursor and scroll. It is restored across restarts. It is sticky across launches.
_Avoid_: browse position, library state, saved position

Idle feed:
An optional RSS feed URL. Its current item title displays in the playback panel when idle and that panel is rendered. In queue-only idle, the playback panel is hidden, so the title does not display. It moves through items on a configured interval.
_Avoid_: idle ticker, RSS ticker, background feed, screensaver feed

Idle collapse:
The Queue-visible behavior of the shell when fully idle. It removes the card from the queue visual slot so the Queue list takes those rows. In queue-only mode it also removes the in-queue playback panel. Paused playback stays visible. A connected but not playing remote session or cast keeps its playback panel.
_Avoid_: idle hide, collapsed queue, empty player

Playback target:
The place where explicit playback actions go. It is the local in-process Player or a directly controlled remote Player owner through ctrl. It is an observed Emby Session through supported remote transport commands only, or an attached cast receiver. It resolves per action from queue scope, active route, and attachment. Only a Player owner target carries queue authority. Send to an observed Emby Session dispatches transport actions without giving queue or occurrence identity to that Session.
_Avoid_: play target, output target, active player

Kind-qualified target:
The stable identity that a Sessions-sidebar activation names. It is Emby(id) or Cast(id). It resolves against the current target snapshot of the shell at dispatch time. Independent Emby and Cast discovery refreshes then cannot move an outstanding selection to a replacement at the old position. An absent key is a no-op. It differs from a display index or row number.
_Avoid_: display index, row number, session index

Three-line flat list:
The reusable embedded flat presentation over the shared ordered Row flow. It paints one selectable item as one three-line unit. A flow position denotes an item, not a terminal line. Visible capacity derives from `stride = 3 + separator gap`. Only fully visible items publish hit geometry. All three lines form one stripe and selected-row-bar unit. Separator lines keep the surrounding surface fill, carry no target, and never resolve clicks. It stays embedded in its mounted parent. The parent keeps gesture recognition, chrome, and translation of target-bearing intents. The F3 Sessions sidebar is its first consumer.
_Avoid_: session card, three-line card, card list, index-based card list

## Panels and surfaces

Panel:
A root-composed region with its own placement, surface fill, layout, and Interactive Component. Panels are Tab, Library, Library playback, Queue, Queue playback, Status bar, and the pane boundaries. Panel focus still means which of Library or Queue receives navigation keys. It is independent of Panel mode.
_Avoid_: active panel, focused panel, pane focus

Panel mode:
The app-wide layout state. It is one of Mini, Narrow, or Wide. The three values are:

1. Mini names a state where only one of Library or Queue is visible, set by Panel focus. An explicit `x` toggle at any terminal width reaches it. A terminal width below the mini-view threshold forces it. Both paths are the same state. Mini names that only one panel shows, not the reason it shows.
2. Narrow names a state where both panels are visible. A hero-bearing Library panel uses the standard fixed-row browser. It opens a Library Hero overlay on demand.
3. Wide names a state where both panels are visible. A hero-bearing Library panel uses Wide hero when the shared width and minimum-height conditions hold.
_Avoid_: layout mode, view mode, panel state, responsive mode, breakpoint mode

Pinned panel:
The Wayland layer-shell surface that pinwin shows beside tiled windows. It holds the mbv TUI drawn to a pty that the panel owns. It is a desktop window made only by a pinned launch. It is not one of the in-TUI Panel regions, that is Tab, Library, Queue, and the rest. A plain terminal launch draws the same TUI with no pinned panel. It can be hidden and shown with `mbv --toggle` while mbv keeps running.
_Avoid_: panel (bare — reserved for the in-TUI regions), layer-shell panel, dock, sidebar, tray

Pinned panel width:
The active saved width of the Pinned panel. Collapsed (`cols`) and expanded (`cols_expanded`) are the two values. The running panel switches between them with the `pinned_width_toggle` keybind (`Ctrl+e` by default). Every pinned launch starts at the collapsed width. The active width is never stored. While pinned, `x` also switches the width and sets the Panel mode with it. Expanded is Library-only. Collapsed is Queue-only. `Ctrl+e` changes only the width.
_Avoid_: panel mode, expand mode, panel size

Focus accent:
A stroke around the Pinned panel window while it holds keyboard focus. It draws in the `[panel]` `accent_color` at `accent_width` pixels unless `accent` is false. It is fixed at launch because pinwin has no runtime accent API.
_Avoid_: focus ring, strip, highlight

Panel covering:
A `[panel]` setting (`cover`) for the Pinned panel. When it is true, the panel draws over the tiled windows and the compositor reserves no space beside it. When it is false, the default, the compositor reserves a strip and tiled windows move aside. The choice is made when the layout is built, so `mbv --pin` applies it at launch.
_Avoid_: cover mode, overlay, always on top

Pinned launch:
A launch that runs the mbv TUI in the Pinned panel in the same process instead of the current terminal. Only the `--pin` flag selects it. It selects where the TUI draws, not who owns the Player. No configuration setting starts one. When the panel cannot start, mbv reports the reason. It then falls back to the current terminal or exits with a non-zero status.
_Avoid_: pin mode, desktop mode, Panel mode (reserved for the in-TUI layout state)

`--pin`:
The launch flag that requests a Pinned launch. It starts no separate program. It opens one panel per launch. It closes that panel when mbv exits.
_Avoid_: `--desktop`, pin setting, pinning option

Panel gutter:
One of the four `[panel]` margins. They are `gutter_top`, `gutter_bottom`, `gutter_left`, and `gutter_right`. They use pixels and can be negative. They offset the Pinned panel from the screen edges. The F2 Panel page or `config.toml` edits them. While pinned, a layout that the panel rejects leaves both the panel and the stored value unchanged. It differs from the in-TUI Library layout gutters and the Selected-row bar gutter treatment.
_Avoid_: gutter (bare), margin, padding, inset, spacing

Zebra stripe:
The alternating-row secondary background on Wide media lists. Grouping Heading rows and Spacer rows keep the surface fill outside the sequence. Member rows of each group alternate from the secondary fill at the first member of the group. A stripe then follows row position in its group and not the screen row it lands on. A Spacer row between groups sits outside the sequence and keeps the surface fill. A list with no Heading above a row counts from its first row, which is unstriped. A selected row paints the selected-row bar instead of its stripe.
_Avoid_: alternating row, striped background, row banding

Selected-row bar:
The fixed selected-row treatment of every canonical media list. While the list holds focus, the whole row paints the `#2d353b` bar fill across its full width. It overrides its zebra stripe, its two-column gutters, and its owning surface. Every span keeps the ordinary unselected foreground role. No title is bold and none takes an accent color. A multi-selected row paints the bar too, including while the list is unfocused. An unfocused list paints no bar for its cursor row.
_Avoid_: gutter accent, gutter-selected style, selection marker

Title reveal:
The closed title-reveal policy of one media list. `Always` paints the full title on every row and is the default. `OnSelection` paints only primary context text on a row outside the selection of the list. It paints the full context-and-title text on the selected row, marqueed while the list is focused. The destination that composes the list declares it one time. The shared row painter applies it. No destination paints or branches its own rows. The podcast episode browser is the only `OnSelection` list today. The list reads as its parent podcasts at rest. It reveals the episode name where the user looks.
_Avoid_: hidden title, hover title, selected-only title, marquee mode

Tab panel:
The root-composed Panel that paints tab selection and overflow controls for the Library column. It owns their hit geometry.
_Avoid_: tab bar

Status bar panel:
The root-composed Panel that paints the status row. It paints volume, mute, and remote controls when the Library column is visible.
_Avoid_: status bar

Library panel:
The root-composed Panel that owns the shared Wide and Narrow library skeleton. Destinations supply typed Selector row, list, Hero header, and Workspace content. They do not place or paint those slots. It also owns the Library Hero overlay lifecycle and its Library-confined placement and hit geometry.
_Avoid_: destination panel, library screen

Library Hero overlay:
A Library-panel-local detail surface for a selected hero-bearing browser row in non-Wide geometry. It is centered in and confined to 90 percent of the visible Library pane. In non-Wide geometry that pane is the inset list box of the browser. The Selector row and the spacer band below it stay outside both the overlay frame and its dimmed backdrop. It reuses the shared Hero header, overview (including cast and crew), artwork, and optional Workspace content. Its provider-link row stays plain text. It leaves a visible Queue independently operable. Library focus can end it with Esc or a click on the dimmed Library remainder. Change of destination ends it. It differs from application popups and from the four anchored Sidebars.
_Avoid_: application popup, Sidebar, full-window overlay, separate detail block

Library playback panel:
The root-composed Panel that paints the playback transport in the Library column when the Queue column is hidden.
_Avoid_: player strip

Queue playback panel:
The root-composed Panel in the Queue column. It owns the header, visual slot, and transport when the Queue column is visible. The header shows the now-playing title while a target plays. Otherwise it shows the playback status or target. It remains while idle, when the visual slot and transport collapse.
_Avoid_: now-playing panel

Now-playing title site:
The one shell-projected location that carries the active item title. It is `Header` while the header row carries it. It is `Artwork` only after the composed title on the queue artwork painted in fact. The header says `Now Playing` while `Artwork` is the site. Uncertainty keeps the site at `Header`.
_Avoid_: title location, title host, overlay site

Queue artwork title overlay:
The static title and optional context composed onto the queue card artwork for the active item. It differs from the plain cached artwork. It holds no time-varying playback information. It carries the title only after its composed artwork painted, as shown by the Now-playing title site.
_Avoid_: overlay (bare), artwork label, logo

Selector row:
The Library panel slot for one browse pill bar. A letter range, group, bucket, section, or the Feeds watched-filter pills followed by feed groups are examples.
_Avoid_: selector bar

Hero header:
The Wide Library panel slot for selected-item facts and artwork. Its closed content-derived shapes are Landscape (artwork above text), Portrait and Square (text beside artwork). The panel derives the shape from the artwork policy. A destination never selects an arm.
_Avoid_: hero variant

Workspace:
The optional Wide Hero pane slot for one optional header row, one optional Selector row, and one constituent-item list. Tracks, episodes, or chapters are examples. The Library panel owns its box, surface, and placement.
_Avoid_: detail workspace

Wide hero:
The sole Wide arrangement for hero-bearing browse surfaces. The one-column Library browser and its pills take the left pane. The selected-item hero or provider-owned detail workspace takes the right pane. It applies only when the shared width breakpoint and minimum-height guard hold. Otherwise the standard fixed-row browser stays active and detail opens through the Library Hero overlay.
_Avoid_: separate detail block, split, side-by-side, hero-on-side, hero-on-left, hero-on-right

Hero pane:
The fixed `#1e2326` `Palette::Ink` dark sheet of the Wide hero right pane. It is the container surface itself, apart from what paints inside it. The sheet is fixed in both focus states. It does not lighten with the focus bit of the panel. It is the same dark chrome that the Library Hero overlay paints when it shows the same hero over a non-Wide browser.
_Avoid_: recessed box, hero panel, detail panel

Main content box:
The `#2d353b` `SURFACE_BACKDROP` inset in a Hero pane or Library Hero overlay. It holds kind-dependent body content at one shared padding value. It holds overview text and, for a Movie in the Wide Hero pane, the Cast and crew table in the same box. It differs from the fixed dark Hero pane sheet it sits in. The pane is the outer container fill. The box is the inner content inset.
_Avoid_: overview box, recessed box

Provider-link row:
The Movie metadata row that joins the names of its declared provider links. It renders after the release-date, runtime, and genre rows. Each name is plain text unless the terminal declares hyperlink support and its URL is an `http` or `https` URL with no control bytes. Eligible names use an OSC 8 hyperlink and stay in the same row. The row stays plain text in the Library Hero overlay.
_Avoid_: external-links row, link list, clickable links

Cast and crew table:
The Wide Hero pane table of Movie people in the Main content box. It has one name column and one shared role column, right-aligned at the right edge of the box. It has no header or separator. It lists every person that the server reports. Every Director comes first in provider order. Then every remaining person follows in provider order without regard to type. Nothing is capped. An empty role falls back to the provider type of the person. A blank row and then a line of block characters in sage green separate the overview text from the first row of the table. The table starts at the first content row of the box when no overview exists. The box scrolls when content exceeds its height.
_Avoid_: credits list, cast list, detail table

Render Component:
A `crates/mbv-render/src/components/` unit. It takes a typed content model plus a `Rect` and, for Ratatui, a `&mut Buffer` or `Frame`. It paints and computes its own geometry in that `Rect`. Components consume semantic theme roles or component style policies. They never take arbitrary `Color` or `Style` passed in from a screen.
_Avoid_: component (bare), interactive component, screen, arrangement

Interactive Component:
A TuiRealm `AppComponent` under `crates/mbv-components/src/`. It owns the local presentation state, input interpretation, updates, rendering, and geometry of one separately routed surface. It returns typed requests for work outside its authority.
_Avoid_: component (bare), controller, render component, widget

Keyboard Router:
The one keyboard routing authority, in the `UiRoot` Interactive Component (ADR 0023). It observes every chord without regard to focus. It resolves each chord against ordered policy to one of the ADR 0002 outcomes. They are `Command`, `Swallow`, `FallThrough`, or a `Deferred` context-sensitive candidate. `FallThrough` lets the typed request of the focused Interactive Component stand. Exactly one exists.
_Avoid_: input handler, dispatcher, context stack, key policy (the policy is data the router evaluates, not a second router)

Arbitration fold:
The one central pure combination (`arbitrate_key`, ADR 0023 amendment). It combines the Keyboard Router outcome and the leaf disposition of the focused component for the same tick. It produces a final disposition and the requests to dispatch. It owns no policy and resolves no chord. It is not a second Keyboard Router. It is the only place where a local claim suppresses a context-sensitive candidate. It is the only place where deferred-candidate clocks advance or reset.
_Avoid_: second router, dispatcher, message filter

Global chord:
A key combination with meaning that does not depend on the focused surface. The Keyboard Router therefore claims it and no Interactive Component interprets it. A selection-dependent chord (`.` for the context menu) is not global, even though every destination binds it.
_Avoid_: global key, hotkey, shortcut (bare — a shortcut can be leaf-local)

`component` state:
The middle interactive-surface ledger state. An Interactive Component paints the surface in it. The shell still mirrors `App` state or legacy input still forwards, or both. `App` teardown stays pending as group 5.
_Avoid_: migrated (until the mirror and legacy handler are removed)

Arrangement:
A `crates/mbv-render/src/arrangements/` unit. It takes a typed content model plus a `Rect`. It places one or more components. It owns breakpoints and rect splitting. Arrangements sit between screens and components in the render order (`screens -> arrangements -> components`).
_Avoid_: layout (bare), component, screen

Variant:
A centrally owned closed visual form. Its arm derives from content or state open to every caller. A caller-selected arm with only one user is a defect, not an endorsed pattern. Conform that caller or broaden the shared content type. The Hero header derives Landscape, Portrait, or Square from artwork policy. The Library panel derives its Wide Hero arrangement from shared geometry.
_Avoid_: mode (reserved for Panel mode), option

Policy:
A centrally owned rule. It derives a closed style or behavior choice from shared content or state. It is not a caller-selected escape hatch. A policy value with one caller or one user is a defect to remove, not approved vocabulary. Policies own their geometry and painting results.
_Avoid_: config, options, flags (bare)

Bespoke surface:
A named Interactive or Render Component needed only after the shared Panel, slot, arrangement, and content vocabulary truly cannot express a surface. It records the reason and has buffer coverage. It is not a caller-selected variant arm, exemption, or second composition path.
_Avoid_: one-off, special case, exception

Sidebar:
An anchored full-height destination rendered at a fixed width from one edge of the window (code: `render_panel_shell_at`, whose rect is literally named `sidebar`). It differs from a popup, a centered dimmed-backdrop overlay (code: `render_modal_frame`). The Feeds-management, multiselect, library-routes, save-playlist, and confirm dialogs are examples. The four sidebars are Search sidebar, Settings sidebar, Sessions sidebar, and Help sidebar. Feeds management is a popup nested in the Settings sidebar, not a sidebar itself. It exists only while Settings is open.
_Avoid_: panel (reserved for Library/Queue), screen, overlay (bare), full-window destination

Search sidebar:
The global cross-library search surface. It filters to navigable media types (Series, Episode, Season, Movie, Audio, MusicAlbum, MusicArtist). It has an optional type filter pill and per-query result deduplication.
_Avoid_: library search, global search (bare), omnibox

Watched filter:
The All, Played, and Unplayed selector in the Feeds tab (`w` key). It filters feed entries by their played flag. Audiobookshelf podcast browsing has a matching All, Played, and Unplayed episode filter.
_Avoid_: played filter, hide watched, unwatched filter

## Audiobookshelf

Podcast:
An Audiobookshelf podcast show or one of its downloaded episodes. The word never means an Emby library. Emby has no podcast feature. A Channel or `podcasts` collection library from an Emby addon receives only generic Emby handling with no podcast behavior.
_Avoid_: Emby podcast, Emby channel, podcast library (bare), podcast channel

Audiobookshelf library:
One Audiobookshelf library shown as a peer tab. It resolves one time into a podcast kind or a book kind at tab selection. Book and Audiobookshelf podcast libraries interleave as peer tabs in the server `/api/libraries` order, exactly as Emby libraries do. No type partition or reorder applies. Identity is Service kind plus library ID.
_Avoid_: ABS library, audiobookshelf collection, podcast library (as kind)

Downloaded podcast episode:
An Audiobookshelf podcast episode stored as downloaded media. It is identified by its `libraryItemId` and `episodeId`. It differs from an RSS FeedEntry and from a remote podcast episode that Audiobookshelf did not download.
_Avoid_: feed episode, podcast item, track

Audiobookshelf show:
One podcast show (series) in an Audiobookshelf podcast library. It is identified by Service kind plus `libraryItemId`. It holds title, author, cover path, and a paged list of downloaded episodes.
_Avoid_: podcast, show item, ABS show

Audiobookshelf book:
One audiobook in an Audiobookshelf book library. It is identified by Service kind plus `libraryItemId` only. Books have no episode identity. It carries title and cover path. It carries the raw author credit (`author_display`) and its first-listed-author surname sort key (`author_sort_key`, through `human_name`, with fallback to the raw credit). It carries book-relative `chapters[]` and `audioFiles` detail. Queueing projects the whole book as one item and one continuous mpv timeline across its audio files. Chapter rows seek in absolute terms against that merged timeline.
_Avoid_: audiobook item, book episode, track

Audiobook chapter:
One book-relative seekable range `{start, end}` in seconds across the whole book timeline, as Audiobookshelf `chapters[]` reports it (it can span audio files). mbv renders each as a first-class row and issues one absolute seek to `start` on the merged timeline on activation.
_Avoid_: track, segment, file part

AudiobookshelfItem:
The nested Audiobookshelf shape of a QueueItem. It is `Episode(AudiobookshelfQueueItem)` or `Book(AudiobookshelfBookQueueItem)`. Ownership follows the QueueItem variant. Shape questions follow the inner variant.
_Avoid_: ABS item, episode-or-book

AudiobookshelfBookQueueItem:
The QueueItem snapshot of a book. It holds content identity, presentation, progress, completion, and Service-scoped artwork identity keyed by `libraryItemId` only. It is a sibling of `AudiobookshelfQueueItem`, never an `episode_id` option. It carries no credential, server URL, playback-session ID, resolved source, or request headers. It matches the redaction boundary of the episode item.
_Avoid_: ABS book item, audiobook episode, book queue entry

AudiobookshelfQueueItem:
The QueueItem snapshot of a downloaded podcast episode. It carries content identity, presentation, progress, completion, and Service-scoped artwork identity. It carries no credential, server URL, playback-session ID, resolved source, or request headers. It is eligible only for Player owners (Owner process, packaged mbvd) with Audiobookshelf setup and credential.
_Avoid_: Audiobookshelf episode, ABS item, feed entry

Audiobookshelf playback session:
Short-lived Audiobookshelf lifecycle state. It opens to resolve and play one episode or one book and to synchronize its progress. It is neither an Emby Session nor an mbv Playback run. It is made just in time for the active slot. Close is bounded and finalized before the next session opens. Monotonic wall-clock listening time accrues only while not paused. Episode sessions use `libraryItemId` plus `episodeId` as key. Book sessions use `libraryItemId` only.
_Avoid_: session, playback run, Emby session

## Feeds

Feeds Service / Feeds tab:
The built-in Feeds Service of mbv and its Feeds tab. They manage RSS and Atom FeedSubscriptions and their FeedEntries. This differs from an Emby homevideos feed view.
_Avoid_: feed view, Emby feed, homevideos feed

Emby homevideos feed view:
The grouped feed-like browse surface of an Emby homevideos library, for example a configured YouTube tab. It differs from the Feeds Service and tab. `restore-feed-group-inline-expansion` concerns this Emby homevideos feed view, not the Feeds Service tab.
_Avoid_: Feeds tab, Feeds Service, RSS subscription

FeedSubscription:
A user subscription to one RSS or Atom feed. It holds display name, URL, and FeedKind. It is stored per user in local `config.toml` as `[[feeds]]`. Fetched entries and fetch metadata are not stored. Edit never changes the URL. A changed URL is a new subscription. YouTube channel URLs are normalized to RSS on subscribe.
_Avoid_: feed config, channel, subscription config

FeedKind:
The Audio or Video classification of a FeedSubscription. It is inferred from enclosure MIME types when present (mixed or absent defaults to Video). The user can edit it. It governs queue admission for entries that carry no MIME type of their own.
_Avoid_: feed type, media type, category

FeedEntry:
One parsed item from a subscribed feed. Identity is guid, else enclosure-URL hash, else title plus pub-date hash. It carries enclosure URL, link, mime type, pub_date, duration in Emby ticks, description, and the normalized subscription URL as `feed_id`. That URL is the stable identity used by local feed-entry state. Each entry carries local `position_ticks` and `played` playback state. It is restored from the local state directory and updated on playback lifecycle events (stop, pause, seek, EOF). Entries never report progress to Emby. Queued FeedEntries are owned snapshots. Deletion of the subscription leaves them playable for the life of the Bound queue.
_Avoid_: episode, post, feed item, rss item

## Remote sessions

A client can also reach playback on another device. It is found through Emby and not through the Owner process. The Owner process is never a remote session. This relation differs from Attach above, even though both involve one process that reaches a Player owner over a socket.

Session:
An Emby-tracked record that some device plays something. It exists apart from mbv, including for non-mbv devices. It is what the Sessions sidebar lists. Emby Sessions can advertise a private-LAN control port in `supported_commands` (`mbv-direct-tcp-port`).
_Avoid_: connection, stream, remote instance

Session watch:
A client that observes the generic Emby Session of another device. With respect to the mbv queue it is read-only. It can display directly observed position and title. It can dispatch supported remote transport commands (play, pause, seek, stop, next, previous, volume, mute, stream selection). It never changes mbv queue membership, order, slot snapshots, cursor, or playlist state. It never infers queue position, occurrence identity, completion, or consume from that Session. It is the fallback when Direct remote control to that device is not present.
_Avoid_: attach, session attach, monitor, remote session (bare), tracked session, tracking

Direct remote control:
A client with its own control-socket connection to the Player owner of another device. It gives the same queue management as local playback, that is reorder, remove, play next, and all of it. The device stays the connected row in the Sessions sidebar. Take of the control socket does not erase that. The Owner process is never Direct remote control. The aqua queue-scope pill shows it.
_Avoid_: green pill, remote takeover, queue management (alone)

Queue scope:
Local or Remote. It tells which queue the queue panel shows. It shows the queue on the local side of the controlling terminal or the queue of the directly controlled remote Player owner. It exists during Sessions-sidebar Direct remote control and explicit remote owner attachment, not Session watch or a Library route. Remote scope is selectable only when a direct remote queue exists. Otherwise Local is forced.
_Avoid_: split view, pill state

Library route:
A stored per-library assignment that sends playback of that library to a selected device. It is set from the library-routing picker and not from the Sessions sidebar. It is stored as `lowercased name -> tcp://host:port` endpoint (device name is transient in the picker and resolves at once to an endpoint before storage). It is independent of Session watch and Direct remote control. Connection through the Sessions sidebar tears down an active route first. F2 Settings manages routes. Hand edit of `config.toml` is supported. Malformed non-`tcp://` values are logged and skipped.
_Avoid_: routing, daemon route

## Cast

Cast receiver:
A Google Cast device found on the LAN through mDNS. It appears as a row in the F3 target panel with Emby sessions. It is identified durably by its advertised identifier across address changes and not by a stored host or port. It is called a cast target once mbv attaches to it. That is the receiver in its role as the place where playback actions go.
_Avoid_: chromecast device, TV, cast device

Cast attachment:
The relation of mbv to a cast receiver it controls. Selection of one attaches mbv without start, stop, or reconfiguration of the local Player. While attached, play of a selection dispatches it to the receiver instead of local playback. It is held beside `connected_session_id`, apart from Emby session state. Both can be attached at one time. It differs from Attach above. It is not a ctrl connection and makes no Client relation, even though both name one process that reaches a playback target. On launch with `auto_reconnect` enabled, mbv attaches to the receiver it was attached to at exit. It restores control and displayed state from its reported status without dispatch of anything.
_Avoid_: attach (bare, without "cast"), cast session, cast connection

Dispatch:
The act that sends the items of a played selection to the own media queue of a cast receiver in one act. mbv does not project, track, or reconcile that queue after. The receiver advances on its own between dispatched items. It differs from Bound (a Player owner that holds the own queue of mbv). A receiver owns what it plays apart from the queue of mbv once dispatch completes.
_Avoid_: enqueue, cast queue, load (bare)

Uncastable:
An item that the mbv queue holds and that cannot go to a cast receiver. It has no retrievable media URL. It has a credential that lives only in a request header. For Audiobookshelf, it can be a multi-file book whose position has no form on a receiver without harm to its stored resume point. It is reported to the user by name and reason and not replaced or dropped in silence. It differs from Unplayable item, which never enters the queue of a Player owner at all.
_Avoid_: unplayable (cast), unsupported item, skip reason

TV content mode:
One selectable mode of the top-level pill row of a TV library. It is `Latest`, `Upcoming`, an alphabet range (`A-I`, `J-R`, or `S-Z`), or `All` (small libraries only). The row appears only at the top browse level while not searching. `Latest` and `Upcoming` rows are flat episode lists that play directly. The selected mode is part of the sticky library position. It follows `tv-library-content-modes`.
_Avoid_: TV pill (for the mode), TV range (for the whole row), TV tab (for the mode)

TV letter range:
The TV-only set of alphabet range pills. They are `A-I`, `J-R`, and `S-Z`. `A-I` sorts names before `J` and includes non-letter names under a `#` in-list header. `J-R` spans from `J` up to but not including `S`. `S-Z` spans from `S` onward. They are offered above the pill threshold instead of the movie library ranges. They follow `tv-library-content-modes`.
_Avoid_: TV alphabet (bare), letter pill (for the set), movie range (for TV)
