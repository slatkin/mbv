## Why

mbv currently treats successful Emby authentication as permission to enter the application, which prevents feed-only use, makes an Emby outage terminate startup, and blocks independently configured Services such as Audiobookshelf. This change establishes Service-independent startup and migrates Emby onto that foundation before any Audiobookshelf work begins.

Tracking issue: [#503](https://github.com/slatkin/mbv/issues/503)

## What Changes

- Enter the TUI before any Remote Service authenticates or becomes available; zero configured Remote Services is valid.
- Add a Settings Services view and open it initially when neither a Remote Service nor a feed subscription is configured.
- Represent each singleton Remote Service with explicit Not configured, Connecting, Ready, Needs authentication, and Unavailable states.
- Move Emby login into Service setup: collect username/password transiently, generate an Emby token, and persist only the validated Service setup and credential.
- Automatically migrate the existing Emby server, token, and user ID into per-Service configuration and secret storage without prompting.
- Initialize Emby independently after the TUI starts; rejected credentials retain the server setup but clear the secret, while connectivity failures preserve both.
- Support Emby Service repair, replacement, and removal; replacement or removal clears Emby-owned local state.
- Separate Local daemon control authentication from Emby authentication by introducing a stable, mbv-owned Control credential scoped to that Player owner.
- Allow bare mode and the Local daemon to operate without configured Emby credentials, including feed-only operation.
- Supersede the previous behavior that exits mbv when Emby is unavailable during startup.
- Defer Audiobookshelf setup and its `/api/me` connection test, all Audiobookshelf catalog/playback work, provider-neutral shared-state identity, and migration of the separately packaged `mbvd`.

## Capabilities

### New Capabilities

- `service-independent-startup`: TUI entry, initial routing, asynchronous Remote Service initialization, and non-blocking behavior when Emby is absent or unavailable.
- `service-management`: Singleton Service lifecycle, Services settings behavior, Emby token generation, credential persistence and migration, and destructive replacement/removal semantics.

### Modified Capabilities

- `ctrl-protocol`: Local daemon clients authenticate with an mbv-owned Control credential rather than an Emby Service credential, with capability-gated compatibility for deferred `mbvd` migration.
- `daemon-lifecycle`: Bare mode and the Local daemon start and remain usable without configured or available Emby credentials.

## Impact

- Startup orchestration in `src/main.rs`, Emby login/setup UI, Settings navigation, and `App` construction/loading.
- Configuration parsing and saving, legacy `token.json` migration, per-Service secret files, and cleanup of Service-owned persisted state.
- `App` ownership of general configuration and optional Emby runtime state instead of a mandatory `EmbyClient`.
- Local daemon startup, ctrl hello/authentication, remote-player connection setup, and capability negotiation.
- Existing Emby browsing and playback behavior must remain intact once the Emby Service reaches Ready.
- No new dependency or required redb/shared-state service.
