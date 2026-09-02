## Context

This slice is the third family slice of the canonical-list migration. It repairs and owns BOTH the currently broken Audiobookshelf Books and Podcasts Wide/layout paths while bringing grouped Music into the shared composition. The standalone #640 Audiobookshelf podcast Wide implementation was reverted and its repair is absorbed here alongside the Books repair; the landed #640 Home podcast hero-on-left correction is a predecessor, not part of this scope. The Feeds Service, the Emby homevideos feed view, and the Emby podcast channel list remain separate surfaces; the Emby podcast channel list is already canonical via the #623 feed-picker dedup.

## Decisions

### D1 — Shared arrangements, no exception
Use the existing Hero-on-left and Inline arrangements and embedded `WideMediaList`/`InlineMediaBrowser`. Extend shared placement policies when needed; do not add a destination-sized list or bespoke exception. Wide Book selected-row replacement is removed: the bespoke `render_book_browser` path currently reuses Narrow's selected-row-replacement logic inside the Wide right rail, and that is the defect this slice corrects. The Book Wide contract is the persistent provider detail workspace on the left and ordinary fixed-height one-column rows on the right — no Wide selected-row replacement and no Inline hero in the right rail, matching grouped Music and the TV/Movies precedent. Audiobookshelf Podcast Wide follows the same precedent: provider episode workspace left, pills plus ordinary one-column rows right. The owned non-list repairs (Podcast Wide pill-row parity, Book Wide left-workspace framing) are captured in the `right-panel-arrangements` delta; no `ui-design-system` delta is needed because pane framing and rail presentation already live in `right-panel-arrangements`.

### D2 — Provider workspaces stay authoritative
Only Music album rows, Podcast show rows, and Book rows are canonicalized. Music grouping and tracks; Podcast show selection, episode list/filter, images; and Book details, chapters, audio files, and chapter seeking remain provider-owned. The canonical controls receive prepared rows and opaque targets and emit typed intents. Shell retains Service/effects/playback/persistence authority.

### D3 — Characterize Music before re-anchor
Before replacing Music handoff, record metadata-bearing grouped Music at Wide/Normal and breakpoint transitions, including selected target, cursor, scroll, and selected-row offset. The replacement must match that evidence unless an approved behavior correction says otherwise. Representative stateful tests then protect target/offset handoff and prove ordinary content pushes do not adopt stale shell cursor/scroll.

### D4 — Geometry and fallback
Music, Podcast, and Book Wide explicitly follow the TV/Movies canonical precedent: provider workspace left; parent-owned pills then ordinary fixed-height one-column canonical rows right; shared width/minimum-height predicate, pane framing, content spacing, and short-height fallback; no Inline hero and no selected-row replacement in the Wide right rail. Preserve the established Narrow/Normal inline presentation and short-height fallback. Stable `ViewportAnchor` is the only breakpoint handoff; no App mirror or per-frame painted-state writeback.

### D5 — One painter
Mouse is out of scope; `restore-mouse-support` (#638) owns it and lands last. This slice adds no mouse subscription, `MouseGestureState`, `HitRegions<Target>`, or parent-to-child point delegation, and existing bespoke `*HitRegion` paths stay wired and untouched. Each reachable breakpoint has one body painter, proven by source trace and a focused execution counter/assertion where practical.

### D6 — Continuous verification and acceptance
Implementation, representative stateful and rendered tests, automated gates, review, and acceptance form one uninterrupted slice. There is no pre-test visual-approval checkpoint. Live Wide, Normal/Narrow, and short-height review remains required; defects found there are fixed as bugs before rerunning affected tests and gates. Evidence includes metadata, group/selector/bucket states, images on/off, focus, active progress, framing, and target/viewport handoff.

### D7 — File limits and splits
Split `src/app/components/audiobookshelf_podcast.rs` into cohesive ownership modules before/with wiring if required, and inspect Music/Book/browser/render files for the 800-line limit. No unrelated cleanup.

## Dependencies and stacking

Requires two predecessors: the canonical media-list foundation (`introduce-canonical-media-list-foundation`) — an accepted prerequisite slice that must merge into `feat/migrate-tui-to-tuirealm` before this slice's implementation — with its `WideMediaList`/`InlineMediaBrowser`/`ViewportAnchor` vocabulary, and the landed #640 Home podcast hero-on-left correction. No base SHA is pinned in this plan; record the feature-branch baseline SHA when implementation is issued. `migrate-home-feeds-to-canonical-lists` is a sibling slice, not a functional dependency. Targets the PR #606 feature branch as a distinct review/rollback slice. The standalone #640 Audiobookshelf podcast Wide implementation was reverted and is absorbed here; this slice must not touch the Emby podcast channel list, the Emby homevideos feed view, or #623/#634/#637.
