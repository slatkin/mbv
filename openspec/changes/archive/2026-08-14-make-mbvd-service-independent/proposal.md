## Why

Packaged `mbvd` currently treats Emby as infrastructure: it cannot start without daemon-owned Emby credentials, constructs and runs Emby facilities unconditionally, and asks ctrl clients to send an Emby token over plaintext LAN transport. Issue #523 must isolate Emby as one optional Remote Service before Audiobookshelf can become another owner-installed Service.

## What Changes

- Make packaged `mbvd` start and retain its core Player-owner, queue, Feed playback, and ctrl behavior with zero configured Remote Services.
- Move Emby API access, WebSocket commands, lookup, capability registration, and playback reporting behind optional owner-local Emby runtime state.
- Add the owner-local `mbvd --connect emby` administration path: prompt for server URL, username, and password; validate and obtain an Emby token before commit; persist no password; and preserve working setup on failure.
- Reconcile a successfully committed setup into a running packaged daemon by signaling a credential-free reread of owner storage, or preserve the durable commit and report that restart is required.
- **BREAKING**: Remove packaged `mbvd` ctrl application authentication and the legacy client-presented Emby-token handshake. Unix filesystem permissions and configured trusted-LAN reachability become the intentional packaged-ctrl access boundaries.
- Preserve the same-user Local daemon Control-credential handshake unchanged.
- Preserve optional shared-data hosting and its existing Emby-scoped authentication unchanged; provider-neutral shared identity and daemon-settings migration remain out of scope.

## Capabilities

### New Capabilities

- `packaged-daemon-service-runtime`: Service-independent packaged-daemon startup and optional Emby-owned runtime behavior.
- `mbvd-service-administration`: Owner-local `mbvd --connect <service>` validation, transactional persistence, replacement, and running-owner reconciliation, initially implemented for Emby.

### Modified Capabilities

- `ctrl-protocol`: Remove Service-credential authentication from packaged `mbvd` ctrl while retaining Local-daemon Control authentication and transport-scoped lifecycle authority.

## Impact

- Affects the `mbvd` CLI and service unit guidance, daemon runtime construction, ctrl handshake, Emby WebSocket and remote-command setup, Player source/reporting dependencies, capability registration, owner state paths, and setup reconciliation.
- Replaces the requirement to run interactive `mbv` under the packaged-daemon identity with `mbvd --connect emby`.
- Requires explicit packaged-versus-Local daemon role behavior where the shared daemon implementation currently infers policy from `MBV_SYSTEM` and an always-present `EmbyClient`. Sharing the refactored core preserves existing Local-daemon behavior; this change does not make a new Local-daemon product guarantee.
- Targets ctrl protocol v9 to prevent an older client from sending an Emby bearer token to a newer packaged daemon. This is required by the documented ctrl wire rule: removing a field or changing handshake framing requires a version bump; the additive-capability rule remains in force for additive changes. The source currently declares v7 while the archived OpenSpec baseline declares v8; implementation SHALL reconcile that pre-existing drift and ship v9, rather than claim the source was already v8.
- Changes no shared-data protocol, Local daemon authentication, Audiobookshelf behavior, or general `mbv` Services-settings behavior.
