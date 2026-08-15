## Context

Podcast support (#504 milestones 1-5) already gives Audiobookshelf its Service lifecycle, `TabSelection::AudiobookshelfLibrary(usize)`, browse dispatch, catalog client (`crates/mbv-core/src/audiobookshelf_catalog.rs`), playback resolution (`audiobookshelf_playback.rs`), and daemon/ctrl progress reconciliation (`ctrl.rs::AudiobookshelfProgressEvent`). All of it is episode-shaped: `episode_id: String` is load-bearing through `AudiobookshelfProgress`, `QueueItemContentId::Audiobookshelf`, `AudiobookshelfQueueItem`, `AudiobookshelfProgressEvent`, and the playback URL itself (`/api/items/{library_item_id}/play/{episode_id}`). The `media_type == "podcast"` filter at `run_loop_drains.rs:60` currently drops book libraries before they ever reach a tab.

See proposal.md for motivation. The UI/data-model decisions behind this design were worked out in #536 ahead of implementation; this document explains how they translate into the existing architecture.

## Goals / Non-Goals

**Goals:**
- Reuse the existing Service lifecycle, browse-dispatch, and Player-owner/daemon playback machinery rather than building a parallel path for books.
- Keep book identity, queue items, and progress fully isolated from the episode-shaped podcast types so neither can be matched against the other by accident.
- Land in one milestone-sized change, unlike podcast support's three-milestone rollout (browse, local playback, daemon playback + live refresh).

**Non-Goals:**
- Socket.IO live progress refresh for books (podcast's own live-refresh milestone (#504 milestone 5) covers only podcast episodes today; extending it to books is a later increment if wanted).
- A resume-emphasizing hero treatment (decision 2 in #536 explicitly defers this).
- Series, narrator, or fiction/non-fiction grouping (decision 4 defers this).
- Full daemon-lifecycle edge-case coverage matching every scenario `audiobookshelf-podcast-playback` accumulated across its three real delivery increments (transient-failure preservation, setup-replacement purge, reattachment mid-session, etc.). The `audiobookshelf-book-playback` capability specifies the core play/seek/progress/finalization contract for this milestone; those daemon-hardening requirements can be added as a follow-up increment the same way they were for podcasts, informed by real usage.

## Decisions

### Lift the podcast-only filter, keep books and podcasts as sibling tab kinds
`run_loop_drains.rs:60` drops `library.media_type != "podcast"`. Removing that filter, in server order, is sufficient for decision 1 (#536) — no additional sort/group pass. `TabSelection::AudiobookshelfLibrary(usize)` already carries a library index; resolving `media_type` once at that point (decision 3) and branching browse state, renderers, and input handlers on the resolved kind is the same shape `service-browse-dispatch` already uses to separate Emby from Audiobookshelf — this change extends that fork one level, it doesn't invent a new one.

### Book hero reuses the Music layout components, not a new layout system
Decision 2 calls for the Music wide two-column hero-on-left mapping at the existing `TWO_COLUMN_THRESHOLD` breakpoint (`layout.rs`). The book tab substitutes book/chapter/author for album/track/artist in that composition (see the `audiobookshelf-book-browsing` spec's substitution table) rather than adding a new breakpoint or geometry.

### Author-surname grouping computed once at catalog build
Decision 4: surname is the title-cased final whitespace token of the first-listed author (no name-parsing dependency), falling back to the raw credit when nothing can be extracted. Store `author_display` (raw credit) and `author_sort_key` (surname) per book at catalog-build time, mirroring `music_group.rs::build_grouped_album_catalog`'s existing split between display and sort key, rather than re-parsing on every render.

### Books get a new `QueueItemKind::AudiobookshelfBook`, not `episode_id: Option<String>`
Decision 6, re-confirmed during design: `episode_id: String` is structural in five existing types plus the podcast play URL itself, not just a progress label. Threading `Option<String>` through all of them would leave every existing match/HashMap-key call site to reconsider a `None` case that was previously impossible by construction. A sibling `QueueItemKind`/`QueueItemContentId`/queue-item struct/progress-event, keyed by `library_item_id` only, keeps both shapes honest about what they identify and follows the same fork-once-and-never-recheck pattern already used for browse dispatch. Cost: some duplication between the two `Audiobookshelf*` progress/queue-item types; accepted because the two media kinds have genuinely different identity, not just an optional field.

### Multi-file books use mpv's native multi-file projection, not manual offset math
Decision 5: hand a book's `audioFiles` to mpv as one continuous timeline (mpv playlist/EDL, already reachable through embedded libmpv2) so mbv keeps one queue item and one playback session across file boundaries, with chapter rows issuing absolute seeks against `chapters[].start`. Whether that's a `loadfile` playlist with a shared `Authorization` header per entry, or an `edl://` URL, is resolved during implementation — see Open Questions.

### No "browsable" filter for books
Decision 7: dropped rather than answered. Podcast's downloaded-episode boundary exists because an RSS episode can be library-visible without ABS having fetched it; books have no analogous sub-item readiness state. A missing book file surfaces as a request error from ABS, handled the same way mbv already handles a missing Emby file — no new filter concept.

## Risks / Trade-offs

- [Two near-identical `Audiobookshelf*` type families (episode-shaped and book-shaped) increase surface area] → Accepted per the sibling-variant decision above; keep both thin and let shared logic (HTTP client plumbing, bounded-worker dispatch, progress-apply generation gating) stay factored under `audiobookshelf_catalog.rs` / `audiobookshelf_playback.rs` rather than duplicated.
- [Merged-timeline mpv projection is unproven for Audiobookshelf's authenticated per-file URLs] → Resolved during implementation: `merge-files=yes` plus per-source `loadfile ... append-play` commands collapses the playlist into a single `edl://` entry (verified against mpv 0.41: one playlist entry, whole-book duration, absolute `seek` across file boundaries). Every source carries the same Bearer `Authorization` header via `http-header-fields` (single server/token per book), so no `edl://` URL syntax is needed. `merge-files` is reset to `no` on non-book loads.
- [Scoping daemon-lifecycle hardening out of this increment] → Bare-mode (in-process Player owner) playback is the increment's floor; daemon-owned book playback should reuse the exact same eligibility/finalization mechanics the podcast capability already proved, so the risk is schedule, not a new architecture.

## Open Questions

- None remaining. The merged-timeline question (see Risks / Trade-offs) was resolved during implementation by testing against mpv; no other open question carries into the shipped behavior.
