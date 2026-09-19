# Invariant 7 — A shared colour value is a deliberate split; an alias is a deliberate bond

**Scope:** `src/app/render/theme/` — the value tier (`primitives.rs` today, the
`Palette` enum in `palette.rs` once `palette-enum` lands), the role tier
(`mod.rs`, 51 role consts), and the surface tier (`surface.rs` level fills,
`surface_table.rs` rows, `surface_resolve.rs`) — plus the re-export bridge
`src/app/palette.rs` and every `palette::` consumer.

## The invariant

Two symbols in the theme can hold the same colour in two opposite ways, and the
syntax is the distinction:

- **Two independent literals with equal values** assert that the two are
  *independently editable*. They are equal today and either may move alone.
- **An alias (`const X: Color = Y;`)** asserts that the two *must match*. An
  edit to `Y` is meant to reach `X`.

`primitives.rs` currently holds 42 consts over 29 distinct values: 40 literals
and 2 aliases. Eight values are shared by more than one symbol:

| Value | Symbols | Split rationale in code |
|---|---|---|
| `#1e2326` | `DARK_BG`, `PILL_SELECTOR_BG`, `PILL_SELECTOR_ROW_BG`, `PILL_SELECTOR_SELECTED_FG` | **none — independence unstated** |
| `#2d353b` | `ARTWORK_PLACEHOLDER`, `LIBRARY_SIDE_BG`, `SELECTED_ROW_BAR` | yes, on `SELECTED_ROW_BAR` |
| `#3c4841` | `SURFACE_FOCUSED_BG`, `BG_GREEN`, `PILL_SELECTOR_OVERFLOW_FG` *(alias of `BG_GREEN`)* | yes, on both literals |
| `#48584e` | `SOFT_CONTENT_BODY_BG`, `BG_GREEN_SOFT`, `SCROLLBAR` *(alias of `BG_GREEN_SOFT`)* | yes, on both literals |
| `#3f3f3f` | `OVERLAY`, `ARTWORK_LOADING_PLACEHOLDER` | yes, on both |
| `#35a77c` | `AQUA`, `PLAYBACK_TITLE` | yes (`now-playing-media-type-titles` D2) |
| `#dbbc7f` | `YELLOW`, `PLAYBACK_CONTEXT` | yes (`now-playing-media-type-titles` D2) |
| `#3a94c5` | `FOAM`, `PILL_SELECTOR_SELECTED_BG` | yes, on `PILL_SELECTOR_SELECTED_BG` |

Seven of the eight carry the rationale in a doc comment naming the *other*
symbol and the change that split them:

```rust
/// The now-playing title row's title-part value. A primitive of its own, not
/// the brand aqua's `AQUA`, whose value it shares today: a brand-aqua edit
/// can never move the now-playing title (now-playing-media-type-titles D2).
pub(super) const PLAYBACK_TITLE: Color = Color::Rgb(53, 167, 124);
```

Note both aliases sit *inside* a split group: `PILL_SELECTOR_OVERFLOW_FG` is
bonded to `BG_GREEN` while `SURFACE_FOCUSED_BG` is split from it. Same value,
three symbols, two different contracts. Read the syntax, not the hex.

Regenerate the table rather than trusting it:

```bash
rg -o 'const (\w+): Color = Color::Rgb\(\s*(\d+),\s*(\d+),\s*(\d+)' \
   src/app/render/theme/primitives.rs
```

## Why it matters

The role tier exists so an edit reaches exactly the places the editor intended.
A shared value whose contract is unstated is indistinguishable from one that
must stay shared, so the next editor guesses — and guessing wrong is invisible:
the build passes, the tests pass, and a colour moves on a screen nobody was
looking at.

## What breaks if it is violated

- **Collapsing a split.** Merging `AQUA` and `PLAYBACK_TITLE` into one symbol
  makes a brand-accent edit silently repaint the now-playing title — the exact
  regression `now-playing-media-type-titles` D2 was written to prevent.
- **Breaking a bond.** Replacing the `SCROLLBAR = BG_GREEN_SOFT` alias with an
  independent literal makes a scrollbar edit stop following its source, which
  is the opposite of what the alias declares.
- **Misreading a split as the defect.** This has happened. The `palette-enum`
  change's original proposal named the duplicate literals and their comments as
  the problem — "duplicate `Rgb` literals and primitive-to-primitive aliases
  that defeat the edit-isolation the code comments claim" — and its task 4.1
  deleted the comments. The work was built, gated, reviewed clean, and reverted
  (`861a83fe`) once the premise was checked against the code. The duplicates
  *were* the isolation mechanism.

A consequence worth stating: a uniqueness test over a closed palette
(`assert_ne!(a.color(), b.color())`) forbids two variants at one hex. That is
correct — real divergence means a new hex — but it means a "same value today,
different tomorrow" placeholder cannot live in the value tier. It lives in the
role tier, as two roles naming one value plus the comment above.

## How the code maintains it today

By comment and review only. There is no test that a split stays split, and one
is not easily written: the two symbols are equal by construction, so no
assertion distinguishes "still deliberately equal" from "accidentally merged".
The enforcement that does exist is narrower:

- `ui-design-language` req 2 keeps raw values private to the theme, so a
  consumer cannot bypass the role tier.
- The one closed surface resolver (`surface_colors`) keeps paint sites from
  naming roles for surfaces at all.
- Once `palette-enum` lands, the `Palette` uniqueness and `docs/palette.json`
  drift tests (`theme/palette.rs`) pin the value tier against the approved name
  table and the docs.

`#1e2326`'s four symbols are the live gap: nothing records whether the tab-bar
background and the three pill-selector symbols are independent or bonded.

## Cheapest strengthening (not done here)

1. Keep each rationale adjacent to its symbol, naming the *other* symbol and the
   change that split them, so a reader landing on either half finds the rule.
   This is the only mechanism that has worked — treat deleting one as a
   behavioural change, not a comment cleanup.
2. When a split genuinely diverges, give the moving symbol its new value and
   delete that group's rationale in the same commit; the comment's
   disappearance is then the record that the split became real.
3. Document `#1e2326`'s four symbols one way or the other, next time someone is
   in that file for another reason.
4. Do not attempt a "these two must differ" test. It asserts nothing while the
   values are equal, which is their entire declared state.

## For an agent touching theme colours

- Read this file and `openspec/specs/ui-design-language/spec.md` before changing
  any symbol under `src/app/render/theme/`.
- A duplicated literal with a comment is load-bearing. Verify the comment
  against the change it cites before treating the duplication as cleanup.
- Line numbers in colour plans go stale fast — every migration inserts or
  removes lines above the literals, and a revert moves them back. Re-derive with
  `rg` against HEAD rather than trusting a plan, a handoff, or this file.
