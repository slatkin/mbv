# Handoff: fix-music-artist-resolution-batching

Status: change scaffolded (`openspec new change` run). **No artifacts written yet** —
no proposal.md, design.md, specs/, or tasks.md. Start there.

## The problem, in one line

Grouped Music resolves album artists with one HTTP request *per album*, which is
~55x slower than one request per folder level, and so slow that the 3s settle
window always expires on large levels — meaning the majority-vote artist feature
mostly does not run, and grouping falls back to folder-name parsing
nondeterministically.

## Evidence (measured 2026-09-19 against the user's live Emby, LAN)

Server `https://emby.poo.town`, user `slatkin`
(`uid 52a516340de04827b9c76b2c690a957e`), music library `ParentId=495186`.

| approach | Hip-Hop (61 albums) | Classical (142 albums) | whole library |
|---|---|---|---|
| N+1 @ concurrency 6 (today) | **5.36s** | **20.96s** | — |
| one request per level | **0.10s** / 822KB | **0.38s** / 3.8MB | — |
| one request, whole library | — | — | **2.88s** / 26MB, 33,611 tracks |

Verified facts:

* Album-level items are plain `Folder` with `AlbumArtist: None`. Emby has **not**
  identified them as `MusicAlbum` — this is a folder-view music library
  (`/mnt/media/music/<Genre>/<Artist> (<Year>) <Album>`). So deriving the artist
  from child tracks is genuinely required; you cannot just request a field.
  The existing `parse_album_folder_name` fallback reads those dir names.
* `ParentId` does **not** accept a comma-separated list → HTTP 500. The batch
  unit cannot be an arbitrary set of albums; it must be an ancestor folder.
* Bucketing Audio rows by their `ParentId` yields exactly 1:1 album buckets:
  61/61 and 142/142.
* **Semantic equivalence proven**: batched vote vs today's per-album vote over
  Hip-Hop = **61/61 identical artists**, using the same `SortBy=
  ParentIndexNumber,IndexNumber` and the same first-5-tracks cap per bucket.
* Nested-subfolder edge case is real but rare: **1 orphan bucket in 243**
  (14 tracks, a multi-disc set whose tracks' `ParentId` is a disc subfolder, not
  an album at the level). Needs handling — walk up, or prefix-match `Path`.
* Level sizes are modest: root = 15 genre folders; Classical 142, Hip-Hop 61,
  Comps 40 albums.

Reproduce with:
`/Users/{uid}/Items?ParentId={level}&IncludeItemTypes=Audio&Recursive=true&Fields=AlbumArtist,Artists,ParentId&Limit=100000`

## Current implementation

* `src/app/image_fetch.rs:136` `spawn_album_artist_fetch` — per album, fetches
  `ParentId={album}&IncludeItemTypes=Audio&Limit=5&Fields=AlbumArtist,Artists`,
  then a **majority vote** over ≤5 tracks' `AlbumArtist` (falling back to
  `Artists[0]`), first-seen wins ties. This vote is deliberate — "so one
  outlier/mistagged track can't poison the whole album's displayed artist".
  **Keep it exactly.**
* `src/app/image_fetch.rs:109` `fetch_album_artist`, `:127`
  `drain_album_artist_fetches`, `MAX_ALBUM_ARTIST_FETCHES = 6`
  (`src/app/images.rs:84`), `pending_album_artist_fetches` queue.
* `src/app/music_grouping.rs:14` `SETTLE_WINDOW = 3s`; `MusicGroupCandidate`
  with `unresolved`/`resolved` sets; commit gated on `unresolved.is_empty()`;
  `:230` and `:260` force `unresolved.clear()` once the window expires.
* `album_artist_cache` is in-memory, keyed by album folder id. Not persisted.
* Level config: `music_levels` (e.g. `["group","album",...]`),
  `is_music_group_view` (`src/app/music_actions.rs:9`),
  `is_viewing_album_folders` (`src/app/lib_cursor_actions.rs:7`).

## Intended change

1. Fill the artist map with **one recursive Audio request per folder level**,
   bucket by `ParentId`, apply the *same* first-5 + majority vote per bucket.
2. **Warm the cache at startup in the background**, per level (15 requests,
   ~3s total), so browsing never waits. User explicitly asked for this: "why do
   we wait until libraries are shown to lazily load them? this is a client and
   it's on a LAN." Per-level rather than one whole-library request keeps peak
   memory at ~3.8MB instead of 26MB — relevant to the #656 footprint work.
3. Handle orphan buckets (tracks under a nested disc folder).
4. `SETTLE_WINDOW`, `MAX_ALBUM_ARTIST_FETCHES`, the pending queue and much of
   the candidate/revision machinery become vestigial once arrivals are a single
   event per level. Confirm before deleting — see the spec question below.

Explicitly **not** in scope: changing the grouping itself, the level config, the
vote, the fallback, or the folder-browse model. The user was emphatic: the
grouping layer mbv adds on top of Emby is required and deliberate; "brittle"
meant fix the mechanism, not excise the feature. Do not propose replacing it
with Emby's native music endpoints (`/Artists` etc.) — that was raised and
rejected.

## Open question for the planner: does any spec change?

`openspec/specs/stable-music-library-grouping/spec.md` (7 requirements) already
describes the settle-and-commit behavior, including:

> **Requirement: Settled initial music grouping** … publish … only after every
> album … has a terminal grouping identity … **Scenario: Artist lookup cannot
> supply metadata** — WHEN artist metadata cannot be obtained within the
> grouping resolution window THEN … deterministic fallback … without waiting
> indefinitely

Batching does not change *what* is published — it changes how fast the map fills
and makes the timeout path rare rather than dominant. So this may be a pure
implementation change requiring `skip_specs: true` in `.openspec.yaml`.

Decide deliberately:
* If the resolution window is **kept** as a safety net → likely no spec delta.
* If it is **removed** → the "grouping resolution window" scenario changes and
  `stable-music-library-grouping` needs a delta spec.
* Startup prefetch may itself be a new requirement (grouping is warm before the
  library is first shown) — possibly under `stable-music-library-grouping` or
  `service-independent-startup`.

Do not invent a requirement just to satisfy validation.

## Verification notes

* Unit tests are mocks only; no live servers (AGENTS.md). The measurements above
  were manual probes, not tests, and must not be committed as tests.
* Existing coverage: `src/app/tests_music_grouping.rs`,
  `src/app/render/tests_music_groups.rs`,
  `src/app/shell_music_workspace_owner_tests.rs`.
* `cargo nextest run -p mbv` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt`.

## Session context

Reached via a long exploration that first considered MPD (#738) and Mopidy as a
fourth content source. **Both were ruled out** and #738 should probably be closed:
no music server exposes playable audio to a third-party client. MPD's protocol
returns only `music_directory`-relative paths (and its `config` command is
local-socket-only), and Mopidy's `LibraryController` exposes `browse`/`lookup`/
`search`/`get_images` but no audio URL — translation to a playable URI happens
inside `PlaybackProvider.translate_uri`, documented as backend-internal. The
user's framing: a service manages media *and* offers access to play it; the
client decides how it plays. Only Emby and ABS satisfy both halves.

That exploration is background only — it is not part of this change.
