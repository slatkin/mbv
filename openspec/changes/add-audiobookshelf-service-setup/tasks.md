## 1. Prerequisite And Core API

- [ ] 1.1 Confirm #503 is applied and verified, then reconcile these tasks with the resulting Service setup, secret persistence, runtime state, setup-generation, and Services action seams before editing application code.
- [ ] 1.2 Add minimal Audiobookshelf `/api/me` wire types and a concrete mbv-core client that sends the API key as a Bearer token and returns only the authenticated user identity needed by this milestone.
- [ ] 1.3 Add bounded request handling and redacted error classification that distinguishes explicit authentication rejection from connectivity, server, protocol, and malformed-response failures without exposing Authorization values.
- [ ] 1.4 Extend the nearest existing HTTP-boundary tests to protect Bearer authentication, active-user decoding, rejection classification, and secret redaction with one compact case table or fixture set.

## 2. Setup And Credential Lifecycle

- [ ] 2.1 Connect Audiobookshelf's base URL and API-key secret to #503's singleton setup and mode-`0600` per-Service persistence without persisting `/api/me` profile data.
- [ ] 2.2 Implement the shared in-memory `/api/me` validator for new setup and repair so failed candidates cannot mutate a working setup, credential, identity, or Service-owned state.
- [ ] 2.3 Commit successful setup and same-server repair through the foundation's transactional lifecycle, and route different-server candidates through validated, confirmed Service replacement.
- [ ] 2.4 Wire confirmed Audiobookshelf removal through the foundation so setup, API key, runtime identity, and Audiobookshelf-owned local state are cleared without affecting Emby or Feeds.
- [ ] 2.5 Extend the nearest persistence/lifecycle tests for mode-`0600` API-key storage, failed-candidate retention, same-server repair, and destructive replacement/removal ordering.

## 3. Runtime Initialization And Connection Testing

- [ ] 3.1 Start configured Audiobookshelf validation independently after TUI entry, transition through the established Service states, and retain or clear the persisted key according to the validator's failure classification.
- [ ] 3.2 Carry the captured Audiobookshelf setup generation on startup and manual-test completion events and ignore stale results after repair, replacement, or removal.
- [ ] 3.3 Add the Services Test connection action using the same `/api/me` validator; report the configured server and authenticated user on success and apply normal persisted-runtime failure semantics.
- [ ] 3.4 Extend the nearest state-transition tests to protect rejected-key clearing, unavailable-key retention, independent initialization, and stale-result rejection without brittle rendered-UI assertions.

## 4. Services User Interface

- [ ] 4.1 Add Audiobookshelf setup and repair input for server URL plus API key, with no username/password flow and no reusable secret retained after submission, cancellation, or form teardown.
- [ ] 4.2 Present Not configured, Connecting, Ready, Needs authentication, and Unavailable with the applicable setup, repair, test, replace, and remove actions in the Audiobookshelf Services entry.
- [ ] 4.3 Present concise validation and connection-test results while keeping raw HTTP details, API keys, and Authorization values out of notifications and logs.
- [ ] 4.4 Confirm that reaching Ready exposes only Service identity and actions and does not request or render Audiobookshelf libraries, media, playback, or Socket.IO state.

## 5. Verification

- [ ] 5.1 Manually verify new setup, rejected and unreachable candidates, startup success, persisted-key rejection, unavailable-server retention, same-server repair, different-server replacement, removal, and stale-result races against the documented Service states.
- [ ] 5.2 Run `cargo check -p mbv-core`, relevant focused tests, `cargo clippy --workspace --all-targets`, and `make check-code-file-lines`; resolve all introduced failures.
