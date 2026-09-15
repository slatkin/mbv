## Context

See proposal.md (Why) and `specs/canonical-media-lists/spec.md` (requirements). Current state on the 719 branch: `WideMediaListPaintPolicy` carries an optional `ZebraStripe { focused, unfocused }` plus a `with_selected_gutter` opt-in flag; `render_wide_media_list_with_zebra` threads both into `media_list_row`, which renders the gutter-selected style (bold focus-accent title, transparent background, no marker) only when the flag is set. Only the queue call site sets either. The Inline adapter calls the same `media_list_row` with `None`/`false` hardcoded. Both library Wide sites (Browser pane `Wide`, workspace `WideWorkspace`) funnel through `PanelList::set_paint_policy` in `src/app/components/library_panel/panel_list.rs`.

Precondition: 719 merges first (this change stacks on it), with its delta spec corrected for the as-merged parity (stripes on 1st/3rd visible items, not the spec'd 2nd/4th) and the gutter-selected style.

## Goals / Non-Goals

**Goals:**
- Library Wide zebra in the specified colour pair through the existing policy channel.
- Gutter-selected as the only Wide selected-row path, with the opt-in deleted rather than extended.

**Non-Goals:**
- No change to the Inline adapter or its selected-background treatment (Narrow scope excluded; inline-search legacy dies with #720).
- No deletion of the `SelectedRowSurface` family, `selected_bg` plumbing, or theme entries — Inline still resolves through them.
- No palette constants for the zebra pairs (caller-supplied, per 719 D1).

## Decisions

### D1 — Library zebra attaches at the `panel_list.rs` seam

Both Wide arms (`Wide` and `WideWorkspace`) chain the library `ZebraStripe` (`#48584e` focused / `#2d353b` unfocused, inline `Color` values like the queue site) in `PanelList::set_paint_policy`. One edit site covers both library Wide lists; destinations and the queue site are untouched.

Alternative: attach at the `wide.rs` call sites. Rejected — the panel owns the policy decision per the D3 seam, and `panel_list.rs` is where the two Wide policies are already constructed.

### D2 — Delete the gutter opt-in; hardcode gutter at the Wide call site

Remove `with_selected_gutter`/`selected_gutter` from `WideMediaListPaintPolicy`. The `selected_gutter: bool` parameter on `media_list_row` stays (Inline still passes `false`), but the Wide adapter passes `true` unconditionally. The queue call site drops the builder call and keeps only its zebra pair.

Alternative: keep the flag and set it on the library policies. Rejected — a variant arm with every user opted in is a flag with no information; the `ui-design-system` spec calls that shape a defect.

### D3 — Parity inherits 719 as-merged

The stripe counter (`visible_item_index`, items only, stripes on even indices) is shared code; the library gets whatever parity 719 merged. No separate parity decision, no library-only counter. Library regression tests assert the same positions as the queue tests.

### D4 — Accent-on-accent episode rows accepted without special-casing

Two-tone episode rows already paint the secondary title in the focus-accent role; a selected episode row paints its primary in the same role, bold. Accepted as-is — carving a selected-row exception for two-tone rows would recreate the per-case branching this change removes.

## Risks / Trade-offs

- [719 stack risk] If 719's merged shape differs from its current branch tip (parity, gutter details), this change's tests and spec scenarios drift. → Mitigation: re-verify the two inherited behaviours against the merged 719 before implementing; they are named explicitly in tasks.md.
- [`selected_bg` becomes a dead value on the Wide path] The Wide adapter still resolves a surface colour that the painter ignores; Inline is its only real consumer. → No mitigation beyond a comment at the call site; splitting `media_list_row` would be a larger diff for zero visual difference.
- [Stale 719 doc comment] The branch's `row.rs` docs claim the owning panel paints a selection marker outside the panel edge; no such marker exists since the icon was dropped. → Correct the comment in this change if 719 hasn't.
