## Why

Large TV libraries are difficult to browse because the TV tab currently lacks the alphabet-range navigation available on the movie screen. Users need a fast way to jump among show names without paging through the entire library.

## What Changes

- Add movie-style alphabet range pills to eligible TV library tabs (`A-C`, `D-F`, through `V-Z`, plus `#`).
- Filter TV results by show name, using the series `SortName` range rather than episode or season names.
- Preserve the existing pill interaction model: mouse selection, keyboard cycling, default selection, loading state, and saved position restoration.
- Keep existing movie letter-pill behavior unchanged.

## Capabilities

### New Capabilities

- `tv-letter-filtering`: Alphabet-range navigation and server-side show-name filtering for large TV libraries.

### Modified Capabilities

- None.

## Impact

- TV library loading and refresh paths in the Rust application.
- Shared library pill rendering, cursor navigation, persistence, and mouse hit-testing.
- Emby item queries, using existing `NameStartsWithOrGreater` / `NameLessThan` range support with series-oriented results.
- Automated rendering and action tests for large TV libraries.
