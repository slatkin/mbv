## 1. Emby data model (mbv-core)

- [x] 1.1 Add `EmbyPerson { name, role, kind }` and `EmbyLink { name, url }`, replace `EmbyItem.genre`
  with `genres: Vec<String>`, delete `director`, and add `people`/`external_urls` (each
  `#[serde(default)]` so an older payload deserializes unchanged). Verify: `cargo check -p mbv-core`;
  `cargo check --workspace --all-targets` reports only the known `EmbyItem` literal sites.
- [x] 1.2 Parse the new fields in `parse_item` (`Genres`, `People` Name/Role/Type with `Role` optional,
  `ExternalUrls` Name/Url). Verify: parser unit tests for a full payload, a payload missing all three
  keys, a person with no `Role`, and a blank `Url`.
- [x] 1.3 Add `ExternalUrls` to the `Fields` query string in `api_client_library.rs` and
  `api_client_playlists.rs`. Verify: a mock-transport test asserting the recorded request's `Fields`
  parameter carries `ExternalUrls`.
- [x] 1.4 Update the `EmbyItem { .. }` literals in `crates/` test modules for the new fields. Verify:
  `cargo nextest run -p mbv-core` green.
- [x] 1.5 Update the `EmbyItem { .. }` literals in `src/` test modules for the new fields. Verify:
  `cargo nextest run -p mbv` green with no behavior change.

## 2. Movie hero content (producer)

- [ ] 2.1 Emit the Movie genre row (every genre, joined) and the provider-link row, and carry the links
  as typed `HeroFacts.links` so the painter has labels to overlay. Keep the row order release date,
  runtime, genres, links, and omit an empty row. Verify: unit tests for row order and for the absent
  genre/link rows, plus a Series and a Music album test proving their rows are unchanged.
- [ ] 2.2 Select the credits in the Movie branch of `hero_content_emby`: every `Director` in provider
  order, then at most 9 `Actor`s in provider order, other person types omitted, role falling back to the
  person's `kind` when empty. Verify: an `rstest` `#[case]` table over one director with 14 cast, two
  directors, no director, one director with 2 cast, no people, and a person with no role text.
- [ ] 2.3 Carry `credits` on `HeroContent` through the owner's content building and confirm the Narrow
  inline hero's plan ignores it. Verify: a content test asserting a Movie's credits reach
  `LibraryPanelContent` and that `inline_hero_plan` returns the same rows for the same content with and
  without credits.

## 3. Wide Hero box, credits table and clickable links

- [ ] 3.1 Extract the overview Main content box painting and its tests out of `hero_header.rs` into a
  sibling module in `components/library_panel/` with no behavior change. Verify: `hero_header.rs` and
  the new module are both under the 800-line cap, the moved tests pass unchanged, and a Wide library
  panel render is byte-identical before and after (existing buffer tests green).
- [ ] 3.2 Render the box when the item has overview text or credits, and paint the credits table one
  blank row under the overview text — or at the box's first content row when there is no overview.
  Verify: buffer tests for the box surface present/absent, the blank gap row, the no-overview case, and
  a Movie with neither overview nor credits showing no box.
- [ ] 3.3 Layout the table: name column sized from the longest rendered name, one shared second column,
  role truncated with an ellipsis at the box's edge, rows clipped at the box's bottom edge. Verify:
  buffer tests for column alignment across rows with different name lengths, a truncated role, and a
  short pane that clips the table without overflowing the pane.
- [ ] 3.4 Paint clickable link labels: sanitize (`http`/`https`, no control bytes), gate on declared
  terminal hyperlink support, and overlay one forced-width escape cell per label wholly inside the box
  and on a single painted row, after the row's text. Verify: unit tests for the sanitizer (non-http
  scheme, embedded control byte, empty URL) and the capability gate (supported/unsupported), plus a
  buffer test asserting the escape cell's symbol and `ForcedWidth`, and that an unsupported terminal,
  a truncated label and a rejected URL each render plain text.

## 4. Verification gates

- [ ] 4.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo nextest run -p mbv -p mbv-core`. Verify: all green.
- [ ] 4.2 Manual visual sweep in a real terminal: Wide Movies with and without an overview, with and
  without provider links, with zero, one and two directors, and a pane short enough to clip the table;
  confirm Narrow Movies, TV Series, Music, Audiobookshelf and Feeds heroes are unmoved. Verify: observed
  at both breakpoints, nothing overflows a pane, no escape bytes appear as visible text, and
  ctrl-clicking a link name opens its provider URL on a hyperlink-capable terminal (and that an
  unsupported terminal still shows plain text).

## 5. Documentation and archive

- [ ] 5.1 Record the new concepts in `CONTEXT.md` (the provider-link row and the credits table) and
  reconcile the *Main content box* entry, which describes the box as holding one kind-dependent payload,
  with a Movie box now holding the overview text plus the credits table. Verify: the glossary format is
  followed, no existing entry is left contradicting the new behavior, and
  `docs/architecture/interactive-surface-ledger.md` needs no change because no slot, hit region or
  geometry owner is added.
- [ ] 5.2 `openspec validate expand-movie-metadata --strict`. Verify: passes.
- [ ] 5.3 At archive, sync the deltas into `openspec/specs/library-panel/spec.md`. Verify:
  `openspec validate --specs` is clean and the modified overview requirement plus the three added
  requirements are present in the main spec.
