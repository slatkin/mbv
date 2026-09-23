# Proposal

## Why

The F3 Sessions sidebar already presents three-line targets, but maintains its own index-based cursor, viewport, and row-hit logic instead of using the shared list mechanics. A reusable three-line flat presentation lets F3 adopt stable-target list behavior without exporting session-specific card design to other destinations.

## What Changes

- Add a reusable three-line flat list presentation over the existing shared row-flow/selection/viewport/paint-geometry seam. Its generic concerns are line placement, list-wide zebra treatment, the standard selected-row bar, and adjustable spacing between entries.
- Move F3's row selection, scrolling, and hit resolution into that embedded list; keep the mounted Sessions sidebar responsible for shell-owned content projection, gesture recognition, chrome, and target-specific messages.
- Keep three content lines and one blank separator row per entry initially; the separator remains unstriped and outside the selected bar. Preserve F3's Emby and Cast information and target ordering; Cast may leave line three blank.
- Remove F3's accent cursor rail and connected-target fill. Show the connected/attached `✚` badge in aqua; the filled selected-row bar means cursor selection only.
- Send stable kind-qualified target identity on activation instead of a display index, so independent Emby/Cast refreshes cannot redirect an outstanding selection.

## Capabilities

### New Capabilities

- `sessions-sidebar`: F3's three-line target presentation, selection, badge, and activation behavior.

### Modified Capabilities

- `shared-list-components`: Add the reusable three-line flat presentation over the existing shared list seam, without creating a second list state machine.

## Impact

`src/app/components/media_list/`, `src/app/components/list/`, `src/app/components/sessions.rs`, `src/app/render/components/sessions.rs` and shared list painting, typed Sessions requests and their shell dispatch, and focused render/component/tick tests. No new dependency, Service API, discovery behavior, or Player authority change.
