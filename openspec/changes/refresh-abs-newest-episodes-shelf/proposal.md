# Proposal

## Why

Refreshing an Audiobookshelf podcast library reloads its shows but never refetches the library's `/personalized` Newest Episodes shelf, which is fetched only once at catalog startup. The podcast destination's Latest pill reads that cached shelf, so an explicit refresh can leave Latest stale for the rest of the session (#763).

## What Changes

- An explicit refresh of a podcast library also re-requests that library's Newest Episodes shelf, scoped to that library only.
- The refetched shelf is accepted only for the current Service setup generation (the existing stale-result guard).
- The cached shelf stays visible until the new result lands; a failed refetch keeps the previous shelf.
- The Latest pill selection and its run-scoped marker acknowledgement survive the refresh and the shelf replacement.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `audiobookshelf-podcast-browsing`: explicit refresh also refreshes the library's Newest Episodes shelf.

## Impact

- `src/app/dispatch/audiobookshelf/browse.rs` (`audiobookshelf_refresh`): one added shelf request.
- New hermetic tests under `src/app/tests/`; no API, dependency, or persistence change.
- Independent of the archived `migrate-home-latest-to-destinations` change, which moved the cached shelf to the destination without adding a refetch.
