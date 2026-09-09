## Why

The Feeds Service/tab still moves both persistent canonical controls in lockstep and rebuilds parent-owned row maps for pointer resolution instead of relying on the active control's retained current-frame geometry. Its remaining presentation deviations also prevent umbrella row 6.1 from reaching the Emby-reference convergence required by D8.

## What Changes

- Make only the active `WideMediaList` or `InlineMediaBrowser` move, select, and reset; retain the existing one-`ViewportAnchor` breakpoint handoff as the sole responsive transfer mechanism.
- Paint the active control through the retained-result seam and resolve clicks and wheel claims from its retained current-frame geometry, deleting Feeds' parent row-map rebuild and painted-offset copy.
- Keep blank and heading rows unclaimed, while preserving the selected-detail hero-cover hit rule.
- Converge Feeds presentation on the shared Emby reference by painting watched-filter chrome through the shared pill-bar primitive, projecting resume progress through canonical row slots, retaining and documenting the intentional hero placeholder image slot, and aligning pill-label truncation if it differs from the Emby policy.
- Add focused, rendered, and Wide plus Normal/Narrow shell-tick integration coverage for active-only ownership, retained hits, responsive handoff, one-painter ownership, and presentation convergence.
- Preserve component-local group and watched-filter selection without adding restart persistence, a shell mirror, or a new request variant.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This ownership and presentation repair conforms to the current canonical-media-list and right-panel-arrangement requirements without changing observable requirements.

## Impact

- Affected area: Feeds Interactive Component, its Wide/Inline Render Component seam and Feeds content projection, plus focused, buffer, and shell-tick tests.
- No API, dependency, persistence-format, specification, shared-helper, router, focus, subscription, or unrelated-destination change.
