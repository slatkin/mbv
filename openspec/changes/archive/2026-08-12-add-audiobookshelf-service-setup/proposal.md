## Why

Once Service-independent startup establishes independently configured Remote Services, mbv needs to prove that the model supports a second authentication scheme without coupling startup back to Emby. Audiobookshelf API-key setup and `/api/me` validation provide that proof before catalog or playback behavior is introduced.

Tracking issue: [#505](https://github.com/slatkin/mbv/issues/505)

Prerequisite: [#503](https://github.com/slatkin/mbv/issues/503) and its `service-independent-startup` change must be applied and verified before this change is implemented.

## What Changes

- Add Audiobookshelf setup in the Services view using a server URL and API key, with no username/password flow.
- Validate setup through authenticated `GET /api/me` and commit it only after confirming the associated active user.
- Persist the API key in Audiobookshelf's isolated mode-`0600` Service secret file and keep it out of general configuration, ctrl messages, logs, and shared state.
- Initialize configured Audiobookshelf independently after TUI entry and classify connection results through the Service states established by #503.
- Add an Audiobookshelf Test connection action that reports the authenticated server and user without changing a working setup.
- Reuse the Service repair, replacement, and removal lifecycle established by #503.
- Exclude Audiobookshelf libraries, catalog UI, queue items, playback sessions, Socket.IO updates, and Local daemon media support.

## Capabilities

### New Capabilities

- `audiobookshelf-service-setup`: Audiobookshelf API-key setup, `/api/me` identity validation, runtime connection state, credential handling, and connection testing within Services settings.

### Modified Capabilities

None.

## Impact

- Audiobookshelf API types and a concrete client limited to authenticated identity requests.
- Singleton Audiobookshelf setup, secret persistence, and runtime state introduced by #503.
- Services settings actions, setup input, connection result presentation, and destructive lifecycle integration.
- Remote Service background initialization and setup-generation reconciliation.
- No ctrl protocol, playback queue, catalog navigation, shared-state identity, Local daemon capability, or new dependency changes.
