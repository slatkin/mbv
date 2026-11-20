# Proposal

## Why

In the GUI trees, the only sign of depth is the row inset. In a long open series or artist, it is hard to see where a parent's rows stop and which season or album a row belongs to. The design reference `~/indent-guides.zip` (`Guides.dc.html`, "D · Indent guides only") adds thin vertical indent guides that connect each open parent to its rows.

## What Changes

- Every GUI tree shows indent guides: the Music tree (artist → album → track), the Shows All view (series → season → episode), and the Playlists tree (playlist → item).
- An open parent row that shows at least one child row starts a vertical guide under its leading element (the art thumb of a top-level row, the label of a nested row). The guide runs down through every row of that parent's subtree and stops at the bottom of its last row.
- A guide under a top-level parent (artist, series, playlist) paints in a neutral tone. A guide under a nested parent (album, season) paints in the accent tone, so the open inner branch stands out.
- The nested rows move right so that their text sits to the right of every guide that passes through them. No guide crosses a row's text.
- A closed parent, or an open parent whose children have not loaded, shows no guide. Flat lists (Home, Emby, Audiobooks, Podcasts, Feeds, Shows New and Upcoming) show no guides.
- Nothing else changes: the row heights, the dividers, the fills, the selection and hover looks, the hint, and the press and double-click behaviour stay as they are.

Decisions made with the user (2026-11-20):

- Only the GUI trees. The TUI trees are out of scope.
- Guides only. The mockup's group framing (the raised fill on the open parent, the band behind its children, the accent label on the open nested parent, the closing divider) and its row highlight are not part of this change.
- Accent rule: guides under a nested parent are accent; guides under a top-level parent are neutral. The Playlists tree has only top-level parents, so its guide is neutral.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: adds the requirement that GUI tree rows show indent guides (which rows start, carry, and end a guide; the two guide tones; text clear of guides; no guide on closed nodes or flat lists).

## Impact

- `crates/mbv-gui/src/window/components/`: a new shared guide part (`tree_guide.rs`); `library_item_row.rs` (the top-level parent's guide start); `library_list.rs` (album and track rows); `library_shows_list.rs` (season and episode rows); `playlists_panel.rs` (the open playlist's node).
- `crates/mbv-gui/src/window/library/list.rs` and `shows_list.rs`: the parent row models carry whether the node shows child rows.
- `crates/mbv-gui/assets/app.css` and the committed `app.generated.css`: two new canvas tokens for the guide tones.
- Docs: `docs/architecture/gui-client.md` (the tree-row insets and the guides).
- The in-progress change `gui-playlists-in-queue-column` moves the Playlists tree into the Queue column without a redesign. This change touches only the tree's row functions, which that change moves as they are; whichever lands second rebases its `playlists_panel.rs` edits.
- The TUI Client, the Client core, the ctrl protocol, persistence, and dependencies do not change.
