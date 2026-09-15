## Context

`unify-screens-under-panel-components` archived 2026-09-13 at the shape this rename assumes: `BrowserComponent` and `ComponentId::Browser` are deleted (its tasks 8.4/12.6), the plain Movies/HomeVideos/Generic kinds are served by the embedded `BrowserContent` owner through the mounted `LibraryPanel`, and TV/Music serve their libraries through `TvContent`/`MusicContent`. What remains of the `Browser*` family is: the identity types (`BrowserKey`, `BrowserKind` — both in `component_id.rs`, with `BrowserKey` wrapped by `LibraryKey::Service(_)`), the owner (`BrowserContent`, `BrowserIdentity`), the shell request family (`Browser*` variants in `msg/shell.rs`, handled by `shell_browser.rs::handle_browser_request` plus three mouse arms in `shell_messages.rs`), the wiring modules (`shell_browser.rs`, `shell_browser_content.rs`), and stale comments in ~18 files. ~122 `BrowserKey`/`BrowserKind` and ~27 `BrowserContent` references; `mbv-core` has none.

Facts that constrain the naming:
- `BrowserKey { service, library_id, kind }` keys **Audiobookshelf** libraries too (`shell_audiobookshelf_book.rs`, `shell_audiobookshelf_podcast.rs` construct it), so any "Emby*Key" name would be wrong.
- The panel's `LibraryKey` (`Home | Feeds | Service(BrowserKey)`, `library_panel/owner.rs`) already owns the "library" word on the panel side.
- The six effect requests `BrowserPlay/Enqueue/ToggleWatched/Shuffle/Refresh/Rescan` are payload-identical to `EmbyLibrary*` twins that `MusicContent` and `TvContent` already emit; `shell_browser.rs` matches every pair as `BrowserX | EmbyLibraryX` arms. The twins exist only because the `Browser*` prefix was taken (msg/shell.rs comment: "reserving the `EmbyLibrary*` prefix because `LibraryRoutes*` already occupies the `Library*` namespace").
- `BrowserCursorIndex` has a second emitter: `tv_content/keyboard.rs` (TV's narrow list persistence). Any rename must stay accurate for TV.

User-confirmed decisions (2026-09-14): `EmbyLibraryContent`; fold `BrowserKey` into `LibraryKey`; unify requests on `EmbyLibrary*`.

## Goals / Non-Goals

**Goals:**
- One mechanical, compile-driven rename leaving behaviour, payloads, dispatch targets, and App effects byte-identical.
- Domain vocabulary that reads correctly at every site: `EmbyLibraryContent` (the plain-kind owner), `LibraryKind` (behavioural category of a service library, any service), `LibraryKey::Service { .. }` (the identity), `EmbyLibrary*` (the one request family for Emby-library effects).
- Specs, CONTEXT.md, and the ledger stop naming deleted or renamed identifiers.

**Non-Goals:**
- No behaviour change, no state reshaping beyond the `BrowserKey` fold, no new messages, no key-policy or arbitration changes (the just-archived `unify-semantic-input-arbitration` surfaces are untouched structurally — only identifier text changes).
- `InlineMediaBrowser`, the **Service browse dispatch / Browse target** and **Browse level** glossary entries, and ADRs stay as they are.
- The mouse-input verification table's stale pre-unification names in untouched rows (Home, Music, Feeds, Audiobookshelf) — separate docs pass.
- Stale `MusicWorkspaceComponent` comments — separate docs pass (sweep only `BrowserComponent` comments in files the rename already touches).

## Decisions

**D1 — Owner: `BrowserContent` → `EmbyLibraryContent`.** It names the tab the owner serves (`TabSelection::EmbyLibrary`) and the App's own vocabulary (`emby_library_index`, `EmbyLibrary*` requests). The doc comment states the precise contract: it serves the Generic/Movies/HomeVideos kinds; TV and Music libraries have their own specialized owners. File `components/emby_library_content.rs`; `BrowserIdentity` → `EmbyLibraryIdentity`; wiring `shell_emby_library.rs` (routing) + `shell_emby_library_content.rs` (projection), functions renamed to match (`handle_emby_library_request`, `push_emby_library_owner_content`, `active_migrated_emby_library_owner`, …).
*Alternative rejected:* `MediaLibraryContent` — "media library" is exactly the kind of vague umbrella this issue removes. *Alternative rejected:* per-kind owners (`MoviesContent`/…) — the three kinds share one browse model (nav-stack `BrowseLevel`s, letter pills, feed groups); three owners would duplicate it for zero benefit.

**D2 — Identity: delete `BrowserKey`, fold into `LibraryKey::Service { service, library_id, kind }`; `BrowserKind` → `LibraryKind`.** One type leaves the domain instead of being renamed; the `LibraryKey::Service(ServiceLibraryKey)` double-wrap disappears. `UserEvent::LibraryReady(LibraryKey, Generation)` and `ShellRequest::LibraryScroll { key: LibraryKey, .. }` carry the panel key; only the `Service` variant is ever constructed at those sites today, and exhaustiveness checking flags any site that needs narrowing. `LibraryKind` keeps all seven variants and `from_collection_type` unchanged, and lives beside `LibraryKey` in `library_panel/owner.rs` (its only remaining raison d'être is the panel key; `component_id.rs` keeps `ComponentId`/overlay/modal ids).
*Alternative rejected:* standalone `ServiceLibraryKey` — same rename churn, one more type, redundant reading inside `LibraryKey::Service(..)`.

**D3 — Requests: unify on one `EmbyLibrary*` set.** Delete the six twins; rename the eight no-twin variants (`BrowserActivate`→`EmbyLibraryActivate`, `BrowserBack`→`EmbyLibraryBack`, `BrowserCycleLetterPill`→`EmbyLibraryCycleLetterPill`, `BrowserCycleGroup`→`EmbyLibraryCycleGroup`, `BrowserRowClick`→`EmbyLibraryRowClick`, `BrowserRowActivate`→`EmbyLibraryRowActivate`, `BrowserPillClick`→`EmbyLibraryPillClick`, `BrowserCursorIndex`→`EmbyLibraryCursorIndex`). A pure rename of `Browser*` to `EmbyLibrary*` is impossible while the twins exist (duplicate variant names), and keeping two mirrored sets preserves the very duplication the collision caused. `EmbyLibraryCursorIndex` is accurate for its TV emitter too — TV is an Emby library, and the message means "Emby-library browse cursor resolved for persistence". `handle_browser_request` → `handle_emby_library_request`; its merged arms collapse to single variants.
*Alternative rejected:* rename-only with two sets — leaves six duplicate variants and merged dispatch arms that read as accidental.

**D4 — Spec-text corrections where the rename would otherwise preserve a false statement.** The mouse-input verification table's plain-kinds row and Wide-TV row describe the pre-unification world (`BrowserComponent` mounted for narrow TV). The delta re-names identifiers **and** drops the stale "including narrow TV" / "narrow ownership is Browser" claims (narrow TV is `TvContent`'s at every width since the predecessor's task 8.1) — truth-alignment forced by touching these cells, not a behaviour change. The library-panel requirement is RENAMED to "The list pane …" (the pane serves Home and Feeds too, so "Browser" was never its name) with scenarios unchanged.

**D5 — Order of operations: types first, then messages, then files, then comments.** The compiler drives each step: (1) `LibraryKind` + `LibraryKey::Service{..}` fold → fix constructors; (2) request variants → fix emitters and dispatch arms; (3) `git mv` modules and test files; (4) sweep stale `BrowserComponent` comments in touched files. Each step ends `cargo check -p mbv` green, so a bisect between steps still builds. `cargo fmt` and clippy `-D warnings` gate each commit-worthy chunk.

## Risks / Trade-offs

- [Message-variant collapse touches the fresh arbitration/dispatch surface] → Payloads and routing are identical; the full `cargo nextest run -p mbv` suite (1455 tests, incl. the routing-matrix and tick-integration families) must be green before any commit; no key-policy or arbitration file is edited beyond identifier text.
- [Broad textual overlap with `Library*` names] → `LibraryKind` vs `LibraryKey` are a deliberate, related pair (a key and the kind it carries); `LibraryRoutes*` popup messages and `LibraryPanelContent` remain distinct and untouched. Compile errors catch any accidental capture during the mechanical pass.
- [Stale-comment sweep could balloon] → Sweep only `BrowserComponent` mentions in files the rename already modifies; the remaining stale `MusicWorkspaceComponent`/`TvWorkspaceComponent` comments are recorded as a separate docs pass.
- [Worktree has unrelated in-flight edits] → The rename commits touch only `src/app`, docs, and openspec; the pre-existing dirty player/queue files are left uncommitted by the implementer and never staged.

## Migration Plan

Single-repo, single-commit-series change; no data, protocol, or config migration. Rollback = revert the series. After the final commit, the rename is complete — no deprecation shim, no alias: this codebase has no external API surface.

## Open Questions

None — the three material naming decisions were confirmed with the user before proposal time; everything else is mechanical.
