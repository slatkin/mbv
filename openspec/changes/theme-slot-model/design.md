# Design

## Context

See proposal.md (Why). The current shape of `crates/mbv-theme`:

- `palette.rs`: the private `Palette` enum, 19 hue-named variants, with `color()` holding the only
  `Color::Rgb` literals.
- `lib.rs`: 55 `pub const ROLE: Color` constants and two `[Color; 3]` role sets
  (`HERO_META_ROLES`, `HINT_PILL_FILLS`).
- `surface.rs`, `surface_table.rs`, `surface_resolve.rs`: the closed `Surface` enum (25 rows)
  plus `Level`, `FocusSource`, a `soft` flag, `RESTING_DEVIATIONS` and a `debug_assert` in
  `surface_colors`.
- `palette_json.rs`: a test that regenerates `docs/palette.json` by parsing the text of
  `palette.rs` and `lib.rs`.

Consumers import the crate as `palette` (`use mbv_theme as palette;`). There are about 280
role-constant references (`mbv-render` ~230, `mbv-components` ~38, `mbv` binary ~29) and
about 120 `Surface`/`surface_colors` references.

Facts that shape the approach:

- `ratatui-core` 0.1.2's `Style::fg`/`Style::bg` take `Color`, not `Into<Color>`, so a role
  type cannot convert at the call site implicitly. Each call site appends `.color()`.
- `Level::focused_fill()` returns `SURFACE_FOCUSED` for all five levels, and 7 of the 9 rows
  with a level default are listed in `RESTING_DEVIATIONS`. Only `FocusSource::Fixed` changes
  a resolved colour; `soft` swaps `Green1` for `Green2`. Each row's colour is therefore fully
  described by one `(resting, focused)` pair.
- `media_list/row.rs:535` (`selected_bg == palette::SELECTED_ROW_BG`) is a dead branch:
  `selected_bg` has one source, `selected_row_surface_color` in `media_list/wide.rs`, which
  always returns `SELECTED_ROW_BG`.
- `media_list.rs`'s `ZebraStripe { focused: Color, unfocused: Color }` is a hand-rolled
  surface pair; `zebra_bg()` picks a half by the policy's `palette_focused` bit.
- `hero_composition.rs:190` picks `WORKSPACE_FOCUSED_FILL` (`Green2`) when focused and
  `surface_colors(MainContentBox, false)` (`Slate`) when not. `MainContentBox` already
  resolves to exactly these two values (soft focused `Green2`, resting `Slate`).

## Goals / Non-Goals

**Goals:**

- A theme is one value holding 19 slot colours; nothing else in the codebase holds an RGB.
- Every role and surface resolves through the active theme when painting.
- One authority per kind: fills come only from the Surface table, foregrounds only from roles.
- Every painted cell is the same colour before and after.

**Non-Goals:**

- Selecting, configuring, or loading a theme. `active()` returns the default.
- Pruning or renaming text roles beyond the mechanical enum conversion.
- Theming `Color::White`/`Black`/`Reset` specials, the pinwin `[panel] accent_color`, or the
  mpv Lua overlays.
- Changing `SelectedRowSurface` (`media_list/wide.rs`) or any focus bit a paint site passes.

## Decisions

### D1. Slots: 19, named by tier or hue

| Old variant | Hex | Slot |
|---|---|---|
| Ink | `#1e2326` | `BgDim` |
| Slate | `#272e33` | `Bg0` |
| Storm | `#2b3238` | `Bg1` |
| Green1 | `#2e383c` | `Bg2` |
| Green2 | `#374145` | `Bg3` |
| Grey1 | `#1a1a1a` | `OnAccent` |
| Green3 | `#6c766c` | `FgFaint` |
| Grey2 | `#9e9e9e` | `FgMuted` |
| Grey3 | `#e6e6e6` | `Fg` |
| Cream | `#faedcd` | `FgWarm` |
| White | `#fdf6e3` | `FgBright` |
| Red | `#e57e80` | `Red` |
| Orange | `#e59875` | `Orange` |
| Yellow | `#dbbc7f` | `Yellow` |
| Iris | `#a7c080` | `Green` |
| Green | `#93b259` | `GreenDeep` |
| Aqua | `#35a77c` | `Aqua` |
| Foam | `#3a94c5` | `Blue` |
| Mauve | `#d699b6` | `Purple` |

Backgrounds are numbered tiers (`Bg0`–`Bg3`, darkest-first), as Everforest and base16 number
theirs; a light theme simply assigns lighter values. Accents keep hue names because most schemes
keep a slot's hue family. Note `Iris` becomes `Green` and the old `Green` becomes `GreenDeep`;
the implementer maps by this table, never by old name.

Alternative considered: role-named or job-named background slots (`backdrop`, `chrome`,
`resting`). Rejected: those are roles, and putting them in the theme recreates the two-authority
problem inside the theme.

### D2. `Theme` is a value; `active()` is the only resolution point

```rust
// slot.rs (crate-private)
pub(crate) enum Slot { BgDim, Bg0, Bg1, Bg2, Bg3, OnAccent, FgFaint, FgMuted, Fg, FgWarm,
                       FgBright, Red, Orange, Yellow, Green, GreenDeep, Aqua, Blue, Purple }
pub(crate) struct Theme { /* one Color per slot */ }
impl Theme {
    pub(crate) const DEFAULT: Theme = /* D1 values */;
    pub(crate) const fn get(&self, slot: Slot) -> Color;
}
pub(crate) fn active() -> &'static Theme { &Theme::DEFAULT }
```

`Slot`, `Theme` and `active()` stay crate-private: no consumer needs them until a theming change
adds selection, and that change decides set-once (`OnceLock`) versus live switching behind
`active()` without touching any consumer. A `Theme` stores its colours in a fixed array indexed by
`Slot as usize` or as named fields; either way `get` is the only accessor, so the spec's "no
indexable table outside the theme" holds.

Alternatives considered: a global `OnceLock<Theme>` now (rejected: nothing sets it yet, so it is
speculative); passing `&Theme` through every painter (rejected: hundreds of signatures for no
gain over one resolution function); deferring resolution by encoding slots as
`Color::Indexed` and remapping the buffer after each frame (rejected: a `Color` in a model would
stop being a colour, and `dim_backdrop` and `image_fetch/protocol.rs` read real RGB mid-frame).

### D3. Roles: a closed `Role` enum

```rust
pub enum Role { TextPrimary, TextMuted, Accent, Duration, ... }
impl Role {
    pub(crate) const fn slot(self) -> Slot;          // the one role table
    pub fn color(self) -> Color { self.color_in(active()) }
    pub(crate) fn color_in(self, theme: &Theme) -> Color { theme.get(self.slot()) }
}
```

Variants are the old constant names in `UpperCamelCase` (`TEXT_PRIMARY` -> `TextPrimary`,
`PLAYBACK_TITLE_FG` -> `PlaybackTitleFg`); no renames beyond case. Call sites change from
`palette::TEXT_MUTED` to `palette::Role::TextMuted.color()`. The enum is generated with an `ALL`
list (as `declare_surfaces!` does today) so the generator iterates it.

Roles that survive (40): every current constant except the fill roles listed in D5. Their slot is
the slot of their current `Palette` variant via D1. `bar_role_fg` stays, taking and returning
`Color`, with `SELECTED_ROW_FG` resolved through `Role::SelectedRowFg`.

`HERO_META_ROLES` becomes `pub const HERO_META_ROLES: [Role; 3]`; its caller resolves the indexed
role with `.color()`.

Alternative considered: keep `pub const TEXT_MUTED: Role` constants. Rejected: two names per
role, and the constants cannot be iterated for the generator.

### D4. Surfaces: one `(resting, focused)` slot pair per row

```rust
pub enum Surface { ... }                         // still generated with ALL
const fn slots(surface: Surface) -> (Slot, Slot) // (resting, focused); the one fill table
pub fn surface_colors(surface: Surface, focused: bool) -> SurfaceColors  // signature unchanged
```

A fixed row states the same slot twice. `Level`, `FocusSource`, `Row`, `soft`,
`RESTING_DEVIATIONS`, the `debug_assert`, `Surface::level()` and the three row helper
functions are deleted. The evidence comments in `surface_table.rs` that cite call sites are
cut down to one line per row saying what the surface is.

Resulting table (existing rows keep their resolved colours exactly):

| Surface | Resting | Focused |
|---|---|---|
| QueueColumn | Bg1 | Bg2 |
| LibraryColumn | Bg0 | Bg2 |
| HeroPane | BgDim | BgDim |
| LibraryPanel | Bg1 | Bg3 |
| QueuePanel | Bg0 | Bg3 |
| MainContentBox | Bg0 | Bg3 |
| PlaybackPanel | Bg0 | Bg0 |
| QueueOnlyPlaybackPanel | BgDim | BgDim |
| SidebarBody | Bg3 | Bg3 |
| PlaybackStatusPill, ArtworkPlaceholder | Bg0 | Bg0 |
| ArtworkLoadingPlaceholder | FgMuted | FgMuted |
| QueueCardVisualizer, StatusBar, StatusBarPill, QueuePanelBand, SidebarBand, TabBar | BgDim | BgDim |
| PillRow, PillChip | BgDim | BgDim |
| PillChipSelected | Blue | Blue |
| QueueScopePillSelected | Aqua | Aqua |
| PillRowGap | Bg0 | Bg2 |
| PopupFrame | Bg2 | Bg2 |
| **SelectedRow** (new; replaces ContextMenuSelectedRow) | Green | Green |
| **ListStripe** (new) | Bg2 | Bg2 |
| **WorkspaceStripe** (new) | Bg1 | Bg2 |
| **CreditsStripe** (new) | Bg1 | Bg1 |
| **PopupBorder** (new) | Bg1 | Bg1 |
| **ModalButton** (new) | BgDim | BgDim |
| **TransportRow** (new) | Bg0 | Bg0 |
| **HintChip0 / HintChip1 / HintChip2** (new) | Blue / Yellow / Orange | same |

`ArtworkLoadingPlaceholder` keeps a foreground-ladder slot as a fill; that is today's colour and
stays so. A future theme author sees it in the table.

### D5. Fill roles fold into surfaces

| Removed role or row | Replaced by | Sites |
|---|---|---|
| `SELECTED_ROW_BG`, `Surface::ContextMenuSelectedRow` | `Surface::SelectedRow` | `media_list/wide.rs` `selected_row_surface_color`, `tree_browser.rs`, `playlists.rs`, `three_line_flat_list.rs`, `panel_list.rs` tests, `queue/tests.rs`, `context_menu.rs` |
| `PLAYLIST_STRIPE_BG`, `SETTINGS_STRIPE_BG`, `SESSIONS_STRIPE_BG` | `Surface::ListStripe` | `playlists.rs`, `settings_component.rs`, `three_line_flat_list.rs` |
| `WORKSPACE_FOCUSED_FILL` | `Surface::MainContentBox` with the site's own bit | `hero_composition.rs:190` becomes `surface_colors(MainContentBox, box_focused)` |
| `WORKSPACE_FOCUSED_STRIPE` + `SURFACE_RESTING` pair | `Surface::WorkspaceStripe` | `panel_list.rs` `WideWorkspace` policy |
| `HERO_CREDITS_STRIPE` | `Surface::CreditsStripe` | `overview_box.rs:311` |
| `SURFACE_RESTING` (modal border) | `Surface::PopupBorder` | `modal_frame.rs:65` |
| `SURFACE_CHROME` (confirm buttons) | `Surface::ModalButton` | `confirm_modal.rs:108` |
| `SURFACE_BACKDROP` (queue transport row, seek gauge bg) | `Surface::TransportRow` | `queue_band.rs:45,195` |
| `HINT_PILL_FILLS` | `HintChip0..2` via `pub const HINT_CHIPS: [Surface; 3]` | `widgets.rs:627` |
| `SURFACE_BACKDROP`, `SURFACE_CHROME`, `SURFACE_FOCUSED`, `SURFACE_RESTING`, `SURFACE_SIDEBAR`, `PILL_ROW_BG`, `PILL_BG`, `PILL_SELECTED_BG` | the slot directly in the Surface table | theme-internal only |

`ZebraStripe` changes from a `Color` pair to a `Surface`: `zebra_bg()` returns
`surface_colors(surface, self.palette_focused).fill`. `queue_row_zebra_stripe()` returns
`Surface::QueueColumn`; the Workspace policy uses `Surface::WorkspaceStripe` in both focus states.
That is equivalent because today's unfocused Workspace policy is built with
`for_library_workspace(false)`, so its `palette_focused` is false and it resolves the resting
`Bg1` either way.

The selection-bar foreground decision in `row.rs` drops the colour comparison: since every
`selected_bg` is the selection bar, `selected_row_foreground` always returns
`Role::SelectedRowFg.color()` and loses its `selected_bg` parameter. The `selected_bg` values
passed through `wide.rs`/`row.rs` keep their `Color` type; they come from
`surface_colors(Surface::SelectedRow, false).fill`.

Merges follow the spec's "one concept takes one identity". The rows a reviewer could argue
about: `TransportRow`, `ModalButton` and `PopupBorder` are new rows rather than reuse of
`PlaybackPanel`, `PillChip` and `QueueColumn`, because they are different concepts that share a
value today.

### D6. The generator iterates enums

`palette_json.rs` stops parsing source. It walks `Slot::ALL`, `Role::ALL` (with `role.slot()`),
`HERO_META_ROLES`, `HINT_CHIPS` and `Surface::ALL` (with `slots(surface)`). Output keys change:
`variants` -> `slots`; each role's `variant` -> `slot`; each surface drops `level`/`soft` and
reports `resting`/`focused` hexes plus slot names. Role and surface `name`s become the enum's
`Debug` names. `uses` prose is still carried over by name, so `docs/palette.json`'s existing
prose keys are renamed once by hand in the same task (constant case to `UpperCamelCase`), and the
prose of merged identities is combined.

### D7. Proving no colour changed

There is no screenshot gate. The evidence is:

1. **The generated `docs/palette.json`.** For every surviving role and pre-existing surface, the
   `hex` (role) or `resting`/`focused` hexes (surface) are unchanged. Every new surface's hexes
   equal those of the role(s) it replaced (D5). The task's reviewer compares the before/after
   files.
2. **The existing buffer tests.** Tests that assert `palette::X` colours are renamed
   mechanically to the role or surface that replaced `X` and must pass with no expected-value
   edits.
3. **One new `mbv-theme` test** owning the spec's "A theme is swapped" scenario: a test theme
   with distinct values per slot, resolved with `color_in`, returns that theme's colour for a
   role and the expected pair for a focus-reactive surface.

## Risks / Trade-offs

- [The workspace does not build mid-migration] -> Task 1 keeps the old `pub const` role names
  as transitional aliases (`pub const TEXT_MUTED: Color = Theme::DEFAULT.get(Slot::FgMuted)`)
  so each crate migrates with a green build; the last task deletes them.
- [Mis-mapping `Iris`/`Green` during the rename] -> Slots are mapped from D1's hex column; the
  palette.json hex comparison (D7.1) catches any slip.
- [A merged identity hides a deliberate difference] -> Only rows with identical resolved colours
  in both focus states merge (D5); the merge list is closed in this design.
- [`.color()` resolves per call instead of being a const] -> One match and one array read per
  call, negligible against painting.
- [Call-site verbosity: `palette::Role::TextMuted.color()`] -> Accepted; consumers may
  `use mbv_theme::Role;` where a file uses many roles.

## Migration Plan

Internal refactor in one change, landed as sequential commits per task (theme core with
aliases, fill folds, role call sites by crate, alias removal). Rollback is reverting the
commits; there is no persisted state, config, or protocol involved.
