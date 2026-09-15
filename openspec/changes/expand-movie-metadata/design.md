## Context

See `proposal.md` for motivation. What shapes the approach:

- **One painter per surface.** Movies reach the Wide Hero pane through `BrowserContent` →
  `hero_content_emby` → `hero_header.rs::paint_hero_pane_content`. The Narrow inline hero
  (`narrow.rs`) derives from the same `HeroContent` but paints its own form. There is no legacy movie
  hero painter left.
- **The overview box already owns the surface and the fill rule.** With no Workspace, the box stretches
  to the pane's bottom edge; the pane is read-only and does not scroll. Movies never have a Workspace.
- **What Emby returns** (verified against a live server, not assumed): `Fields=ExternalUrls` on the
  existing list query returns full provider links, so no extra request is needed; `People` is ordered
  cast-first with directors anywhere in the list (observed at index 0, 8, 11, 12, 20 and 56); a
  director's `Role` is usually absent while an actor's role is the character name; of 60 movies, 6
  declared two directors and 2 declared none; 69 of 1126 people had no role text.
- **`EmbyItem` carries none of it.** `genre` holds only the first genre and `director` holds one name
  that nothing in production reads.
- **ratatui 0.30.2 already supports the hyperlink cell, end to end.** `ratatui-core 0.1.2` has
  `CellDiffOption::ForcedWidth` (`buffer/cell.rs`) and `CellWidth` reads it back; its diff walk advances
  past the forced span and emits the cell when it differs; and ratatui's own tests
  (`buffer/buffer.rs::merge_diff_link`, `merge_diff_split_link`) build exactly the OSC 8 pattern
  `\x1b]8;;url\x1b\\label\x1b]8;;\x1b\\` in a forced-width cell. The `ratatui-crossterm 0.1.2` backend
  writes that cell with `Print(cell.symbol())` (`src/lib.rs:274`) — the escape bytes reach the terminal
  unfiltered — and re-issues `MoveTo` unless the previous cell was at `x - 1` (`:244`), so the cursor
  does not drift across the forced span. mbv draws through `Terminal::draw`, so this is the live path.
- **Precedent, unimplemented.** `idle-feed-rotation` already specifies OSC 8 feed titles; there is no
  escape sequence anywhere in `src/` today, so this change ships mbv's first hyperlink.
- **No second routing site.** ADR 0024 keeps component-surface pointer routing with the surface that
  painted it; the Library panel already resolves pill rows itself. Adding hero-pane link hit geometry
  would be a new routing path, which the chosen approach avoids entirely.

## Goals / Non-Goals

**Goals:**

- A Movie hero shows the metadata the server already sends: cast and crew, all genres, provider links.
- Zero new requests, zero new dependencies, zero new routing sites, one painter change per surface.
- A clickable link that never emits anything but a sanitized `http`/`https` URL.

**Non-Goals:**

- No change to the Narrow inline hero's form, and no cast table in Narrow.
- No change to any other item type's hero content.
- No keyboard binding, no hit geometry, and no URL copying affordance for links.
- No shared "hyperlink" abstraction beyond what this one row needs; the `idle-feed-rotation` feed title
  is not wired up here.

## Decisions

### D1 — The cast and crew table renders inside the existing overview Main content box

The box already owns the recessed surface, the shared padding and the "fills the pane when nothing
follows" rule. The table is body content in that box: overview text, one blank row, table.

*Alternatives:* a shared optional section in `HeroContent` rendered by both the Wide and Narrow
painters (rejected: the user scoped this to Wide, and Narrow's in-flow block has a 10-row image cap and
an "admit the block or restore the ordinary row" fallback that a 10-row table would regularly break);
a Movies-only extension block (rejected: `ui-design-language` closes the inline-hero vocabulary and
`library-panel` forbids destination-only structure, so an exception would need spec amendments the
shared box does not).

*Consequence:* `HeroContent` gains `credits`, produced by the shared Emby producer and painted only by
`hero_header.rs`. `narrow.rs` deliberately ignores it — the cast table is a Wide-pane element, and the
spec states that. If Narrow is ever to carry it, the field is already on the content it reads.

### D2 — New hero content is keyed on the Emby item type `Movie`, not on the destination

`hero_content_emby` branches on `item.item_type == "Movie"` for the genre row, the link row and the
credits. A Series keeps its combined year-and-genre row and gets neither: it would otherwise show its
genre twice and spend ~10 rows of its hero pane that its seasons/episodes Workspace needs. Keying on
the item type rather than the tab means a Movie in a generic Emby library renders identically to one on
the Movies tab, so no destination-only difference appears.

### D3 — `EmbyItem` gains `people`, `genres` and `external_urls`; `genre` and `director` go

```rust
pub struct EmbyPerson { pub name: String, pub role: String, pub kind: String }  // Role <- "Role", kind <- "Type"
pub struct EmbyLink   { pub name: String, pub url: String }                     // <- ExternalUrls[]
```

`genre: String` becomes `genres: Vec<String>` and `director: String` is deleted; both are replaced by
the new fields, and neither changes the field count materially. `ExternalUrls` joins the two `Fields`
query strings (`api_client_library.rs`, `api_client_playlists.rs`). The ~39 `EmbyItem { .. }` literals
in test modules take the new fields.

*Alternative:* keep `genre`/`director` and add alongside — rejected as dead duplication; `director` has
no production reader today and `genre` has exactly one (the Series meta row, which becomes
`genres.first()`).

### D4 — The links row is a typed row rendered last by the painter, not a new meta-row enum

`meta_rows: Vec<String>` stays the ordered plain-text list; `HeroFacts` gains
`links: Vec<HeroLink>` and the painter paints it as one more row after `meta_rows`. This keeps ~20
`HeroFacts { .. }` sites and both painters untouched while giving the painter the labels it must
overlay escapes on.

*Clarified 2026-09-14 during apply* (the spec delta requires exactly "one row joining the item's
provider link names" and says metadata rows "remain an ordered list of plain-text rows coloured by
position"): the producer pushes the joined link-name row into `meta_rows` like any other row, and
`HeroFacts.links` carries the typed names/URLs so the Wide painter can overlay the OSC 8 escape
cells onto that already-painted row, per D5's "painted normally first and then covered". The
painter SHALL NOT paint a second, separate links row.

*Ceiling:* the painter, not the producer, decides that links render last. Acceptable while no provider
needs links anywhere else; promote `meta_rows` to an ordered `Text | Links` row enum the day one does.

### D5 — Clickable links are painted as forced-width escape cells, not hit geometry

One cell per link carries the whole escaped label — opening sequence, label text, closing sequence —
with `CellDiffOption::ForcedWidth(label display width)`, written into `Frame::buffer_mut()` by the
Wide hero painter, after the row's text has been painted so nothing overwrites it. A link is painted
clickable only when its whole label lies on one painted row inside the box; otherwise it stays plain
text. The terminal owns ctrl-click.

*Alternatives:* mbv hit windows → slot event → shell request → spawned `xdg-open` (rejected: a new
component-surface routing path under ADR 0024, a new `ShellRequest`, hoisting `feed_actions`'
`open_url`, and a spawned process, all for one row); wrapping the escape inside a `Paragraph` span
(impossible — `Paragraph` has no per-cell hook).

This needs no prototype: the buffer, diff and backend behaviour above is read from the pinned sources
and covered by ratatui's own tests, and the mbv side of it is a buffer assertion. Whether a given
terminal acts on ctrl-click is a terminal fact with a graceful failure (plain text), checked in the
manual sweep rather than prototyped in throwaway code.

*Consequence of the diff model:* a forced-width cell makes the diff walk skip the cells it covers, so
nothing else may claim the label's columns. The label is painted normally first and then covered, which
keeps the read path (`Buffer` contents) honest.

### D6 — URLs are sanitized, and the escape is gated on declared terminal support

A URL is embedded only when it is `http`/`https` and contains no ASCII control byte; anything else
renders as plain text and does not affect its neighbours. The row renders plain text when the terminal
does not declare hyperlink support (the terminals `idle-feed-rotation` names: kitty, foot, iTerm2,
WezTerm, Windows Terminal, plus VTE-based terminals), defaulting to *not supported*.

*Rationale for the gate:* the failure asymmetry. A false negative loses a click; a false positive
prints escape bytes into the frame. The escape is a new trust boundary — the URL comes from the media
server's metadata, and mbv already strips OSC 8 out of feed text for exactly this reason.

### D7 — Every director, then at most 9 actors, in provider order

Directors do not consume cast places, so the table is 10 rows with one director, 11 with two and 9 with
none — the data supports all three (2/60 movies had no director, 6/60 had two). People typed as neither
`Director` nor `Actor` are omitted: the "crew" in *Cast & Crew* is the directors. A person's role is the
provider's role text, falling back to the provider type when it is empty, which is what makes a
role-less director read as `Director` and a role-less actor as `Actor`.

### D8 — Two aligned columns, sized from the table's own content, clipped at the box

The name column is the longest rendered name plus a gap, capped so a role column survives; the role
column spans the box's remaining width, and every role is right-aligned at the box's right edge so
the table uses the whole panel (amended 2026-09-15 from the user's visual sweep). A role longer than
the remaining width is truncated with an ellipsis at the box's edge and still painted right-aligned.
When the pane is shorter
than the box's content, the table clips at the box's bottom edge — the Hero pane does not scroll, and
the alternative (shrinking the artwork further) would fight the existing text-starvation rule.

## Risks / Trade-offs

- **[A forced-width escape cell is not re-emitted when unchanged]** → the escape is self-closing inside
  the label's span, so a skipped redraw is correct; if a future renderer (image protocol) clears the
  region outside ratatui's knowledge, the row must be repainted, and `AlwaysUpdate` is ratatui's lever
  (mutually exclusive with `ForcedWidth`, so it would need the split-cell form).
- **[Hyperlink support detection is a heuristic]** → the failure mode is a lost click on an unknown
  terminal, never visible garbage; the row renders as plain text.
- **[The table competes with the overview for a short pane]** → clipping at the box bottom is the
  specified behaviour, and the existing artwork-starvation rule still shrinks the Landscape artwork
  first, so the box gets its rows before the pane runs out.
- **[Server-supplied text reaching a cell as an escape]** → the sanitizer is the only path into a cell
  symbol, and it rejects anything but `http`/`https` with no control bytes.
- **[`People` ordering is provider data]** → the ordering rule is applied client-side; provider order is
  preserved within each group rather than re-sorted, so mbv never invents a ranking Emby did not send.

## Migration Plan

Client-only change; no stored state, no schema, no server contract changes. The new fields are additive
on the wire, so an older server simply returns no `ExternalUrls` and the row renders absent, as it does
for a movie with no links. Rollback is a revert of the change: nothing persisted depends on it.

## Open Questions

None. The escape-cell mechanism is settled by the pinned ratatui sources (buffer diff, `CellWidth`,
and the crossterm backend's raw `Print`) plus ratatui's own link tests, and the mbv side is asserted by
buffer tests; the only observation left is whether the user's terminal honors ctrl-click, which the
manual sweep covers and which degrades to plain text.
