## Why

The canonical media-list foundation leaves grouped Music and Audiobookshelf destinations. Books and Podcasts currently have broken Wide composition/layout (including duplicated selected-book detail). This slice makes those destinations compose the shared controls instead of preserving a bespoke exception, while keeping provider workspaces and playback authority intact.

## What Changes

- Canonicalize only Music album rows, Audiobookshelf Podcast show rows, and Audiobookshelf Book rows through `WideMediaList`/`InlineMediaBrowser` as applicable. Episodes, chapters, tracks, and provider workspaces remain parent-owned.
- Compose the three parent row surfaces with the correct Wide, Normal, and short-height fallback arrangements, characterizing Music re-anchor before replacement.
- Remove Wide Book selected-row replacement in favor of the canonical presentation: the Book Wide contract is the persistent provider detail workspace on the LEFT and ordinary fixed-height one-column rows on the RIGHT, with no Wide selected-row replacement and no Inline hero in the right rail. Preserve show/book provider workspaces, episode/chapter/track authority, images, selectors, surname buckets, typed intents, and shell effects.
- Repair the owned Audiobookshelf Podcast and Book non-list arrangement defects (Podcast Wide right-rail pill row missing versus Narrow; Book Wide left-workspace framing) through the shared hero-on-left arrangement, not a bespoke surface. These provider-arrangement fixes are covered by the `right-panel-arrangements` delta and are distinct from the canonical list control itself; no `ui-design-system` delta is needed.
- Preserve the distinction between the Feeds Service, the Emby homevideos feed view, and the Emby podcast channel list; the Emby podcast channel list already composes canonically through the #623 feed-picker dedup and is out of scope, as are #623/#634/#637.
- Split `audiobookshelf_podcast.rs` and any near-limit files ownership-preservingly before or with wiring; no new dependency or runtime/provider/protocol change.

## Sequencing

This is a distinct PR stacked on `feat/migrate-tui-to-tuirealm` / PR #606's feature branch. It depends on two landed predecessors: the canonical media-list foundation (`introduce-canonical-media-list-foundation`, merged on `feat/migrate-tui-to-tuirealm`) for `WideMediaList`, `InlineMediaBrowser`, `ViewportAnchor`, and the parent/child mouse contract; and the landed #640 Home podcast hero-on-left correction (podcast hero artwork-on-top, shared hero placement). No base SHA is pinned in this plan; record the feature-branch baseline SHA when implementation is issued. `migrate-home-feeds-to-canonical-lists` is a sibling slice, not a functional dependency. The standalone #640 Audiobookshelf podcast Wide implementation was reverted; that repair and the Audiobookshelf Books repair are absorbed and owned here, so this slice owns BOTH currently-broken Audiobookshelf Podcasts and Books.

Visual correction and explicit user live confirmation precede any UI test changes. Music pre-replacement characterization is limited to source/state inspection and live visual evidence. UI tests may be changed or added only after explicit user live visual approval; tests characterize the confirmed result and do not drive appearance.
