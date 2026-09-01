## Why

Home and the Feeds Service/tab still compose destination-sized list mechanics instead of the reviewed canonical controls. This slice applies the foundation to those two distinct destinations without conflating the Feeds Service with an Emby homevideos feed view.

## What Changes

- Compose `WideMediaList` and `InlineMediaBrowser` for Home sections and the Feeds Service/tab.
- Preserve Home section identity (`pref_key`/`restore_section`), per-section cursor/scroll, images, and workspace effects.
- Project Feeds group labels as `Heading` and separators as `Spacer`, retaining watched filtering, group selection, and the accepted #623 Wide one-column/framing baseline.
- Make canonical list rows the source of truth for the deferred #623 two-space row-indent follow-up.
- Keep non-hero two-column policies, Emby homevideos feed-view fixes (#634/#637), #640, and Audiobookshelf out of scope.

This stacks on PR #606's feature branch and depends on accepted #634/#637, the canonical-list foundation, and the Feeds Wide prerequisite.

## Impact

UI components, shell composition, focused characterization/render tests, and planning evidence only. No Service, provider, protocol, daemon, persistence, or dependency changes.
