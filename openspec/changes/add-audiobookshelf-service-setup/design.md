## Context

See `proposal.md` for motivation and `specs/audiobookshelf-service-setup/spec.md` for behavior. This change is implemented only after #503 establishes singleton Service setup records, per-Service secret paths, runtime Service states, setup generations, transactional lifecycle actions, and post-TUI background initialization.

Audiobookshelf 2.36 accepts an API key as a Bearer token and exposes the associated user through `GET /api/me`. This milestone needs no library, media, playback-session, or Socket.IO API. The API key is already a Service credential; mbv does not exchange username/password for another token.

## Goals / Non-Goals

**Goals:**
- Add the smallest concrete Audiobookshelf API seam that later catalog and playback changes can extend.
- Use one identity-validation path for setup, startup initialization, repair, and Test connection.
- Preserve the transactional and generation-safe Service lifecycle established by #503.
- Keep the API key confined to Audiobookshelf's secret boundary.

**Non-Goals:**
- A generic media-Service trait or a common Emby/Audiobookshelf wire model.
- Persisted Audiobookshelf user profiles or server catalogs.
- Audiobookshelf queue identity, playback URL resolution, progress reporting, or playback sessions.
- Passing Audiobookshelf setup or credentials to a Local daemon or packaged `mbvd`.
- Real-time refresh through Socket.IO.

## Decisions

### Decision 1: Add a concrete minimal Audiobookshelf client in mbv-core

Add concrete Audiobookshelf request and response types alongside the existing core API boundary. Its initial authenticated operation accepts a server URL and API key, calls `/api/me`, and returns only the identity fields needed to confirm and display the authenticated user.

Keeping the client in mbv-core makes the API contract reusable by later Player-owner work without moving TUI concerns into core. A broad provider trait was rejected because #503 deliberately preserves provider-specific runtimes and this milestone has no shared catalog or playback behavior to abstract. A TUI-local HTTP call was rejected because later playback owners will need the same authenticated Service boundary.

### Decision 2: Persist only the server setup and API key

The non-secret Audiobookshelf setup contains the configured base URL. The API key uses the Audiobookshelf secret path introduced by #503. The user ID and display name returned by `/api/me` remain runtime identity derived on every successful validation rather than becoming another persisted source of truth.

This avoids stale profile metadata and keeps setup repair simple: the key identifies its current user on the server. Persisting the `/api/me` response was rejected because no behavior in this milestone works without a current successful connection.

### Decision 3: One validator separates candidate setup from persisted runtime failure

Use one bounded `/api/me` validator for setup, repair, startup initialization, and Test connection. It produces a successful active-user identity, explicit authentication rejection, or an availability/protocol failure. Authentication rejection includes responses that explicitly deny the Bearer credential or indicate that its user cannot authenticate; transport failures, timeouts, server errors, unexpected responses, and malformed success bodies do not prove that the key is invalid.

Candidate setup and repair run entirely in memory until validation succeeds. Any candidate failure leaves an existing working setup untouched. Once a persisted key is under test, explicit rejection follows #503 by clearing only that secret and entering Needs authentication; every other failure preserves setup and secret and enters Unavailable.

Separate validators for setup and runtime were rejected because their error classification and identity checks would drift. Treating every non-success response as bad authentication was rejected because transient server and compatibility failures must not destroy a credential.

### Decision 4: Reuse the foundation's transactional lifecycle commit

After candidate validation succeeds, commit setup and secret through #503's per-Service persistence transaction rather than writing Audiobookshelf files directly from UI code. Repair against the same configured server replaces only the validated secret and runtime identity. A different server URL enters the confirmed replacement path; only after validation and confirmation does the foundation clear old Service-owned state and commit the replacement. Removal uses the same confirmed cleanup path.

This keeps write ordering, rollback on persistence failure, permission enforcement, and setup-generation advancement consistent with Emby. An Audiobookshelf-specific replacement mechanism was rejected because it would duplicate the security-sensitive lifecycle contract.

### Decision 5: Every request result carries the captured setup generation

Startup initialization and Test connection capture the current Audiobookshelf setup generation before launching their bounded request. Their completion event includes that generation. The App applies identity, state, and credential changes only when it still matches the current setup.

This extends #503's stale-result protection to manual tests and repair races. Cancelling tasks alone was rejected because cancellation does not guarantee that an already-completed result cannot be delivered.

### Decision 6: Test connection is an explicit validation, not a separate health endpoint

The Services action invokes the same authenticated `/api/me` validator. Success reports the configured server plus the returned user and leaves the working setup unchanged. Failure applies the persisted-runtime classification, including clearing an explicitly rejected key.

A separate unauthenticated health request was rejected because it would not test the credential or prove the user identity. Silently treating Test connection as read-only on credential rejection was rejected because it would leave the Service displaying a credential already known to be invalid.

### Decision 7: Keep UI state free of reusable secret values

The setup form owns the entered API key only while editing and submitting. It passes the value to validation without copying it into general App diagnostics, runtime identity, or rendered result state, then clears the input on success, cancellation, or form teardown. Request errors are reduced to redacted classifications before reaching user-visible notifications or logs.

Retaining the key in the Service runtime was rejected because the persisted secret boundary and request construction can provide it when needed. Including raw HTTP debug values was rejected because common client diagnostics can expose Authorization headers.

## Risks / Trade-offs

- **[Risk] Audiobookshelf changes `/api/me` or its user shape** -> Keep response decoding minimal, classify incompatible responses without deleting the key, and confine future compatibility edits to the concrete client.
- **[Risk] A stale startup or Test connection result overwrites repaired setup** -> Require the captured setup generation on every completion before applying any state or secret mutation.
- **[Risk] Candidate replacement partially updates files** -> Use #503's transactional Service commit and preserve the existing setup until validation and confirmation have completed.
- **[Risk] Diagnostics expose the API key** -> Never place secrets or Authorization values in error types; verify redaction at the API boundary and persistence tests.
- **[Trade-off] Ready exposes identity but no content** -> Keep the Services result useful while deferring all catalog requests to the next roadmap milestone.
- **[Trade-off] User identity is fetched on every initialization** -> Accept one bounded `/api/me` request to avoid persisted stale profile data.

## Migration Plan

1. Apply and verify #503, including Audiobookshelf's Not configured entry, per-Service secret storage, lifecycle actions, setup generations, and independent worker seam.
2. Add the concrete identity client and error classification without enabling catalog requests.
3. Wire transactional setup, repair, replacement, removal, startup initialization, and Test connection through the foundation.
4. Enable the Audiobookshelf Services actions after persistence, redaction, race, and failure-path verification passes.

There is no existing Audiobookshelf data to migrate. Rollback disables the integration; its additive setup and secret remain inert and can be removed through Service removal before rollback when complete cleanup is required.
