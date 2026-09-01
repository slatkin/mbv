## Context

`render_audiobookshelf_podcast_content` already calls `shared_hero_presentation`, but its Wide branch paints the hero and ordinary browser directly into the returned panes and does not use the shared right-pane pill/framing policy. The shell also retains legacy right-area projection for overlays. The correction is limited to placement and geometry; provider content remains the source of truth.

## Goals / Non-Goals

**Goals:** make Audiobookshelf podcast Wide conform to the sole Hero-on-left arrangement; keep one fixed-row right rail; retain Wide pills, semantic framing, provider episode workspace, images, and typed interaction geometry; preserve Narrow.

**Non-Goals:** canonical media lists, Feeds Service or Emby homevideos feed views, provider fetch/playback semantics, queue/protocol changes, keyboard/mouse routing, or PR #606 merge sequencing.

## Decisions

### D1 — Use the shared arrangement at the shell/component seam
Keep the shell's existing Wide predicate as the source of the content-area decision, but ensure the component receives the same Wide geometry and uses `hero_on_left_right_pane`/`pill_bar_areas` and the shared rail border painter rather than reconstructing a detached right panel. The arrangement owns breakpoints and rect splitting; the podcast component owns content and provider-native targets.

### D2 — One fixed-row rail
Wide show rows use one column regardless of `library_column_count`. The right rail's pill row and list panel are laid out with the shared helpers; selected show detail remains the left hero workspace. Episode pills/table remain in the existing provider workspace and are not replaced by generic list content.

### D3 — Preserve Narrow and characterize the boundary first
Before replacement, add/retain TestBackend captures and geometry assertions for a metadata/state-bearing fixture at widths 81 and 82 plus a larger Wide size. Record the current threshold transition and re-anchor/scroll behavior, including the existing short-height fallback. Update assertions only for the intended shared placement; use the Narrow capture as a regression guard.

### D4 — Keep the shell projection contract
Update `AudiobookshelfPodcastGeometry` and shell projection only as needed to describe the shared rects. Do not move App, Service, credentials, fetching, or effectful image work into the render component. If the render component exceeds 800 lines after the smallest implementation, extract a cohesive provider-render helper/test module in the same change.

### D5 — Verification
Focused TestBackend tests MUST inspect rendered cells/rects, not only construct models: one-column x geometry, pill and rail framing, selected/active/played marker alignment, hero placement, image-enabled and disabled paths, 81/82 threshold, short-height inline fallback, and stable Narrow re-anchor. Run the existing Audiobookshelf podcast tests and source-size gate.

## Risks / Trade-offs

- Shared helpers may expose assumptions about right-pane width → assert exact threshold and rail rects before changing callers.
- Shell overlay anchoring may depend on legacy right-area fields → preserve compatible geometry projections and test selected/episode targets.
- Provider file size may exceed the cap → split only the cohesive render section required by the implementation, not unrelated cleanup.
