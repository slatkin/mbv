# Invariant 7 — A shared colour value is a deliberate split; an alias is a deliberate bond

**Scope:** `src/app/render/theme/` — the value tier (the `Palette` enum in
`palette.rs`, landed by `palette-enum`), the role tier (`mod.rs`, 51 role
consts), and the surface tier (`surface.rs` level fills, `surface_table.rs`
rows, `surface_resolve.rs`) — plus the re-export bridge `src/app/palette.rs`
and every `palette::` consumer.

## The invariant

Two symbols in the theme can hold the same colour in two opposite ways, and
the distinction is what the symbols are:

- **Two independent roles naming one `Palette` variant** assert that the two
  are *independently editable*. They are equal today and either may move
  alone, by repointing to a different variant. Until one moves, both follow
  the variant's one literal.
- **An alias (`const X: Color = Y;`)** asserts that the two *must match*. An
  edit to `Y` is meant to reach `X`. Theme code currently holds no alias:
  `palette-enum` deleted the two that existed, and their former bonds survive
  only as the comments recording them.

The value tier is the closed `Palette` enum: 29 variants, one `Rgb` literal
each in `const fn color()`, with a uniqueness test forbidding two variants at
one hex — the tier can no longer hold a duplicate. The deliberate splits live
one tier up, as independent roles and surface values naming the same variant.
The eight shared values the primitive tier carried over, and where each
group's rationale lives now:

| Value | Variant | Symbols | Split rationale in code |
|---|---|---|---|
| `#1e2326` | `Ink` | `SURFACE_CHROME`, `PILL_ROW_BG`, `PILL_BG`, `PILL_SELECTED_FG` | **none — independence unstated** |
| `#2d353b` | `Slate` | `SURFACE_BACKDROP`, `SELECTED_ROW_BG` | yes, on `SELECTED_ROW_BG` |
| `#3c4841` | `Green1` | `SURFACE_FOCUSED`, `TEXT_ACCENT_MUTED`, `PILL_OVERFLOW_FG` *(former alias of the text green)* | yes, on both split halves |
| `#48584e` | `Green2` | `SCROLLBAR` *(former alias of the soft fill)*, the soft content-body fill (`surface_resolve.rs`) | yes, on both |
| `#3f3f3f` | `Grey2` | `BORDER_UNFOCUSED`, the `ArtworkLoadingPlaceholder` row (`surface_table.rs`) | yes, on the row |
| `#35a77c` | `Aqua` | `ACCENT`, `PLAYBACK_TITLE_FG` | yes (`now-playing-media-type-titles` D2) |
| `#dbbc7f` | `Yellow` | `TEXT_FOCUS_ACCENT`, `TEXT_HERO_TITLE`, `HERO_CREDITS_NAME`, `PLAYBACK_CONTEXT_FG` | yes, on `PLAYBACK_CONTEXT_FG` (`now-playing-media-type-titles` D2) |
| `#3a94c5` | `Foam` | `TEXT_METADATA`, `PILL_SELECTED_BG` | yes, on `PILL_SELECTED_BG` |

(`ROW_DATE_FG` — formerly in the Yellow group — was deleted by
`unify-row-metadata-gutter`; its split rationale collapsed into
`STATUS_AVAILABLE`, which now carries the media-row date/year gutter.)

Seven of the eight carry the rationale in a comment naming the *other*
symbol and the change that split them:

```rust
/// The now-playing title row's title part: the item's own name (episode,
/// track, entry, ...). Its own role rather than `ACCENT`/`TEXT_FOCUS_ACCENT`,
/// whose `Palette::Aqua` value it shares today: the two are equal today and
/// independently editable, so a focus-accent or brand edit moves the accent
/// alone (now-playing-media-type-titles D2).
pub const PLAYBACK_TITLE_FG: Color = Palette::Aqua.color();
```

Note both former aliases sat *inside* a split group: `PILL_OVERFLOW_FG` was
bonded to the text green while `SURFACE_FOCUSED` was split from it. Same
value, three symbols, two different recorded contracts — and since
`palette-enum` removed the alias tier, both are independent roles over
`Palette::Green1` whose comments carry the difference. Read the comment, not
the hex.

Regenerate the table rather than trusting it:

```bash
rg -o 'Palette::(\w+) => Color::Rgb\(\s*0x(..),\s*0x(..),\s*0x(..)' \
   src/app/render/theme/palette.rs
```

## Why it matters

The role tier exists so an edit reaches exactly the places the editor intended.
A shared value whose contract is unstated is indistinguishable from one that
must stay shared, so the next editor guesses — and guessing wrong is invisible:
the build passes, the tests pass, and a colour moves on a screen nobody was
looking at.

## What breaks if it is violated

- **Collapsing a split.** Merging `ACCENT` and `PLAYBACK_TITLE_FG` into one
  symbol makes a brand-accent edit silently repaint the now-playing title —
  the exact regression `now-playing-media-type-titles` D2 was written to
  prevent.
- **Orphaning a recorded split.** Repointing one half of a documented share
  (giving it a new variant) without deleting that group's rationale in the
  same commit leaves a comment asserting a share that no longer exists — the
  divergence becomes invisible again.
- **Misreading a split as the defect.** This has happened. The `palette-enum`
  change's original proposal named the duplicate literals and their comments as
  the problem — "duplicate `Rgb` literals and primitive-to-primitive aliases
  that defeat the edit-isolation the code comments claim" — and its task 4.1
  deleted the comments. The work was built, gated, reviewed clean, and reverted
  (`861a83fe`) once the premise was checked against the code. The duplicates
  *were* the isolation mechanism.

A consequence worth stating: the uniqueness test over the closed palette
(`assert_ne!(a.color(), b.color())`) forbids two variants at one hex. That is
correct — real divergence means a new hex — but it means a "same value today,
different tomorrow" placeholder cannot live in the value tier. It lives in
the role tier, as two roles naming one variant plus the comment above.

## How the code maintains it today

By comment and review only. There is no test that a split stays split, and one
is not easily written: the two symbols are equal by construction, so no
assertion distinguishes "still deliberately equal" from "accidentally merged".
The enforcement that does exist is narrower:

- `ui-design-language` req 2 keeps raw values private to the theme, so a
  consumer cannot bypass the role tier.
- The one closed surface resolver (`surface_colors`) keeps paint sites from
  naming roles for surfaces at all.
- The `Palette` uniqueness and `docs/palette.json` drift tests
  (`theme/palette.rs`), landed by `palette-enum`, pin the value tier against
  the approved name table and the docs.

`#1e2326`'s four symbols are the live gap: nothing records whether the tab-bar
background and the three pill-selector symbols are independent or bonded.

## Cheapest strengthening (not done here)

1. Keep each rationale adjacent to its symbol, naming the *other* symbol and the
   change that split them, so a reader landing on either half finds the rule.
   This is the only mechanism that has worked — treat deleting one as a
   behavioural change, not a comment cleanup.
2. When a split genuinely diverges, give the moving symbol its new variant and
   delete that group's rationale in the same commit; the comment's
   disappearance is then the record that the split became real.
3. Document `#1e2326`'s four symbols (`SURFACE_CHROME`, `PILL_ROW_BG`,
   `PILL_BG`, `PILL_SELECTED_FG`) one way or the other, next time someone is
   in that file for another reason.
4. Do not attempt a "these two must differ" test. It asserts nothing while the
   values are equal, which is their entire declared state.

## For an agent touching theme colours

- Read this file and `openspec/specs/ui-design-language/spec.md` before changing
  any symbol under `src/app/render/theme/`.
- A shared-variant pair's comment is load-bearing. Verify the comment against
  the change it cites before treating the share as cleanup.
- Line numbers in colour plans go stale fast — every migration inserts or
  removes lines above the literals, and a revert moves them back. Re-derive with
  `rg` against HEAD rather than trusting a plan, a handoff, or this file.
