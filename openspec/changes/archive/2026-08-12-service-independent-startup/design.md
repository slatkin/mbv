## Context

See `proposal.md` for motivation and scope. Startup currently constructs one mandatory `EmbyClient`, authenticates it before every TUI path, and stores general configuration inside that client. `App::run` then performs synchronous Emby capability and Home loading before normal event processing. The Local daemon and ctrl handshake likewise require an Emby token, while feed subscriptions and local feed state are conceptually independent.

The existing persisted shape is one `[server]` URL in `config.toml` and one mode-`0600` `token.json`; configuration parsing also returns early when `[server]` is absent. Ctrl changes must follow the capability rule above `CTRL_PROTOCOL_VERSION`. The separately packaged `mbvd` remains on legacy Emby-authenticated control during this change.

## Goals / Non-Goals

**Goals:**
- Make application and local playback-owner construction possible with zero Remote Services.
- Separate general configuration, Service setup, runtime availability, Service credentials, and Control credentials.
- Preserve existing Emby browsing/playback behavior after Emby reaches Ready.
- Make legacy Emby migration lossless and non-interactive.
- Establish extension seams needed by the next Audiobookshelf authentication change without implementing Audiobookshelf.

**Non-Goals:**
- A generic trait covering every catalog, search, Session, or playback behavior.
- Audiobookshelf credentials, `/api/me`, libraries, queue items, or playback.
- Multiple servers of one Service kind.
- Provider-neutral shared-state identity or changes to its Emby-scoped server contract.
- Migrating packaged `mbvd` to Service-independent startup or Control credentials.
- Persisting remote catalogs for offline browsing.

## Decisions

### Decision 1: App owns configuration and optional concrete Service runtimes

General configuration moves out of `EmbyClient` ownership. Startup builds an application context containing configuration and runtime state for singleton Services; Emby remains a concrete runtime rather than being forced behind a broad media-provider trait.

This is the smallest seam that permits zero Emby clients and a later concrete Audiobookshelf client while preserving provider-specific browsing models. A universal provider interface was rejected because current navigation, Sessions, routing, search, and wire types remain deliberately Emby-specific.

### Decision 2: The event loop starts before Remote Service workers

Process-role selection, configuration loading, local state restoration, and Player-owner construction complete without Remote Service network calls. Once the TUI can render, each configured Remote Service begins a bounded background initialization and reports state changes through application channels.

This avoids replacing one global authentication gate with a concurrent all-services gate. Lazy initialization on first use was rejected because configured content should become available without requiring the user to visit Settings or a provider-specific tab.

### Decision 3: Service state is explicit and runtime-only

Remote Services expose Not configured, Connecting, Ready, Needs authentication, and Unavailable. Persisted setup and credentials determine initial state but the state itself is not persisted. Authentication rejection clears only the rejected secret; connectivity failure preserves it.

Keeping error strings or optional clients as implicit state was rejected because the UI and independent initialization need stable, actionable semantics.

### Decision 4: Services is a Settings destination with first-launch routing

The existing Settings surface gains Services management. When no Remote Service and no feed subscription is configured, initial navigation opens that destination inside the ordinary TUI. Feed subscriptions remain the contents of the always-present Feeds Service rather than acquiring artificial authentication state.

A top-level Services content destination and a pre-TUI setup wizard were rejected as unnecessary permanent navigation and a new startup gate, respectively.

### Decision 5: Emby setup exchanges transient credentials for a token

The current login form is recast as Emby Service setup/repair. Server URL, username, and password are submitted to Emby; only the returned token and required Emby identity metadata survive successful validation. Failed setup is transactional and cannot overwrite a working Service.

Direct token entry is deferred because preserving the current token-generation path is sufficient for the migration and keeps the first setup surface narrow.

### Decision 6: Secrets are isolated per Service

Non-secret Service setup remains in `config.toml`; each Remote Service receives its own mode-`0600` secret file. The legacy `token.json` is migrated with write-new-then-remove-old ordering. No Service credential enters redb/shared-state storage.

One combined credentials file was rejected because per-Service files isolate rotation, deletion, migration, permissions checks, and future Service additions. Storing tokens in `config.toml` was rejected because it weakens the existing explicit secret boundary.

### Decision 7: Server replacement invalidates Service-owned state

Because each Service kind is singleton, provider-native IDs are qualified by Service kind rather than a generated account or connection registry. Changing a Service to a different server is destructive replacement: after confirmation, clear its queued items, library positions, routes, caches, and secret before committing the new validated setup. Removal performs the same cleanup and returns to Not configured.

Canonical URL identity and generated Service-incarnation IDs were rejected because old items cannot be resolved or reported without the old configured server anyway. Clearing prevents accidental ID collision with the replacement server.

### Decision 8: Local daemon control uses a per-owner Control credential

Generate and persist a stable mode-`0600` credential for this user's Local daemon. The daemon advertises a new additive ctrl capability, and capable clients present the Control credential in a defaulted hello field. The existing Emby-authenticated field retains its old meaning for peers without the capability.

The daemon hello arrives first, so clients can select the appropriate handshake without changing framing. New clients can continue reaching deferred `mbvd` through the legacy path when Emby is Ready; feed-only clients receive a compatibility error. Reusing Service credentials and unauthenticated sockets were rejected because control authority and upstream media access are separate trust domains.

The implementation may choose the narrowest safe reload/restart mechanism when Local daemon Service files change; credentials themselves never cross ctrl.

### Decision 9: Player owners resolve Service-backed playback

A Player owner must hold the Service setup and credential needed for a Service-backed queue item. Clients send qualified items, never Service credentials or client-resolved expiring URLs. Owners without the required Service treat those items as unplayable and do not admit them to a Bound queue.

This preserves stay-alive progress reporting and stream renewal after every client exits.

### Decision 10: Shared state only degrades gracefully

This change does not invent an mbv account to replace the shared-state server's Emby identity. With no usable Emby identity, shared state follows its existing local fallback contract. Local startup, state, browsing, and playback remain available.

## Risks / Trade-offs

- **[Risk] Existing code assumes `EmbyClient` is always present across many App paths** -> Introduce the startup context first, preserve concrete Emby modules, and gate only Emby-owned actions on Ready rather than spreading placeholder clients.
- **[Risk] Background initialization races rendering or stale attempts overwrite newer setup** -> Tag initialization results with the Service setup generation and ignore results for a replaced or removed setup.
- **[Risk] Migration or replacement loses credentials/state** -> Use atomic writes, remove legacy secrets only after durable replacement, and order destructive cleanup only after new setup validation and explicit confirmation.
- **[Risk] Mixed ctrl peers expose confusing authentication failures** -> Advertise Control auth explicitly, preserve legacy field meaning, and return targeted compatibility diagnostics when no valid legacy path exists.
- **[Trade-off] Deferred `mbvd` remains Emby-gated** -> Contain this to peers lacking the new capability and migrate `mbvd` in a later proposal before Audiobookshelf daemon playback.
- **[Trade-off] Shared roaming remains Emby-scoped** -> Preserve local fallback and defer identity design instead of creating a mandatory mbv account.
- **[Trade-off] Clearing state on server replacement is destructive** -> Require confirmation and enumerate the affected Service-owned state before committing replacement or removal.

## Migration Plan

1. Introduce Service-independent configuration parsing and per-Service paths without changing existing startup behavior.
2. On load, atomically import legacy Emby setup into the new Emby Service files; retain legacy data on any write failure.
3. Introduce runtime Service state and move App/general configuration ownership out of `EmbyClient`.
4. Start the TUI and Player owner before initializing Emby, then move Emby Home/capability loading behind Ready transitions.
5. Add Services settings and recast the login form as transactional Emby setup/repair.
6. Add Local daemon Control credentials and capability-gated ctrl authentication while preserving legacy attachment for deferred peers.
7. Remove the global Emby startup gate only after bare, Local daemon, migrated Emby, feed-only, and unavailable-Emby paths work through the new model.

Rollback keeps the legacy credential until migration commits. After that point, rollback requires restoring the prior `token.json` from the new Emby secret fields; implementation should keep formats directly convertible during this change.
