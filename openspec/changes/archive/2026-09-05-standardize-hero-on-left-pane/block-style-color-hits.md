# `Block::default().style(<Color>)` hits (task 1.2)

Factual inventory, tree-wide, of the `Block::default().style(...)` call taking
a bare `Color` (rather than a `Style`) as its argument. `impl From<Color> for
Style` sets **foreground only**, so this pattern is a silent background-paint
bug wherever it appears — the compiler cannot catch it because `Color` and
`Style` are both accepted by `.style()`.

Recorded for task 5.3's ast-grep rule
(`Block::default().style(<expr of type Color>)`, requiring explicit
`.bg()`/`.fg()`) so that rule can land against a known, closed baseline.

## Method

```
rtk grep -n "Block::default()\s*\.style(" src crates
rtk grep -n "\.style(palette::\|\.style(resolve_surface_focus\|\.style(crate::app::palette::" src crates
```
then manually confirmed each match's argument type against its definition.

## Hits (2, both in-scope for this change)

| Site | Argument | Fixed by |
|---|---|---|
| `src/app/render/components/audiobookshelf_book.rs:183` | `palette::resolve_surface_focus(focused && interaction.chapter_selection.is_some())` (`Color`) | task 2.2 |
| `src/app/render/components/audiobookshelf_book.rs:210` | `palette::resolve_surface_focus(focused && interaction.chapter_selection.is_none())` (`Color`) | task 2.2 |

Both hits are on `AudiobookshelfBookComponent`'s wide hero-on-left pane and
right-rail fills — the "ABS Books" defect the top-level analysis
(`/tmp/mbv-hero-pane-analysis.md`) already identified. Task 2.2 replaces both
with `hero_on_left_pane`/an explicit `.bg()`-based fill.

## Non-hits ruled out

- `src/app/render/components/chrome_status.rs:306` — `Block::default().style(bar_style)`
  where `bar_style: Style = Style::default().bg(palette::SURFACE_CHROME)`. Argument is
  already a `Style`, not a bare `Color`; not a foreground-only bug.

No other `Block::default().style(...)` call sites exist anywhere under `src/`
or `crates/` as of this change's HEAD (commit preceding this file). After
task 2.2 lands, this inventory's 2 hits are expected to be zero, and task
5.3's `rtk ast-grep scan` should report a clean tree with no separately-filed
baseline exceptions.
