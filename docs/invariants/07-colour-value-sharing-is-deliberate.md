# Invariant 7 — A shared slot is a deliberate split; one concept takes one identity

**Scope:** `crates/mbv-theme/src/` — the slot tier (the `Slot` enum and the
`Theme` value in `slot.rs`), the role tier (the `Role` enum in `role.rs`),
and the surface tier (the `surface_table.rs` rows, each one (resting,
focused) slot pair, resolved in `surface_resolve.rs`) — plus every consumer.

## The invariant

Two theme symbols can hold the same colour in two opposite ways, and the
distinction is what the symbols are:

- **Two independent roles naming one slot** assert that the two are
  *independently editable*. They are equal today and either may move alone,
  by repointing to a different slot. Until one moves, both follow the slot's
  one colour in the active theme.
- **Two symbols naming one concept** assert a bond that must not exist: one
  concept takes one identity (`ui-design-language` "One concept takes one
  colour identity"). A second identity created only so a concept's colour can
  be edited separately is the failure this file records — in both
  directions. Collapsing two concepts into one symbol silently repaints
  screens nobody was looking at; splitting one concept across two symbols
  lets the two drift until two screens paint "the same" thing in two
  colours.

The slot tier is the closed `Slot` enum: 20 variants named by tier or hue,
with exactly one `Rgb` literal each in `Theme::DEFAULT` — the only place a
palette literal lives. The deliberate splits live one tier up, as
independent roles and surface rows naming the same slot. Slots that host more
than one role today (re-derive; roles move):

| Slot | Roles sharing it |
|---|---|
| `FgMuted` | `TextSecondary`, `TextMuted`, `SplitRowTitleFg`, `ProgressTrack`, `SidebarScrollbar` |
| `Yellow` | `TextFocusAccent`, `TextHeroTitle`, `PlaybackContextFg`, `HeroCreditsName`, `AccentAudiobookshelf` |
| `Green` | `Duration`, `PlaybackValueFg`, `PlaybackMetaFg`, `IdleFeedTitleFg`, `HeroOverviewSeparator`, `AccentActive` |
| `Blue` | `WorkspaceHeaderFg`, `TextMetadata`, `GroupHeadingFg` |
| `Purple` | `TabSelectedUnderline`, `IndicatorAudioFg`, `PlaybackHostRemoteFg`, `EmptyQueueFg` |
| `Orange` | `PlaylistLoadedFg`, `IndicatorResolutionFg`, `ProgressPercent` |
| `Aqua` | `PlaybackTitleFg`, `Accent` |
| `FgWarm` | `TextEmphasis`, `SplitRowContextFg` |
| `BgDim` | `SelectedRowFg`, `PillSelectedFg` |
| `Bg2` | `TextAccentMuted`, `PillOverflowFg` |

Each group asserts independence: `Accent` and `PlaybackTitleFg` share `Aqua`
today, and a brand-accent edit must move the accent alone — the exact split
`now-playing-media-type-titles` D2 recorded. Read the role's intent, not the
hex.

The reverse direction — one concept, one identity — is what the slot-model
migration enforced by merging. Each merge below joined symbols that named the
same concept in the same colours, so the merge changed no pixel:

- the list selected-row fill and the context-menu selected row are one
  `Surface::SelectedRow`;
- the playlists, settings, and sessions stripes are one
  `Surface::ListStripe`;
- the workspace focused fill is `Surface::MainContentBox` resolved with the
  site's own focus bit.

And the merges stop where the concepts differ: `TransportRow`,
`ModalButton`, and `PopupBorder` are their own surface rows rather than
reuses of `PlaybackPanel`, `PillChip`, or `QueueColumn`, because they are
different concepts that share a value today. A future theme may move any of
them alone — and one did: the 2026-10-10 queue transport colour decision
gave `TransportRow` its own `BgDim1` slot, the split becoming real rather
than merely declared.

Re-derive the shared-slot table rather than trusting it:

```bash
rg -n "=> Slot::" crates/mbv-theme/src/role.rs
```

## Why it matters

The role tier exists so an edit reaches exactly the places the editor
intended. A shared slot whose contract is unstated is indistinguishable from
one that must stay shared, so the next editor guesses — and guessing wrong is
invisible: the build passes, the tests pass, and a colour moves on a screen
nobody was looking at. The same invisibility covers the reverse: two symbols
for one concept drift apart one edit at a time, and no build or test names
the drift.

## What breaks if it is violated

- **Collapsing a split.** Merging `Accent` and `PlaybackTitleFg` into one
  symbol makes a brand-accent edit silently repaint the now-playing title —
  the exact regression `now-playing-media-type-titles` D2 was written to
  prevent.
- **Splitting a concept.** Adding a second identity for an existing concept
  so its colour can be edited separately lets the two drift: two screens
  paint "the same" stripe, bar, or box in two colours, and no test
  distinguishes the drift from intent.
- **Orphaning a recorded split.** Repointing one half of a shared slot to a
  new slot without checking the other half's intent leaves the remaining
  symbol asserting a share that no longer exists — the divergence becomes
  invisible again.
- **Misreading a split as the defect.** This has happened. The `palette-enum`
  change's original proposal named the duplicate literals and their comments
  as the problem — "duplicate `Rgb` literals and primitive-to-primitive
  aliases that defeat the edit-isolation the code comments claim" — and its
  task 4.1 deleted the comments. The work was built, gated, reviewed clean,
  and reverted (`861a83fe`) once the premise was checked against the code.
  The duplicates *were* the isolation mechanism.

A consequence worth stating: the slot tier holds exactly one literal per
slot, so a "same value today, different tomorrow" placeholder cannot live
there. It lives in the role tier, as two roles naming one slot plus the
intent above.

## How the code maintains it today

By review only. There is no test that a split stays split, and one is not
easily written: the two symbols are equal by construction, so no assertion
distinguishes "still deliberately equal" from "accidentally merged". The
enforcement that does exist is narrower:

- `ui-design-language` keeps raw values private to the theme (`Slot`,
  `Theme`, and `active()` are crate-private), so a consumer cannot bypass
  the role tier.
- Roles serve only as foregrounds and fills resolve only through the one
  surface resolver (`surface_colors`): a background painted from a role is a
  review reject, not a build break.
- The `docs/palette.json` generator iterates `Slot::ALL`, `Role::ALL`, and
  `Surface::ALL`, so a new role or surface without a slot mapping breaks the
  snapshot instead of slipping through silently.
- The theme-swap test (`a_swapped_theme_recolors_roles_and_surfaces` in
  `slot.rs`) proves every role and surface resolves through the active theme
  — but it cannot tell a deliberate share from a merge. Splits stay review's
  job.

## Cheapest strengthening (not done here)

1. Keep each split's intent findable from either half, naming the *other*
   symbol and the change that split them, so a reader landing on either half
   finds the rule. This is the only mechanism that has worked — treat
   deleting one as a behavioural change, not a comment cleanup.
2. When a split genuinely diverges, give the moving symbol its new slot; the
   move itself is then the record that the split became real.
3. When a merge is proposed, check the concepts first: same concept in the
   same colours merges; different concepts sharing a value today keep their
   own rows, as `TransportRow`, `ModalButton`, and `PopupBorder` do.
4. Do not attempt a "these two must differ" test. It asserts nothing while
   the values are equal, which is their entire declared state.

## For an agent touching theme colours

- Read this file and `openspec/specs/ui-design-language/spec.md` before
  changing any symbol under `crates/mbv-theme/src/`.
- A shared-slot pair is load-bearing. Verify the intent against the change
  it cites before treating the share as cleanup.
- A new fill needs a surface row; a new foreground needs a role. Check first
  whether an existing identity already names the concept — adding a second
  one is the defect, not the fix.
- Line numbers in colour plans go stale fast — every migration inserts or
  removes lines above the literals, and a revert moves them back. Re-derive
  with `rg` against HEAD rather than trusting a plan, a handoff, or this
  file.
