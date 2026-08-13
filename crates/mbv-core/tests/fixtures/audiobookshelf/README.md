# Audiobookshelf contract fixtures

These minimal fixtures were captured on 2026-08-13 from live Audiobookshelf
2.36.0 responses with `audiobookshelf_contract_probe`, then reduced and
deterministically sanitized. IDs, device/user identity, titles, URLs, hostnames,
credentials, and filesystem paths are placeholders. `session-sync.json` and
`session-close.json` each preserve the two-byte `{}` body returned by the
corresponding successful request.

`authentication-failure.json` came from the live Service with an intentionally
invalid placeholder Bearer credential. `server-failure.json` and
`malformed-response.txt` came from controlled loopback HTTP responses because
inducing either condition in the live Service would be destructive or
nondeterministic; the probe observed the existing client classifications
`Server` and `MalformedResponse`, respectively.

The probe uses repository dependencies and boundaries only: setup via
`config::load_config`, credential access via `config::load_service_secret`,
catalog selection via `AudiobookshelfClient`, and playback via `libmpv2` 6.0.0
(`libmpv2-sys` 4.0.1; runtime mpv 0.41.0). Direct playback started with its
Bearer header configured before libmpv initialization and sought from 0 to 1
second. Forced-transcode HLS was ready on the first 250 ms REST-only poll within
a 20-second bound, started without a credential, and sought from 0 to 1 second.
No Socket.IO connection was made or required.

Both successful close responses were `{}`. Subsequent authenticated GETs for
each session returned 404, and the unauthenticated HLS path returned 404 after
close. The probe's tracked-open-session count was zero; its `Drop` guard also
best-effort closes every tracked session on every early return.
