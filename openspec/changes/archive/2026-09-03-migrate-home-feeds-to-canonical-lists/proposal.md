## Why

Home and the Feeds Service/tab still compose destination-sized list mechanics instead of the reviewed canonical controls. The unaccepted Home/Feeds wiring from `173bdba1`/`400c0b59` is in-tree at the slice baseline in the wrong shape (legacy underpaint, render-offset write-back, parent cursor/scroll mirrors). This slice reworks that wiring to the canonical contract for those two distinct destinations without conflating the Feeds Service with an Emby homevideos feed view.

## What Changes

- Rework the in-tree Home/Feeds wiring (`173bdba1`/`400c0b59`) to compose `WideMediaList` and `InlineMediaBrowser` for Home sections and the Feeds Service/tab: remove the legacy underpaint, the render-offset write-back, and the parent cursor/scroll mirrors.
- Preserve Home section identity (`pref_key`/`restore_section`), the single active-section cursor/scroll, images, and workspace effects.
- Project Feeds date/age labels as `Heading` rows and separators as `Spacer` rows as canonical-list content, while the subscription/group selector pills and the watched selector stay parent-owned chrome outside the canonical control.
- Retain the accepted `restore-feeds-service-wide-list` (umbrella task 1.3a) Wide one-column/framing baseline and group selection.
- Make canonical list rows the source of truth for the deferred two-space row-indent follow-up from `restore-feeds-service-wide-list` (umbrella task 1.3a).
- Keep non-hero two-column policies and the Emby homevideos feed view (#634/#637) out of scope as boundary notes. The Music/Audiobookshelf canonical slice is out of scope; standalone #640 is superseded.
- Centralize the one-row status-bar reserve in the shared hero-on-left arrangement primitive (surfaced during 4.6 live review, when Feeds Wide panels were found touching the status bar): the shared primitive returns panes that already exclude the status row, the per-tab reserves collapse, the Feeds per-tab stopgap (`89ef789d`) is removed, and the invariant is recorded in the arrangement spec delta (`D7`). Consolidation, not a visual change.

This stacks on PR #606's feature branch on top of the merged and archived canonical-list foundation (merge `a72f60f9`, archive `9122cc1b`) and depends on the accepted `restore-feeds-service-wide-list` prerequisite (umbrella task 1.3a). The slice baseline is the merge-containing HEAD, which includes the unaccepted Home/Feeds wiring — that wiring is acknowledged in-tree material this slice repairs, not a clean baseline and not a silent revert. The stashed `home-wip-paused-during-canonical-merge` work is retained as unaccepted evidence alongside it.

## Impact

UI components, shell composition, focused characterization/render tests, and planning evidence only. No Service, provider, protocol, daemon, persistence, or dependency changes.
