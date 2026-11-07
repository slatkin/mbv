# Gap list — Main.dc.html (expanded queue panel) vs current Svelte UI

Scope: expanded 400 px queue panel only (header, artwork, title, scrubber, transport, track chips, queue header, rows, footer, sample-state script). Window-resize/container-query sentence ignored per brief.

## Header

1. Cast button content — canvas: `Living room TV` green (`#83c092`) text with a cast SVG before it and a chevron-down (`m6 9 6 6 6-6`) after, `aria-label="Cast to device"`. Current: a Lucide `Cast` icon plus host text in primary colour, no chevron, no green text (`QueuePlaybackPanel.svelte`, target span). Kind: WRONG-VISUAL (missing chevron affordance for the chooser the press opens).
2. State label — canvas: `● PLAYING`, `'JetBrains Mono'`, 11 px, `letter-spacing: 0.08em`, `#a7c080`, padded `0 8px`. Current: bare bold word (`statusWord()` → `PLAYING`/`PAUSED`/`IDLE`), no leading `●`, no letterspacing (`QueuePlaybackPanel.svelte`, `statusWord`/`statusColor`). Kind: WRONG-VISUAL.
3. Collapse button icon — canvas: four-arrow expand glyph (`M4 14h6v6M20 10h-6V4M14 10l7-7M3 21l7-7`), `#9da9a0`, 44×44 px hit area, `aria-label="Collapse panel"`. Current: `ChevronUp` 16 px (`QueuePlaybackPanel.svelte`, `collapseEl`). Kind: WRONG-VISUAL.
4. Extra control — current header adds a second `queueFold` button (`ListCollapse`/`ChevronDown`, the `q` toggle) that Main.dc.html does not paint (`QueuePlaybackPanel.svelte`, `queueFoldEl`). Kind: WRONG-VISUAL (extra vs canvas; the fold state itself belongs to the folded artboard).
5. Header sizing — canvas: 44 px row, `border-bottom: 1px solid #3d484d`, 44 px button hit areas. Current: everything in cell units (`var(--mbv-cell-row)` height, `p-1` buttons), no bottom border (`QueuePlaybackPanel.svelte`, header div). Kind: WRONG-VISUAL.

## Artwork

6. Still slot — canvas: fixed `height: 225px` box, `#343f44`, centred caption `[Episode still · Emby Primary / Thumb]` (mono 12 px `#9da9a0`). Current: no fixed box; `Image` paints the bitmap at `max-height: calc(var(--mbv-cell-row) * 24)`, `Loading` an aspect-square muted fill, otherwise nothing (`QueuePlaybackPanel.svelte`, `art` block). Kind: MISSING (fixed still slot + source caption visual).
7. Episode badge — canvas: `S4 · E1` (middle dot), mono 11 px, `#232a2e` chip on `#d3c6aa`, `position: absolute; left: 12px; bottom: 10px`. Current: core formats `"S2 E5"` with a space (`queue_playback_projection.rs:431-443`), painted top-right in popover colours (`QueuePlaybackPanel.svelte`, `episodeBadge`). Kind: WRONG-VISUAL (position, chip colours, and `·` vs space separator).

## Title

8. Title block — canvas: `Nether Zone`, 18 px, weight 600, `#e9e1cc`; below it `<a href="#">` `Peep Show`, 13 px, `#83c092` (green link). Current: title truncated semibold in `playbackTitleFg`, parent line muted span, not a link and no press region (`QueuePlaybackPanel.svelte`, title block). Kind: WRONG-VISUAL (size/link styling) + MISSING (parent line carries no region/command; canvas paints it as a link).

## Scrubber

9. Time labels — canvas: left `1:03` (`#a7c080`), right single span `−22:14 · ends {{currentEnds}}` (`#9da9a0`), U+2212 minus, mono 12 px tabular. Current: elapsed in `playbackMetaFg`, and the right side split into two spans (`-{remainingClock}` + bare `{finishClock}`) with a hyphen-minus and no `ends` word (`QueuePlaybackPanel.svelte`, scrubber row). Kind: WRONG-VISUAL.
10. Bar segments — canvas, left to right: 6 px track `#414b50`; intro block `width: 6px` `#e69875` (`title="Intro"`); played fill `width: 16px` `#a7c080`; chapter ticks `width: 2px; height: 14px` `#7a8478` at 116/232 px; credits block `width: 4px` `#e69875` (`title="Credits"`); thumb 16 px circle `#e9e1cc` with `box-shadow: 0 0 0 3px #2d353b`. Current: 4 px track, marks at 40 % opacity under the played fill, played fill and thumb both `var(--mbv-accent)`, 1 px ticks in `textMuted`, 12 px thumb with background-colour ring (`QueuePlaybackPanel.svelte`, seekbar). Kind: WRONG-VISUAL.
11. Hover position line — canvas: `left: 233px; top: 14px; width: 1px; height: 22px; background: #e9e1cc` full-height line at the pointer. Current paints no hover line (`QueuePlaybackPanel.svelte`, `hover` block). Kind: MISSING.
12. Hover preview — canvas: tile `128×72` with `border: 1px solid #56635f`, placeholder text `[trickplay]` (mono 10 px), label chip `12:31 · Ch. 2` (mono 11 px, `#232a2e` on `#e9e1cc`). Current: `aspect-video w-32` box with a `Film` icon placeholder, label is `{timeLabel}` + full chapter name (`QueuePlaybackPanel.svelte`, preview block). Kind: WRONG-VISUAL (minor; position above the bar matches).

## Transport

13. Pause button — canvas: `width: 56px; height: 56px; border-radius: 28px; background: #a7c080`, dark pause glyph (`aria-label="Pause"`). Current: `p-1` rounded-full in `var(--mbv-secondary)` with an accent/muted glyph (`QueuePlaybackPanel.svelte`, `playPauseEl`). Kind: WRONG-VISUAL.
14. Seek buttons — canvas: text buttons `−10` / `+30` (mono 11 px, `#d3c6aa`, 44 px areas, `aria-label="Back 10 seconds"` / `"Forward 30 seconds"`). Current: `RotateCcw`/`RotateCw` icons with overlaid 8 px numerals (`QueuePlaybackPanel.svelte`, `seekBackEl`/`seekForwardEl`). Kind: WRONG-VISUAL.

## Track chips

15. Chip style — canvas: three `height: 32px` buttons (`FHD · direct`, `EN 2.0`, `CC EN`), mono 12 px `#e69875` on `#343f44` with `#475258` border and a chevron-down each. Current: centred pills with leading Lucide icons (`MonitorPlay`/`AudioLines`/`Captions`), no chevrons; the video chip is label-only by D4a ruling (no popover) while canvas gives it a chevron (`TrackChips.svelte`). Kind: WRONG-VISUAL (shape, colour, chevrons; video-chip interactivity diverges by cited D4a decision).

## Queue header

16. Heading text — canvas: `Up next · {{remainingCount}}` (13 px, weight 600, `#e9e1cc`) over `queue ends {{queueEnds}}` (mono 11 px, `#9da9a0`). Current: `Queue` (uppercase muted tracking-widest) + bare `{summary.remaining}` number + bare `{finishClock}` (`QueueHeader.svelte`). Kind: WRONG-VISUAL (both lines' wording/structure).
17. Length/Ends-at segment — canvas: `background: #232a2e; border-radius: 6px; padding: 2px`, active segment `background: #475258; color: #e9e1cc` 28 px tall, 12 px text. Current: bordered `rounded-full` switch, active pill `var(--mbv-secondary)` (`QueueHeader.svelte`, `MODES`). Kind: WRONG-VISUAL.
18. Filter control — canvas: icon-only magnifier button (`circle cx=11 cy=11 r=7` + `m20 20-3.5-3.5`), 44×44 px, `aria-label="Filter queue"`; no text painted. Current: text pill painting `{summary.filter || 'Filter'}` plus block caret while `filterActive` (`QueueHeader.svelte`, `data-filter`). Kind: WRONG-VISUAL (icon button vs text-entry pill; behaviour — live text + caret — has no canvas counterpart on this artboard).

## Rows

19. Row numbers — canvas: every row leads with `01`…`38` (mono 11 px, `width: 18px`, `#7f897d`, `#232a2e` on the current row). Current paints no ordinal (`QueuePanel.svelte`, `rowCells`). Kind: MISSING + DATA-GAP — needs a 1-based queue-position field on `GuiQueueRow` (e.g. `queue.rs: GuiQueueRow::Item`); the data exists in the core (queue order in `QueueSurface::rows()` / `queue_summary_projection.rs` slot order).
20. Row geometry/type — canvas: `min-height: 30px`, single 13.5 px title line, `gap: 10px; padding: 0 8px`, rows are `<button>`s. Current: 30 px rows match (`ROW_HEIGHT`), but cells paint a two-line primary/secondary pair plus a 6-column date gutter the canvas row has no room for (`QueuePanel.svelte`, `rowCells`). Kind: WRONG-VISUAL (canvas row carries title + progress + right value only).
21. Current row — canvas: filled `#a7c080`, all text `#232a2e`, no leading icon; progress track `#7d9460` with `#232a2e` fill. Current: accent fill with `textOnAccent`, plus a leading Lucide `Play` marker the canvas row lacks (`QueuePanel.svelte`, `rowBackground`/`tone`/`rowCells`). Kind: WRONG-VISUAL (extra marker; track/fill colours).
22. Watched row — canvas: `#9da9a0` with `text-decoration: line-through; text-decoration-color: #56635f`. Current: muted + `line-through` in the text colour (`QueuePanel.svelte`, `tone`). Kind: WRONG-VISUAL (minor; strike colour).
23. In-progress row — canvas: 2 px track `#414b50`, fill `#e69875`, `width: max(pct,3)%` (sample `p,5` → 5 %). Current: fill `var(--mbv-accent)` on an inset padded bar (`QueuePanel.svelte`, `rowCells` progress). Kind: WRONG-VISUAL (minor; fill colour + bar insets).
24. Selected (non-current) row — canvas: `picked` row `background: #3d484d; color: #d3c6aa`. Current: barred rows paint the selection bar `var(--mbv-duration)` with selection roles (`QueuePanel.svelte`, `bars()`/`tone()`). Pick maps to the core cursor (press → hit map → core), so the state exists; the paint differs. Kind: WRONG-VISUAL.
25. Right-column colour rule — canvas: Ends-at values `#a7c080`, Length values `#9da9a0`, current row `#232a2e`. Current paints both modes in `var(--mbv-duration)` (muted meta) for plain rows (`QueuePanel.svelte`, `tone().right`). Kind: WRONG-VISUAL (minor; mode-dependent colour lost).
26. Hover chrome — current paints a `GripVertical` grip and an `Ellipsis` actions button on `queue.hover`, each a hit region (`QueuePanel.svelte`, hover block). Main.dc.html paints no row-hover chrome. Kind: WRONG-VISUAL (extra vs canvas; behaviour path itself is core-owned per design D4).

## Footer

27. Left line — canvas: `Peep Show · Series 1–6` (`#7fbbb3`, mono 12 px): library context (series + season range), not a playlist name. Current: playlist pill (Lucide `list-music` icon + `footer.playlist.text`, the loaded playlist's name or nothing) (`QueueFooter.svelte`; `queue_footer_projection.rs: playlist_footer_pill`). Kind: DATA-GAP — needs a queue-context line field on `GuiQueueFooter` (series + season range); the series name exists in core item facts (`playback_title_parts`), a season-range string does not.
28. Right marker — canvas: `woims ●` (`#dbbc7f`): label then dot, no icon. Current: `Cable` icon + label + live dot (`QueueFooter.svelte`, owner group; dot already after the label per D4a). Kind: WRONG-VISUAL (minor; extra leading icon).
29. Footer centre/extras — canvas centre is empty space (`justify-content: space-between`); no toast, prefix badge, autosave, or dirty pills on this artboard. Current centre-fills the live toast and paints `PREFIX`/autosave/dirty states (`QueueFooter.svelte`). Kind: WRONG-VISUAL (extras vs canvas; the inline-toast form is the permitted choice per the component's spec comment).

## Sample-state logic (script vs core projections)

30. Clock format — canvas `clock()` renders zero-padded 24 h `HH:MM` (`String(...).padStart(2,'0')`, sample `21:45`). Core `fmt_wall_clock` renders 12 h `"11:42 AM"` (`crates/mbv-ui-model/src/ui_util.rs:216-228`), carried verbatim into `finishClock`, `row_finish_clocks`, and the scrubber's `finish_clock`. Kind: WRONG-BEHAVIOR (all three clocks misformat against the canvas everywhere they paint).
31. Day overflow — canvas appends `' +1d'` when the end passes midnight (`queueEnds: clock(t) + (t >= 86400 ? ' +1d' : '')`). No DTO field carries it; `finish_clock`/`row_finish_clocks` are preformatted strings with the overflow dropped. Kind: DATA-GAP — needs a day-overflow fact (or unformatted epoch); computable in the core from the end epoch vs local midnight, not present on the wire.
32. Remaining count — canvas `remainingCount++` for every non-current row, including watched rows *before* the current one. Core `remaining` counts only slots after the active index (`queue_summary_projection.rs:118`); identical while current is first (sample: 37), divergent otherwise. Kind: WRONG-BEHAVIOR (canvas sample rule vs core projection; flag to reconcile, not to reimplement in the front end).
33. Agreement (no gap) — canvas `currentEnds` (now + active remainder) matches core `playback_finish_epoch` + `finish_clock`; canvas cumulative per-row ends match `finish_chain`/`row_finish_clocks` (`queue_summary_projection.rs:142-183`); canvas partial-progress remainder (`p` → `dur·(1−pct/100)`) matches remaining-ticks anchoring. Modulo items 30–32, the arithmetic agrees.
