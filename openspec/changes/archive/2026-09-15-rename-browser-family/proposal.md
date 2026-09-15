## Why

The `Browser*` naming family (`BrowserContent`, `BrowserKey`/`BrowserKind`, the `Browser*` shell requests, and the `browser*` modules) carries almost no domain meaning and collides with two established glossary concepts — **Service browse dispatch / Browse target** and **InlineMediaBrowser** — that mean something else. Since `unify-screens-under-panel-components` archived (2026-09-13), the "browser" is not one thing: the mounted `LibraryPanel` owns painting and routing, and per-kind embedded content owners own state and typed content. The predecessor deliberately deferred this rename so its tasks 8–13 would not churn; it has archived and its residual `BrowserComponent` was already deleted, so the rename is now safe and the codebase is at its post-migration shape.

Decisions confirmed with the user (2026-09-14): owner named `EmbyLibraryContent`; `BrowserKey` folded into the panel's `LibraryKey` with `BrowserKind` → `LibraryKind`; the payload-identical `Browser*`/`EmbyLibrary*` request twins unified on one `EmbyLibrary*` set.

## What Changes

- Rename the plain Emby-library content owner: `BrowserContent` → `EmbyLibraryContent` (`components/browser_content.rs` → `emby_library_content.rs`), `BrowserIdentity` → `EmbyLibraryIdentity`; shell wiring modules `shell_browser.rs` → `shell_emby_library.rs` and `shell_browser_content.rs` → `shell_emby_library_content.rs` (function names follow: `handle_browser_request` → `handle_emby_library_request`, `push_browser_owner_content` and siblings).
- Delete the `BrowserKey` struct: its `{ service, library_id, kind }` fields fold into `LibraryKey::Service { .. }` (owner.rs). `BrowserKind` → `LibraryKind` with the same seven variants (`Generic`, `Movies`, `TvShows`, `Music`, `HomeVideos`, `AudiobookshelfPodcast`, `AudiobookshelfBook`); `UserEvent::LibraryReady` and `ShellRequest::LibraryScroll` carry `LibraryKey`.
- Unify the shell request family on one `EmbyLibrary*` set. Delete the six payload-identical `Browser*` twins (`BrowserPlay`, `BrowserEnqueue`, `BrowserToggleWatched`, `BrowserShuffle`, `BrowserRefresh`, `BrowserRescan`) whose `EmbyLibrary*` twins Music/TV already emit; rename the eight no-twin variants (`BrowserActivate`, `BrowserBack`, `BrowserCycleLetterPill`, `BrowserCycleGroup`, `BrowserRowClick`, `BrowserRowActivate`, `BrowserPillClick`, `BrowserCursorIndex`) to `EmbyLibrary*`. Payloads, dispatch targets, and App effects are unchanged; this is a rename, not a behaviour change.
- Update `CONTEXT.md` (the Inline Search entry still says "Browser, MusicWorkspace, or TvWorkspace"; verify no entry canonizes the old names) and `docs/architecture/interactive-surface-ledger.md`.
- Update the two main specs that name renamed identifiers: `mouse-input` (wheel-verification table rows using `BrowserComponent`, "Browser", `BrowserCursorIndex`) and `library-panel` (the "Browser pane" requirement re-titled to the list pane vocabulary).
- Sweep the stale `BrowserComponent` doc comments left by the predecessor's deletions (the type no longer exists) in files the rename touches.

Out of scope: `InlineMediaBrowser` (a distinct, correctly-named canonical media-list concept); the glossary **Service browse dispatch / Browse target** and **Browse level** entries (unaffected concepts); ADRs (historical records); per-kind owner splits (the three kinds share one browse model — one owner stays); the mouse-input table's other stale pre-unification component names in untouched rows (Home, Music, Feeds, Audiobookshelf), which predate this change and belong to a separate docs pass.

## Capabilities

### New Capabilities

(none — no new behaviour)

### Modified Capabilities

- `mouse-input`: the "Wheel behavior is verified for each scrollable surface" verification record re-names the plain-Emby-library row's owner and resolved request (`BrowserComponent`/`Browser`/`BrowserCursorIndex` → `EmbyLibraryContent`/Emby-library vocabulary/`EmbyLibraryCursorIndex`); behaviour unchanged.
- `library-panel`: the "The Browser pane has one Selector row and one optional List controls row" requirement is re-titled and re-worded to the list pane vocabulary; the skeleton it mandates is unchanged.

## Impact

- `src/app`: `components/component_id.rs` and `components/library_panel/owner.rs` (identity types), `components/browser_content.rs` (renamed + moved), `components/msg/shell.rs` (request variants), `shell_browser.rs`/`shell_browser_content.rs` (renamed + moved), `shell_messages.rs` dispatch arms, `tv_content/keyboard.rs` and `music_content.rs` (emitter sites), plus constructors in the shell wiring files and test helpers; test files renamed with their subjects (`tests_tick_integration_browser.rs`, `components/browser_inline_search_tests.rs`, stale comments in `tests_narrow_browse_migration.rs` and ~16 other files).
- Docs/specs: `CONTEXT.md`, `docs/architecture/interactive-surface-ledger.md`, `openspec/specs/mouse-input/spec.md`, `openspec/specs/library-panel/spec.md`.
- No `mbv-core` changes (it has zero `Browser*` references); no protocol, persistence, or config format changes.
- Gates: full `cargo nextest run -p mbv` green, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `openspec validate --strict`.
