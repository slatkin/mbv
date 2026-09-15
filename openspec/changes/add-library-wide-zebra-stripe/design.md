## Context

See proposal.md (Why) and `specs/canonical-media-lists/spec.md` (requirements).

Baseline is #719 **as merged** (`8a9132c2`, release v0.19.3) with its deltas already synced into
`openspec/specs/canonical-media-lists/spec.md`. Verified against the tree at `b0d0ea7c`:

- `WideMediaListPaintPolicy` (`src/app/components/media_list/mod.rs:217-268`) carries
  `zebra: Option<ZebraStripe>` (`ZebraStripe { focused, unfocused }`, `:210-214`) and a
  `selected_gutter: bool` opt-in (`:221`, `:257-264`). The Queue is its only caller
  (`src/app/render/components/queue.rs:31-39`).
- The Queue resolves its pair through the surface table — `palette::surface_colors(Surface::QueueColumn,
  focused).fill` — not raw hex; #719's cleanup commit `f18f3c54` removed the original `Color::from_u32`
  literals. Any new stripe pair must follow that shape.
- `render_wide_media_list_with_zebra` (`src/app/render/components/media_list/wide.rs:63-150`) threads
  both into `media_list_row`; `render_wide_media_list_component` passes `policy.selected_gutter()`
  (`:300-308`). The `#[cfg(test)]` wrapper `render_wide_media_list` (`:43-61`) hardcodes `None`/`false`,
  and the Inline adapter calls `media_list_row` with `None`/`false` (`:200-214`).
- Parity as merged: `visible_item_index % 2 == 0` stripes the **1st/3rd/5th** visible items
  (`wide.rs:97-98`), and the synced main spec says so (`spec.md:430`, `:437-440`).
- Selection as merged: under the gutter, `paint_selected` is false, so the zebra branch still runs —
  the selected row **keeps its stripe** (`row.rs:117`, `:215-233`; `spec.md:52` "zebra striping SHALL
  apply to it as to any other row").
- The two library Wide arms funnel through `PanelList::set_paint_policy`
  (`src/app/components/library_panel/panel_list.rs:31-49`) and sit on **different panel bodies**:
  the Browser-pane list box is filled with `Surface::LibraryPanel`
  (`src/app/components/library_panel/wide.rs:208-211`) — focused `#3c4841`, unfocused `#333c43`
  (`theme/surface_table.rs:150-155`); the Workspace box is filled with `Surface::MainContentBox`
  (`wide.rs:403-404`, keyed to the same `workspace.focused` bit the policy receives) — focused
  `#48584e`, unfocused `#2d353b` (`surface_table.rs:170-175`).
- The Browser-pane rail is letter-grouped (`src/app/components/browser_content.rs` →
  `letter_grouped_rows`), so its flow carries real `Heading`/`Spacer` rows; the Queue's does not.
- `Heading` rows paint bold in `TEXT_FOCUS_ACCENT` (`row.rs:44-52`) — the same role the gutter accent
  gives a selected title.

## Goals / Non-Goals

**Goals:**
- Visible zebra on both library Wide arms, each through a named surface identity.
- Gutter-accent as the only Wide selected-row path, with the opt-in deleted rather than extended.
- #719's merged behaviour inherited verbatim: parity, stripe containment, and the selected row keeping
  its stripe.

**Non-Goals:**
- No change to the Inline adapter or its selected-row background (Narrow is out of scope; inline-search
  legacy painters die with #720).
- No new palette rows or primitives: both library pairs are already in the surface table.
- No new marker glyph, and no re-litigating #719's accent-on-accent choice for two-tone episode rows.

## Decisions

### D1 — Each library arm stripes with the *other* arm's panel-body pair

Two pairs, one per arm, attached at the same seam: `PanelList::set_paint_policy`
(`panel_list.rs:31-49`) already builds a distinct policy per arm, so the Browser-pane arm
(`Wide`) chains the `MainContentBox` pair and the Workspace arm (`WideWorkspace`) chains the
`LibraryPanel` pair. Both resolve through
`palette::surface_colors(<surface>, focused).fill` at the call site, exactly like the Queue site.

A stripe must differ from the fill its own list box is painted with, or there is no visible
alternation — that is why one pair cannot serve both arms, and it is what the first draft of this
change got wrong: `#48584e`/`#2d353b` *is* `MainContentBox`, the Workspace box's own fill, so the
Workspace list would have striped invisibly in both focus states. No single existing pair works for
both arms (each candidate equals one arm's body), so "one library pair" is dropped as a requirement.

Alternative: a dedicated `Surface::LibraryRowStripe*` pair per arm. Rejected — same pixels, new table
rows, and the surface conformance coverage table would need two more entries to justify them.

Alternative: one pair for both arms with a new colour. Rejected — the two library bodies are already
`#3c4841`/`#333c43` and `#48584e`/`#2d353b`; any third tone is visible on one and closer on the other,
and swapping the two existing pairs is visible on both.

### D2 — Delete the gutter opt-in; the Wide adapter passes the accent unconditionally

Remove `with_selected_gutter`/`selected_gutter` and its accessor from `WideMediaListPaintPolicy`.
Rename `media_list_row`'s `gutter_glyph: bool` parameter to `gutter_accent` (there has been no glyph
since #719 dropped the icon) and keep it, because Inline still passes `false`; the Wide adapter passes
`true` unconditionally. The Queue call site drops the builder chain and keeps only its zebra pair.
Correct `row.rs`'s doc comment: it still claims the owning panel paints a marker outside the panel edge
(`row.rs:26-29`) — no such marker exists.

Alternative: keep the flag and set it on both library policies. Rejected — a caller-selected variant arm
whose only user is one screen is a defect to remove, not extend (`openspec/specs/ui-design-system/spec.md:85`).

### D3 — Multi-selected rows take the accent too, in library parity with the Queue

`wide.rs:100-105` folds multi-selection into the same `selected`/`focused` row arguments, so
unconditional accent changes library Visual-mode and Ctrl+Click rows from a `selected_bg` fill to a
bold focus-accent title that keeps its stripe. That is what the Queue ships today, so this change
makes the Wide presentation uniform instead of adding a second rule; the delta spec states it as the
contract rather than claiming multi-selection is untouched.

The one existing proof is `non_adjacent_multi_selected_rows_and_unfocused_cursor_paint_selected_surface`
(`src/app/render/components/media_list.rs:354-400`), which reaches the painter through the
test-only wrapper and so keeps asserting the pre-change shape. It is converted, not deleted, so the
Wide multi-select contract keeps an owner proof (AGENTS "deletion evidence").

### D4 — Parity and stripe/selection interaction inherit #719 verbatim

No library-only counter, no library-only override: the selected row keeps its zebra parity and adds the
accent, and the library regression tests assert the same positions the Queue tests assert. The first
draft's "the selected row always shows the selected-row treatment instead of the stripe" contradicted
both the merged code and the synced main spec and is gone.

The delta carries this as REMOVED + ADDED rather than MODIFIED: `openspec validate --strict` refuses a
MODIFIED block that omits a scenario the main spec still has ("Selected row overrides zebra"), which is
the deliberate removal here. Keep that shape unless the scenario is genuinely staying.

### D5 — The owning-surface identities survive as the Wide scrollbar backing only

After D2 the policy's `SelectedRowSurface` no longer paints a selected row on the Wide path; on a
focused overflowing list it still resolves the scrollbar column's backing fill
(`wide.rs:136-140`), so `OwningQueueColumn` and `OwningLibraryPane` keep a real (if small) observable
effect and stay. `InlineMediaBrowserPaintPolicy` only ever sets `ListBackdrop`
(`components/media_list/mod.rs:293-300`), so `SelectedRow` remains Inline's — and the legacy
`list_rows.rs` painters' — identity. Say so in the spec (done) and in the conformance module's coverage
table, whose rows for those two surfaces cite `components/tv_wide_tests.rs`, a file that no longer
exists (`src/app/render/tests_surface_conformance.rs:41-42`).

Alternative: delete the `Owning*` variants and their surface rows. Rejected for now — they still
distinguish the two columns' scrollbar tints; revisit if a later change unifies those.

### D6 — Domain terms get written down with the change

`CONTEXT.md` (Presentation) gains **Zebra stripe** and **Gutter accent**, with the *Avoid* list naming the
draft's synonyms ("gutter-selected style", "gutter treatment", "selection marker"), per AGENTS.md
"add new terms with the change".

## Risks / Trade-offs

- [Invisible stripe again] A future arm that resolves its stripe from its own box surface stripes
  nothing. → Mitigated by the spec's "A stripe never equals its own panel body" scenario, which each
  arm's test asserts against the real box fill rather than a colour constant.
- [Accent reads like a heading] The Browser rail's `Heading` rows are already bold focus-accent, so a
  selected item row shares that role. → Accepted as #719's treatment (D4); check it in the manual gate
  and raise a follow-up only if it is unreadable in practice.
- [`selected_bg` on the Wide path is nearly inert] Only the scrollbar column consumes it. → Comment at
  the Wide call site (D5); splitting `media_list_row` would be a larger diff for zero visual difference.
- [Test-only wrapper drift] `render_wide_media_list` keeps `gutter_accent = false`, so tests that use it
  pin a shape production no longer emits. → Convert the Wide-contract tests in
  `src/app/render/components/media_list.rs` to the component adapter; leave the wrapper for geometry and
  scroll tests that do not assert selection paint.
