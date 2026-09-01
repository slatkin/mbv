## Why

Home and the Feeds Service/tab still compose destination-sized list mechanics instead of the reviewed canonical controls. This slice applies the foundation to those two distinct destinations without conflating the Feeds Service with an Emby homevideos feed view.

## What Changes

- Compose `WideMediaList` and `InlineMediaBrowser` for Home sections and the Feeds Service/tab.
- Preserve Home section identity (`pref_key`/`restore_section`), the single active-section cursor/scroll, images, and workspace effects.
- Project Feeds date/age labels as `Heading` rows and separators as `Spacer` rows as canonical-list content, while the subscription/group selector pills and the watched selector stay parent-owned chrome outside the canonical control.
- Retain the accepted #623 Wide one-column/framing baseline and group selection.
- Make canonical list rows the source of truth for the deferred #623 two-space row-indent follow-up.
- Keep non-hero two-column policies and the Emby homevideos feed view (#634/#637) out of scope as boundary notes. The Music/Audiobookshelf canonical slice is out of scope; standalone #640 is superseded.

This stacks on PR #606's feature branch and depends on the landed canonical-list foundation and the accepted #623 `restore-feeds-service-wide-list` prerequisite (umbrella task 1.3a).

## Impact

UI components, shell composition, focused characterization/render tests, and planning evidence only. No Service, provider, protocol, daemon, persistence, or dependency changes.
