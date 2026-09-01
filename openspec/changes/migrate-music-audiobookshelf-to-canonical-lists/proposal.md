## Why

The canonical media-list foundation leaves grouped Music and Audiobookshelf destinations. Books and Podcasts currently have broken Wide composition/layout (including duplicated selected-book detail). This slice makes those destinations compose the shared controls instead of preserving a bespoke exception, while keeping provider workspaces and playback authority intact.

## What Changes

- Canonicalize only Music album rows, Audiobookshelf Podcast show rows, and Audiobookshelf Book rows through `WideMediaList`/`InlineMediaBrowser` as applicable. Episodes, chapters, tracks, and provider workspaces remain parent-owned.
- Compose the three parent row surfaces with the correct Wide, Normal, and short-height fallback arrangements, characterizing Music re-anchor before replacement.
- Remove Wide Book selected-row replacement in favor of the canonical presentation; preserve show/book provider workspaces, episode/chapter/track authority, images, selectors, surname buckets, typed intents, and shell effects.
- Repair non-list layout defects required for composition, including the Podcast Wide rail and Book framing, without creating a bespoke surface.
- Preserve the distinction between Feeds Service and Emby homevideos feed view; this slice does not implement #623/#634/#637.
- Split `audiobookshelf_podcast.rs` and any near-limit files ownership-preservingly before or with wiring; no new dependency or runtime/provider/protocol change.

## Sequencing

This is a distinct PR stacked on `feat/migrate-tui-to-tuirealm` / PR #606's feature branch, after the accepted canonical foundation. The accepted canonical-foundation merge tip is intentionally not pinned in this plan; record the accepted canonical-foundation merge tip when implementation is issued. It supersedes standalone #640; its repairs are absorbed here. It depends on the foundation's `WideMediaList`, `InlineMediaBrowser`, `ViewportAnchor`, and parent/child mouse contract.

Visual correction and explicit user live confirmation precede any UI test changes. Music pre-replacement characterization is limited to source/state inspection and live visual evidence. UI tests may be changed or added only after explicit user live visual approval; tests characterize the confirmed result and do not drive appearance.
