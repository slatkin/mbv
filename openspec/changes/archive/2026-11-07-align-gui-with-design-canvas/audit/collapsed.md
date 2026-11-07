# Gap list — Collapsed artboard (Collapsed.dc.html)

Spec: exact inline CSS values in the canvas file. UI: `crates/mbv-gui/ui/src/lib/components/`
(`CompactStrip.svelte`, `FoldedQueue.svelte`, `CollapsedRail.svelte`, `Panel.svelte`,
state choice in `lib/bridge/panel.ts`). Data: `GuiQueueSummary` / `GuiQueuePlayback`
(`ui/src/lib/bridge/types.ts`, Rust `crates/mbv-gui/src/bridge/frame/{queue,playback}.rs`).
Ordered by visual impact. The canvas sentence about container queries vs window size is ignored per brief.

## State selection (applies to all three states)

- G0. Canvas shows three states with labels `"COMPACT STRIP · container < 520px tall"`,
  `"EXPANDED-NO-QUEUE · queue folded away"`, `"RAIL · 72px"`, but specifies no precedence
  when triggers overlap. UI `panel.ts:panelState` picks `collapsed` > short container >
  `queueFolded` (`container.height > 0 && container.height < COMPACT_STRIP_MAX_HEIGHT`
  → strip; `COMPACT_STRIP_MAX_HEIGHT = 520`). So a short container hides an active queue
  fold (strip instead of folded line), and `z` hides shortness (rail instead of strip).
  Kind: WRONG-BEHAVIOR (undecided precedence; canvas is silent). Lowest impact.

## Expanded-no-queue folded line (FoldedQueue.svelte) — the whole line differs

- G1. Canvas: `"Up next · 37 · <span …>ends 12:50 +1d</span>"` at `"font-size: 13px; color: #d3c6aa"`,
  ends-clock in `"font-family: 'JetBrains Mono', monospace; color: #a7c080"`, bar
  `"height: 48px"`, `"padding: 0 8px 0 16px"`, `"justify-content: space-between"`.
  UI (`FoldedQueue.svelte:78-109`) paints `"Queue"` (11px semibold uppercase letterspaced
  muted) + `summary.remaining` number + optional `summary.finishClock`, at
  `style:height="var(--mbv-cell-row)"` with two-column margin/padding insets on the queue-box
  fill. Wording, order, sizes, height, insets all differ; the `"ends …"` prefix and the
  `"+1d"` day suffix have no painted counterpart. Kind: WRONG-VISUAL. (Data fields
  `remaining`, `finishClock` exist; whether `finishClock` carries the `"+1d"` suffix is
  unverified — see G2.)
- G2. Canvas ends-clock `"ends 12:50 +1d"` includes a day suffix. UI `types.ts:78`
  `finishClock: string | null` exists (`frame/queue.rs:316`), but no source in this recon
  confirms the `+1d` rollover format is produced. Kind: DATA-GAP (possible; needs format
  check in the core clock formatter, not a new field if the formatter already suffixes).
- G3. Canvas expand control: `aria-label="Expand queue"`, chevron-UP glyph
  `"<path d=\"m6 15 6-6 6 6\">"` (`stroke-width="2"`, `"width=\"16\" height=\"16\""`,
  `"color: #9da9a0"`). UI (`FoldedQueue.svelte:101-111`) uses `ChevronDown` (points down,
  wrong direction), lucide stroke/geometry instead of the canvas path. Kind: WRONG-VISUAL.

## Compact strip (CompactStrip.svelte)

- G4. Canvas play/pause: `"width: 44px; height: 44px"`, `"background: #a7c080"`,
  `"border-radius: 22px"` (filled circle), glyph `"color: #232a2e"`, pause bars
  `"<rect x=\"6\" y=\"4\" width=\"4\" height=\"16\" rx=\"1\">"`. UI
  (`CompactStrip.svelte:178-195`) paints a `rounded-full p-1` button on
  `var(--mbv-secondary)` with 16px lucide Play/Pause — small pill, theme fill, wrong
  glyph geometry and size. Highest single-element impact in the strip. Kind: WRONG-VISUAL.
- G5. Canvas progress: full-width bar under the row, `"height: 3px; background: #414b50"`,
  fill `"width: 4.5%; height: 3px; background: #a7c080"`, square edges, spanning the band.
  UI (`CompactStrip.svelte:163-175`) paints a 2px `rounded-full` bar on
  `var(--mbv-progressTrack)` nested INSIDE the text column (title/meta/transport row),
  so it spans only the middle column, not the band width. Height, radius, placement,
  track color all differ. Kind: WRONG-VISUAL.
- G6. Canvas band: `"background: #2d353b; border-top: 1px solid #3d484d;
  border-bottom: 1px solid #3d484d"`, row `"padding: 10px 8px 8px 12px; gap: 12px"`.
  UI strip root has background `var(--mbv-queuePlaybackBand)` with NO top/bottom borders,
  padding/gap from `var(--mbv-cell-row/col)` tokens. Kind: WRONG-VISUAL.
- G7. Canvas still: `"width: 96px; height: 54px"`, `"background: #343f44;
  border-radius: 4px"`, placeholder text `"[still]"` in `"font-size: 9px; color: #9da9a0"`.
  UI (`CompactStrip.svelte:140-154`) matches 96×54 but empty state is a flat
  `var(--mbv-textMuted)` block with no `"[still]"` label, `rounded-sm` instead of 4px,
  artwork `object-cover` with `alt=""` (canvas implies a still slot, not `<img>` cover).
  Kind: WRONG-VISUAL.
- G8. Canvas title `"font-size: 14px; font-weight: 600; color: #e9e1cc"`,
  meta `"font-size: 12px; color: #9da9a0"`, text `"Peep Show · next: New Year's Eve"`
  (lowercase `next:`). UI (`CompactStrip.svelte:54,156-161`) builds
  `` `Next: ${next}` `` (capital N — string mismatch) and paints through theme roles
  (`playbackTitleFg`/`playbackContextFg`, sizes via `truncate`/`text-xs` classes, not
  14px/12px). Kind: WRONG-VISUAL.
- G9. Canvas next control: `aria-label="Next episode"`, `"width: 44px; height: 44px"`,
  `"background: none … color: #d3c6aa"`, glyph `"width=\"18\" height=\"18\""`,
  `"<path d=\"m5 4 10 8-10 8zM19 5v14\">" stroke-width="1.8"`. UI
  (`CompactStrip.svelte:197-203`) has `aria-label="Next"` (wrong accessible name),
  18px lucide SkipForward (geometry close but not the canvas path). Kind: WRONG-VISUAL
  (name) + minor glyph delta.
- G10. Canvas pause glyph 18px (`"<svg width=\"18\" height=\"18\""`). UI uses `size={16}`
  lucide icons for both transport buttons. Kind: WRONG-VISUAL (minor).

## Rail (CollapsedRail.svelte)

- G11. Canvas expand: `aria-label="Expand panel"`, FOUR-arrow expand glyph
  `"<path d=\"M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7\">"` (`stroke-width="1.8"`,
  16px, `"color: #9da9a0"`). UI (`CollapsedRail.svelte:240-252`) uses `ChevronsRight`
  (») — a different metaphor (forward vs expand), lucide geometry. `aria-label` matches.
  Kind: WRONG-VISUAL.
- G12. Canvas remaining: `"−22m"` (U+2212 minus, compact minutes) in
  `"font-family: 'JetBrains Mono', monospace; font-size: 11px; color: #9da9a0"`.
  UI (`CollapsedRail.svelte:230-238`) paints `-{playback.remainingClock}` (ASCII hyphen +
  M:SS elapsed-format clock, e.g. `-22:15`-style) in `playbackMetaFg`, and hides it
  entirely when `runtimeTicks <= 0`. No compact-minutes value exists on the wire:
  `GuiQueuePlayback.remainingClock` (`types.ts:223`) is the scrubber-format clock.
  Kind: DATA-GAP — needs a compact remaining field (e.g. `remainingMinutes`/`remainingShort`)
  or a specified format; data exists in core (`positionTicks`/`runtimeTicks` already on
  `GuiPlaybackState`), so it is a projection/format gap, not a missing measurement.
- G13. Canvas rail box: `"width: 72px; background: #2d353b; border: 1px solid #3d484d;
  border-radius: 8px; … gap: 8px; padding: 12px 0"`. UI (`CollapsedRail.svelte:138-147`)
  matches 72px but paints `var(--mbv-queuePlaybackBand)` only when `playback.shown`,
  with NO border and NO 8px radius, padding/gap from cell tokens. Kind: WRONG-VISUAL.
- G14. Canvas ring: `"width=\"52\" height=\"52\""`, `r="24"`, `"stroke-width=\"3\""`,
  track `"stroke=\"#414b50\""`, arc `"stroke=\"#a7c080\" … stroke-dasharray=\"6.8 150\"
  transform=\"rotate(-90 26 26)\" stroke-linecap=\"round\""`, inner art
  `"left: 7px; top: 7px; width: 38px; height: 38px; border-radius: 19px;
  background: #343f44"`. UI (`CollapsedRail.svelte:41-43,150-197`) computes
  `RING_RADIUS_PX = (52-3)/2 = 24.5` (canvas: 24), art `52-6-4 = 42px` (canvas: 38px),
  track `var(--mbv-progressTrack)`, rotation via CSS `-rotate-90` class instead of the
  `transform` attribute, empty art `var(--mbv-textMuted)` instead of `#343f44`.
  Kind: WRONG-VISUAL (geometry + placeholder color; arc color via theme role is
  by-design indirection, unverified against `#a7c080`).
- G15. Canvas rail play/pause: `"background: none … color: #a7c080"`, 44×44, 18px glyph.
  UI (`CollapsedRail.svelte:198-215`) wraps it in `var(--mbv-secondary)` rounded-full
  (filled pill, like the strip) with 16px lucide icons — same filled-button mismatch as
  G4, while the canvas rail transport is unfilled. Kind: WRONG-VISUAL.
- G16. Canvas rail next: `"background: none … color: #d3c6aa"`, 18px canvas-path glyph.
  UI (`CollapsedRail.svelte:216-228`) same lucide substitution as G9 (label `"Next"`
  unmatched to any canvas rail next label — canvas says `aria-label="Next episode"`).
  Kind: WRONG-VISUAL (minor; label delta as in G9).

## Cross-cutting (all collapsed states, lower impact)

- G17. All hex colors (`#2d353b`, `#3d484d`, `#414b50`, `#a7c080`, `#d3c6aa`, `#9da9a0`,
  `#e9e1cc`, `#232a2e`, `#343f44`) arrive via `var(--mbv-*)` theme roles set from the frame
  theme (`theme.ts`), never as literals. Role→hex mapping unverified in this recon; any
  role whose themed value ≠ canvas hex is a latent WRONG-VISUAL per element above.
  Kind: WRONG-VISUAL (unverified, systemic).
- G18. All glyphs are lucide components (`Pause`, `Play`, `SkipForward`, `ChevronDown`,
  `ChevronsRight`) rather than the canvas inline SVG paths (rounded `rx="1"` pause bars,
  `stroke-linejoin="round"` next glyph). Stroke widths, caps, and optical sizes differ at
  every transport/expand control. Kind: WRONG-VISUAL (systemic, minor per instance).
- G19. Canvas type is `'IBM Plex Sans'` / `'JetBrains Mono'` via Google Fonts link; UI
  `app.css` sets the same families as theme fonts — assumed match, file-level font
  loading (`fonts.test.ts`, Tauri offline bundling) not checked here. Kind: none found
  (noted for completeness).
