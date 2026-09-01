## Context

This slice is the third family slice of the canonical-list migration. It repairs the currently broken Audiobookshelf Books and Podcasts Wide/layout paths while bringing grouped Music into the shared composition. Standalone #640 is superseded. Feeds Service and Emby homevideos feed view remain separate surfaces.

## Decisions

### D1 — Shared arrangements, no exception
Use the existing Hero-on-left and Inline arrangements and embedded `WideMediaList`/`InlineMediaBrowser`. Extend shared placement policies when needed; do not add a destination-sized list or bespoke exception. Wide Book selected-row replacement is removed.

### D2 — Provider workspaces stay authoritative
Music grouping and tracks; Podcast show selection, episode list/filter, images; and Book details, chapters, audio files, and chapter seeking remain provider-owned. The canonical controls receive prepared rows and opaque targets and emit typed intents. Shell retains Service/effects/playback/persistence authority.

### D3 — Characterize Music before re-anchor
Before replacing Music handoff, capture metadata-bearing grouped Music at Wide/Normal and breakpoint transitions, including selected target, cursor, scroll, and selected-row offset. Re-anchor only after the existing behavior is recorded.

### D4 — Geometry and fallback
Podcast and Book use the same Wide predicate/rail framing and one-column rail policy as working TV/Movies. Preserve the established Narrow/Normal inline presentation and short-height fallback. Stable `ViewportAnchor` is the only breakpoint handoff; no App mirror or per-frame painted-state writeback.

### D5 — Mouse and one painter
Parent owns gesture subscription; child owns hit regions and resolves list points. Explicit child targets precede workspace targets. Each reachable breakpoint has one body painter, proven by source trace and a focused execution counter/assertion where practical.

### D6 — Visual-first verification
Correct visuals in a live run at Wide, Normal/Narrow, and short height; obtain explicit user confirmation. Only then modify/add rendered tests. Evidence must include representative metadata, group/selector/bucket states, images on/off, focus, active progress, framing, and target/viewport handoff.

### D7 — File limits and splits
Split `src/app/components/audiobookshelf_podcast.rs` into cohesive ownership modules before/with wiring if required, and inspect Music/Book/browser/render files for the 800-line limit. No unrelated cleanup.

## Dependencies and stacking

Requires accepted canonical foundation and Home/Feeds composition prerequisites, plus the foundation mouse seam. Targets the PR #606 feature branch as a distinct review/rollback slice. It supersedes #640 and must not touch #623/#634/#637.
