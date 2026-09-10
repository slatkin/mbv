## Context

See proposal.md for motivation. Grouped Music currently keeps live album position in both shell projection data and `MusicWorkspaceComponent`, then reseeds controls and writes painter-derived scroll back during rendering. The existing canonical-list contract instead makes persistent controls the live authority, with the shell providing content and explicit re-anchor requests.

## Goals / Non-Goals

**Goals:**

- Give the active persistent album control exclusive live cursor/scroll ownership.
- Preserve a stable selected album over ordinary content refresh and hand off one `ViewportAnchor` only when the presentation changes.
- Make album pointer resolution use the painting control's retained current-frame geometry.
- Retain Music authority for provider workspace content and outward semantic intents.

**Non-Goals:**

- Change Queue, any other campaign destination, non-grouped Music, or non-hero catalog grids.
- Redesign grouped-Music visual content, search, group pills, tracks, images, effects, persistence, or keyboard policy.
- Add a universal enforcement rule or reconcile unrelated documentation.

## Decisions

### D1. Controls own live album position; the shell retains only resting/re-anchor state

`WideMediaList<String>` and `InlineMediaBrowser<String>` remain persistent component fields. Content pushes call their stable-target-preserving content APIs; shell projection does not read a component cursor/scroll back to seed a later render. A shell request explicitly re-anchors the active presentation from resting state.

Alternative: retain shell cursor/scroll as a synchronized live mirror. Rejected because it preserves two authorities and permits render-time adoption.

### D2. Rendering is geometry and paint only

The Wide and Inline render paths configure geometry and paint policy then invoke the mounted control. They do not set content, selection, or scroll, and they do not publish painter-derived album position. Parent compatibility row maps and point resolution are removed; album hits resolve through retained control geometry. Parent pill geometry remains parent-owned chrome.

Alternative: keep parent maps for compatibility. Rejected because a second geometry authority can drift from the current painted frame.

### D3. Responsive transfer uses the shared anchor protocol

A presentation transition captures one `ViewportAnchor<String>` from the former active control and applies it to the newly active persistent control. The offset is measured in display rows, including structural heading and spacer rows. Ordinary refresh does not use this path.

Alternative: reconstruct scroll from grouped-row arithmetic in the parent. Rejected because that creates a destination-specific second viewport authority.

### D4. Music workspace authority remains intact

Track workspace state and rows, search behavior, group-pill state, images, effects, persistence, and typed request translation remain owned by Music. This repair changes only canonical grouped-album list mechanics.

Alternative: migrate all Music state while touching the component. Rejected as unrelated scope expansion.

## Risks / Trade-offs

- [Anchor offset differs across grouped structural rows] → exercise transitions with headings and spacers in media-list and Music component tests.
- [Removing compatibility maps leaves a stale hit path] → cover mounted tick/subscription pointer handling at Wide and Normal/Narrow breakpoints.
- [Refresh regresses selection] → characterize stable-target preservation, clamp, and explicit re-anchor separately.

## Migration Plan

1. Characterize current visible output and control behavior at supported breakpoints.
2. Move grouped-album live position and handoff to the persistent controls; remove duplicate projection, render reseeding, writeback, and compatibility geometry.
3. Verify focused component, shell-tick pointer integration, formatter, and required project gates.
4. If needed, revert the bounded source change; no persisted data migration is involved.
