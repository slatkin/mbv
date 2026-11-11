# Hero look audit and refactor plan

Audit date: 2026-11-11, at `e59e6d822` (branch `add-gui-library-panel`).
Rule: `docs/invariants/30-shared-ui-is-never-bent-to-fit-a-consumer.md`.
Target: one Hero look for every Library destination (design D15): a header
band (kicker, title, facts), then a detail block (art, text, actions,
overview), then a list band.

All paths below are relative to `crates/mbv-gui/ui/src/lib/components/`
unless they start with `src/`, `crates/`, or `docs/`.

## Findings

### Hero, per destination

The reference look is `LibraryHeroFrame.svelte`: root padding `28px 32px`,
band gap `28px` (`:85-86`); header band with a `20px` bottom pad and a
border (`:90-95`); kicker 12 px mono (`:97-102`); title `h1` 32 px/600
(`:104-111`); facts 13 px mono, painted only when non-empty (`:112-119`);
block gap `32px` beside the art, `18px` stacked (`:126-127`); text column
gap `18px` (`:140`); block name `h2` 24 px (`:154-160`); line gap `6px`
(`:144`).

| Destination | Painter | Header band | Kicker | Header title | Header facts | Block text | Art arm | Action row | Overview | List band |
|---|---|---|---|---|---|---|---|---|---|---|
| Music | Frame (`LibraryHero.svelte:206`) | Yes, plus Shuffle artist at right (`:210`, `:93-112`) | `artist · COLLECTIONS` (`$lib/library/hero.ts:24-28`) | Parent: artist (`:209`) | Album count · track total (`hero.ts:39-44`) | Eyebrow year, `h2` album, track total (`:211-216`) | Square, placeholder `[Album art · Emby Primary]` (`:217-220`) | Play, Play next, Add, More; **own copy of the buttons** (`:116-170`) | None | Tracklist (`:223`) |
| Shows, series | Frame (`LibraryShowsHero.svelte:110`) | Yes | `series` (`:114`) | Parent: series (`:115`) | All meta rows joined ` · ` (`:116`) | `h2` season name, episode count (`:118-123`) | Policy arm, default placeholder | **None** (no `actions`) | Series overview | Season pills + episode table (`:131-297`) |
| Shows, Latest episode | Frame | Yes | `episode` (`:114`) | Item: episode title | All meta rows joined, **duration row included, role lost** (`:116`) | **Empty** (no season, no facts); only the overview paints | Policy arm | **None** | Episode overview | **None** |
| Movies / Emby | **`LibraryHeroSummary`** (`LibraryEmbyHero.svelte:43`) | **No** | **None** | **No header**; `h2` 26 px with `-0.01em` (`LibraryHeroSummary.svelte:77-84`) | **None**; meta rows sit in the block, gap `4px` (`:85`) | Title + meta rows + overview | Policy arm; side gap **28px** (`:59`) | Play, Play next, Add (`:111-115`) | Yes | None |
| Home | **`LibraryHeroSummary`** (`LibraryHomeHero.svelte:42`) | **No** | **None** | As Movies | As Movies | As Movies | As Movies | Play, Play next, Add | Yes | None |
| Podcasts | Frame (`LibraryPodcastsHero.svelte:44`) | Yes | `episode` (`:47`) | Item: episode title | **Empty**, so the facts line does not paint (`:47`; frame `:112`) | Meta rows only, no `h2` (`:50`) | Square (policy) | Play, Add (`:58-62`) | Description | None |
| Audiobooks | Frame (`LibraryAudiobooksHero.svelte:92`) | Yes | `audiobook` (`:95`) | Item: book title | **Empty** (`:95`) | Meta rows only (`:98`) | Portrait | Play, Add (`:106-110`) | Description | Chapter table (`:113-224`) |
| Feeds | Frame (`LibraryFeedsHero.svelte:45`) | Yes | `entry` (`:48`) | Item: entry title | **Empty** (`:48`) | One duration row (`:51`) | Policy arm, art always `null` (`:54`) | Play, Add (`:58-62`) | None | None |

### Hero, differences in look not in the table

1. **Two root boxes.** Movies and Home wrap the summary in their own root:
   padding `28px 32px` but gap **`20px`** (`LibraryEmbyHero.svelte:42`,
   `LibraryHomeHero.svelte:41`), against the frame's `28px`.
2. **Two text columns.** The summary's text gap is **`14px`**
   (`LibraryHeroSummary.svelte:74`) against the frame's `18px`; its meta
   line gap is **`4px`** (`:85`) against `6px`; its title is **26 px with
   letter-spacing** against the frame's block name, 24 px without it.
3. **Two block text shapes inside the frame.** Music and Shows fill
   `eyebrow`/`name`/`facts`; Podcasts, Audiobooks, and Feeds fill `meta`
   (`LibraryHeroFrame.svelte:43-44`, `:143-178`). With `meta`, the block
   has no `h2`, so the art sits beside small mono lines only. The block
   looks different from Music's album block.
4. **Header title means two things.** Music and Shows (series) put the
   *parent* in the header and the picked child in the block. Podcasts,
   Audiobooks, Feeds, and Shows (Latest) put the *item itself* in the
   header and leave the block without a name.
5. **Header height differs.** Podcasts, Audiobooks, and Feeds pass
   `facts: ''`, so the header loses its third line (frame `:112`). Music
   and Shows always have one.
6. **Meta rows go to two places.** Shows joins all meta rows into the
   header facts (`LibraryShowsHero.svelte:116`). Podcasts, Audiobooks, and
   Feeds put them in the block (`meta`). For a Shows Latest episode, the
   duration row loses its tabular role in the header.
7. **Kicker words.** `artist · COLLECTIONS` (mixed case, collections in
   capitals), `series`, `episode` (both Shows Latest and Podcasts),
   `audiobook`, `entry`. Movies and Home have none.
8. **Art placeholder text.** Music has its own text
   (`LibraryHero.svelte:220`); the others take the default
   (`LibraryHeroArt.svelte:35`).
9. **Action rows.** Music: four controls. Movies, Home: three. Podcasts,
   Audiobooks, Feeds: two. Shows series and Shows Latest: **none**. The
   Shows Latest episode is the only single-item Hero with no Play button,
   but the core already has `showsPlay`, `showsPlayNext`, and `showsQueue`
   (`LibraryShowsHero.svelte:74-94`).
10. **Root attribute clash.** Home's root carries `data-library-hero`
    (`LibraryHomeHero.svelte:41`), the same hook as Music's frame root
    (`LibraryHero.svelte:208`). The `[data-library-hero]` queries in
    `LibraryPanel.test.ts:134,157` would match a Home Hero too.
11. **Stale comments.** `LibraryHeroSummary.svelte:6-8` says Shows and
    Audiobooks paint no action (Audiobooks does now).
    `LibraryPodcastsHero.svelte:3` and `LibraryFeedsHero.svelte:3` say "the
    Emby Hero look", which is the summary layout they no longer use.

### List band (Workspace), per destination

| Aspect | Music tracklist (`LibraryTracklist.svelte`) | Shows episodes (`LibraryShowsHero.svelte`) | Audiobooks chapters (`LibraryAudiobooksHero.svelte`) |
|---|---|---|---|
| Header row | `#`, TITLE, (blank), TIME (`:87-101`) | Same, own copy (`:170-184`) | **No `#` column**: TITLE, (blank), TIME (`:130-143`) |
| Row grid | `40px 1fr 88px 64px`, 34 px (`:105-109`) | Same, own copy (`:188-192`) | `1fr 88px 64px` (`:147-150`) |
| Hover | CSS `group-hover` (`:121-124`, `:144`) | Store `$showsWorkspace.hover` + `display` (`:203-204`, `:235`) | Store `$audiobooksWorkspace.hover` (`:156-157`, `:191`) |
| Hover controls | Play replaces the number; Play next, Add (`:122-176`) | Play next, Add; number stays, **no play control** (`:238-269`) | Play only, in the action cell (`:194-209`) |
| Double-click | **Does not play** (`:15`) | Plays (`:223`) | Plays (`:170`) |
| Playing mark | Accent glyph beside the title (`:133-136`) | None; progress run under the row (`:279-291`) | Accent glyph (`:173-179`) |
| Loading line | **None**: header paints over an empty list | `Loading episodes…`, 13 px mono, inline (`:161-167`) | `Loading chapters…`, same, own copy (`:122-128`) |
| Pills above | None | Season pills, **28 px / 13 px, no focus ring**, own markup (`:132-160`) | None |
| Right-click | Yes (`:114`) | **No** | **No** |

The top pill rows paint through `LibraryPillRow.svelte` at **32 px / 14
px with a focus outline** (`:153-178`), so season pills are a second pill
look.

### List column, per destination

| Aspect | Music | Home | Shows | Movies/Emby | Audiobooks | Podcasts | Feeds |
|---|---|---|---|---|---|---|---|
| `LibraryListHeader` | Yes (`LibraryList.svelte:238`) | Yes (`LibraryHomeList.svelte:98`) | Yes (`LibraryShowsList.svelte:184`) | Yes (`LibraryEmbyList.svelte:106`) | Yes (`LibraryAudiobooksList.svelte:103`) | Yes (`LibraryPodcastsList.svelte:92`) | **No**: bare filter field, padding `8px 16px 0` (`LibraryFeedsList.svelte:72-80`) |
| Scroller | **Own `<ol>`**, padding `0 0 0 8px` (`LibraryList.svelte:263-272`) | `LibraryVirtualList`, default `8px` | VirtualList `8px 0 8px 8px` | Same as Shows | Same as Shows | Default | Default |
| Item row padding | tree rows | `0 10px` (`:144`) | `0 12px` (`:328`) | `0 12px` (`:162`) | `0 12px` (`:158`) | `0 10px` (`:133`) | `0 10px` (`:104`) |
| Played dim | n/a | Yes | Yes | **No** (title always `text`, `LibraryEmbyList.svelte:181`) | **No** (`LibraryAudiobooksList.svelte:177`) | Yes | Yes |
| Progress run | n/a | Yes (`:201-212`) | Yes | No | No | Yes (`:176-185`) | Yes (`:143-155`) |
| Right-click | Yes | Yes | Yes | Yes | Yes | Yes | **No** |
| Empty line | `No artists` | `Nothing to play` | **None** | `No items` | `No audiobooks` | `No episodes` | **None** |
| Failure line | Core text | **None** | Upcoming only | `…this library.` | `…this catalog.` | `…this library.` | `…this destination.` |
| Letter heading | 12 px mono (`LibraryList.svelte:277-286`) | Section heading 14 px/600 (`:128-138`) | 12 px mono (`:220-230`) | 12 px mono (`:146-156`) | 12 px mono (`:142-152`) | n/a | n/a |

## Duplicates

Each line names the copies and the shared piece that must own them.

1. **Hero buttons.** `LibraryHero.svelte:116-170` copies
   `LibraryHeroActions.svelte:26-85` line for line. The cause is a bug in
   the shared piece: with `attr=""` it writes `data-library--hero-play`
   (`LibraryHeroActions.svelte:21-23`), while the frame and the art handle
   the empty prefix (`LibraryHeroFrame.svelte:77-80`,
   `LibraryHeroArt.svelte:39-41`). Owner: `LibraryHeroActions`.
2. **Hero root and block.** `LibraryHeroSummary.svelte` repeats the frame's
   block (art arm, text column, action row, overview) with other numbers,
   and Movies and Home repeat the frame's root box. Owner:
   `LibraryHeroFrame`.
3. **Workspace table header row.** `LibraryTracklist.svelte:87-101`,
   `LibraryShowsHero.svelte:170-184`, `LibraryAudiobooksHero.svelte:130-143`.
   Owner: a new `LibraryWorkspaceTable` (header row, grid, row shell with
   the cursor fill).
4. **Workspace row hover controls.** The 28 px icon buttons in
   `LibraryTracklist.svelte:143-177`, `LibraryShowsHero.svelte:231-271`,
   `LibraryAudiobooksHero.svelte:187-211`, with two hover mechanisms. Owner:
   a new `LibraryRowActions`, inside `LibraryWorkspaceTable`.
5. **Playing glyph.** `LibraryTracklist.svelte:133-136`,
   `LibraryAudiobooksHero.svelte:173-179`. Owner: `LibraryWorkspaceTable`.
6. **Workspace loading line.** `LibraryShowsHero.svelte:161-167`,
   `LibraryAudiobooksHero.svelte:122-128` (Music has none). Owner:
   `LibraryWorkspaceTable`, which paints its loading line when the rows
   have not landed.
7. **Pill button.** Season pills (`LibraryShowsHero.svelte:139-158`) copy
   the pill button of `LibraryPillRow.svelte:159-182` at other sizes. Owner:
   a `LibraryPill` extracted from `LibraryPillRow`; the row and the season
   pills both paint it.
8. **Resume progress run.** `LibraryShowsHero.svelte:279-291`,
   `LibraryHomeList.svelte:201-212`, `LibraryPodcastsList.svelte:176-185`,
   `LibraryFeedsList.svelte:143-155`. Owner: a new `LibraryProgressRun`.
9. **Flat item row** (title, subtitle, date gutter, selected weight, fills).
   `LibraryHomeList.svelte:139-213`, `LibraryShowsList.svelte:324-370`,
   `LibraryEmbyList.svelte:159-205`, `LibraryAudiobooksList.svelte:154-200`,
   `LibraryPodcastsList.svelte:129-187`, `LibraryFeedsList.svelte:99-156`.
   Owner: a new `LibraryItemRow` (padding one value; Home's card art as
   carried data, not a flag).
10. **Row fill rule** (cursor fill, else raised fill on the selected row).
    `rowBackground` in Feeds `:58-63`, Podcasts `:75-80`, Audiobooks
    `:72-77`, Emby `:94-99`, Home `:79-84`, Shows `:108-113`. Owner: one
    function in `$lib/library/list.ts`, used by `LibraryItemRow`.
11. **Letter heading row.** `LibraryList.svelte:277-286`,
    `LibraryShowsList.svelte:220-230`, `LibraryEmbyList.svelte:146-156`,
    `LibraryAudiobooksList.svelte:142-152`. Owner: a new
    `LibraryLetterHeading`.
12. **List header.** Feeds paints its own filter block
    (`LibraryFeedsList.svelte:72-80`). Owner: `LibraryListHeader`.
13. **Virtual scroller.** Music's list paints its own windowed `<ol>`
    (`LibraryList.svelte:263-275`). Owner: `LibraryVirtualList`, if it can
    take the tree's three row heights (check in step 13).
14. **Attribute helper `own()`.** Four copies with different empty-prefix
    rules (`LibraryHeroFrame.svelte:77`, `LibraryHeroArt.svelte:39`,
    `LibraryHeroActions.svelte:21`, `LibraryHeroSummary.svelte:52`). Owner:
    one function in `$lib/library/hero.ts`.

## Escape hatches

Props on a shared piece that let a consumer drop or swap a band (invariant
30 rule 2):

| Piece | Prop | Effect | Users |
|---|---|---|---|
| `LibraryHeroFrame` | `actions?` (`:55`, `:180`) | Drops the action row | Shows (series and Latest) |
| `LibraryHeroFrame` | `children?` (`:57`, `:198`) | Drops the list band | Podcasts, Feeds, Shows Latest (and Movies, Home after they move) |
| `LibraryHeroFrame` | `block.eyebrow/name/facts/meta/overview`, all optional (`:38-46`, `:143-194`) | Two block text shapes (name+facts, or meta rows); an empty block | Podcasts, Audiobooks, Feeds (meta); Shows Latest (empty) |
| `LibraryHeroFrame` | `header.facts` empty hides the line (`:112`) | Header loses a line | Podcasts, Audiobooks, Feeds |
| `LibraryHeroFrame` | `headerAction?` (`:36`, `:121`) | Adds a control at the header's right | Music only |
| `LibraryHeroFrame` / `LibraryHeroArt` | `artPlaceholder?` / `placeholder?` (`:53`; `:28`, `:35`) | Changes the empty art text | Music only |
| `LibraryHeroActions` | `onplay?`, `onplaynext?`, `onqueue?` (`:11-16`); Play next and Add are nested under Play (`:26`, `:46`, `:66`) | Drops buttons; no Play drops all | Podcasts, Audiobooks, Feeds (no Play next) |
| `LibraryHeroSummary` | The whole component; `overview?`, `art?`, `onplay?` (`:25`, `:28`, `:39-43`) | A second Hero layout | Movies, Home |
| `LibraryVirtualList` | `padding?` (`:27`, `:40`) | Changes the list inset | Shows, Emby, Audiobooks (rail lists) |

Cleared (hooks or behavior only, no look change): `LibraryListHeader`
`titleAttr?`/`labelAttr?`, `LibraryPillRow` `pillAttr?`,
`LibraryLetterRail` `jump?`, `QueueRowMenu` `left?`/`right?` (position
only).

Notes on the hatches. The action set per destination comes from real
command gaps (Podcasts and Feeds have no play-next command, design D12
rows 10.1 and 11.1), so the buttons are data, not look. The plan keeps
`LibraryHeroActions` handlers optional only for that reason and removes
the nesting under Play. The list band and the block shapes are real
data-fit questions; they are open questions 1, 5, and 6 below.

## Header content per destination

Source of truth: `HeroContentData` (`crates/mbv-components/src/library_panel/hero.rs:26-30`):
`facts.title`, `facts.meta_rows` with `duration_row` and `progress_row`,
`facts.links`, `facts.artwork`, `overview`, `credits`. The TUI has no
header/block split and no kicker: it paints title, meta rows, overview,
and credits as one hero. The GUI header is therefore a mapping. The plan
uses one rule for all single-item Heroes (open question 1):

- header kicker: the item's kind word (core must carry it where marked);
- header title: `facts.title`;
- header facts: the meta rows with no role, joined ` · `;
- block: the duration and progress rows, the actions, and the overview.

Music and Shows (series) keep their two levels: the parent in the header,
the picked child in the block.

| Destination | Producer | Kicker | Title | Header facts | Block | Core must add |
|---|---|---|---|---|---|---|
| Music | GUI's own `GuiMusicHero` (`src/app/gui_library/projection.rs:361-382`), not `HeroContentData` | `artist · COLLECTIONS` (keep) | Artist | Album count, artist track total | Year, album, album track total | **Flag:** the TUI artist hero also carries the year span (`crates/mbv-components/src/music_content/workspace.rs:89-96`); `GuiMusicHero` does not. Add only if the user wants it. |
| Shows, series | `hero_content_series_with_episode` (`hero.rs:363-441`) | `series` | Series | Genre · years · N Seasons · network (all rows, no roles in this mode) | Season name, episode count | None |
| Shows, Latest | `hero_content_emby` on the episode (`src/app/gui_library/shows_projection/hero.rs:86`) | `episode` | Episode | Release date | Duration | **Flag:** no series name. `emby_hero_meta_rows_plain` gives an episode only the date and duration (`crates/mbv-render/src/components/hero_model.rs:38-44`); the TUI has the same gap. |
| Movies / Emby | `hero_content_emby` (`hero.rs:261-344`) | **Needs the item kind** (`movie`, `video`, …) | Item | Release date · `IMDb` · genres | Duration | **Flag:** `GuiEmbyHero` has no kind (`library-types.ts:658-668`). **Flag:** `credits` (director, cast) and `links` (IMDb URL) are dropped in `src/app/gui_library/emby_projection.rs:466-480`; the GUI paints the word `IMDb` with no link. |
| Home | `hero_content_queue` (`src/app/gui_library/home.rs:347`) | **Needs the item kind** (open question 2) | Item | Per kind (as the rows of that kind's destination) | Duration, progress | **Flag:** `GuiHomeHero` has no kind (`library-types-home.ts:75-92`); `GuiHomeRowKind` is only `collection`/`media`. Credits dropped as for Movies. |
| Podcasts | `hero_content_abs_episode` (`hero.rs:592-628`) | `episode` | Episode | Show · publish date | Duration | **Flag (open question 3):** the show name is a positional meta row, not a field. |
| Audiobooks | `hero_content_abs_book` plus catalog rows (`src/app/gui_library/audiobooks_projection.rs:187-240`) | `audiobook` | Book | Author · `Read by …` · year (· `Finished`) | Duration, progress | None. Note: `Finished` has no role, so it lands in the header facts under this rule. |
| Feeds | `hero_content_feed` (`hero.rs:631-655`) | `entry` | Entry | **Empty**: the producer has only the duration | Duration | **Flag (open question 4):** feed name and date are on the list row (`GuiFeedsRow`), not in the Hero. |

## Decisions

The user answered the open questions below on 2026-11-11. These answers are final and replace the recommendations.

- **Rule: the header's big title is always the largest unit's name.** It is the podcast, the feed, the series, or the artist. The block beside the art holds the selected smaller unit (episode, entry, season, album). A movie or a book is its own largest unit. This answers Q3 (yes) and Q4 (the feed's name is the title; the entry goes in the block).
- **Q1: yes.** Plain facts sit on one line under the big title; only running time and progress sit beside the art.
- **Q2: closed.** Home is out of scope. It is redone after the GUI migration and stays on `LibraryHeroSummary`. Drop Home from every step.
- **Q5: yes, except Feeds.** Feeds is a collection like Music and TV. Its list band holds the selected feed's entries, newest first, with the selected entry marked. Movies, Podcasts, and Shows Latest have no list.
- **Q6: yes.** A TV series shows Play, Play next, and Add to queue for the selected season (new core command).
- **Q7: yes.** The window shows a movie's director, cast, and a real IMDb link. TUI parity is the floor and is never a question.
- **Q8: yes.** Season buttons match the top pills (size, text, focus ring).
- **Q9: yes.** Audiobook chapters get a number column.
- **Q10: yes.** A double-click on a track plays it.

## Open questions (answered; see Decisions)

Each is about what you will see. Each blocks only the steps named.

1. **Plain facts on one line under the big title.** Should a Movie, Home
   item, Podcast episode, Audiobook, Feed entry, and Shows Latest episode
   show its plain facts (author, show, date, genre, narrator) on one line
   under the big title at the top, as a TV series does now, with only the
   running time and progress beside the artwork? Recommended: yes. If no,
   the series header changes instead, so the facts move beside the
   poster. (Steps 3, 6, 7.)
2. **Small capital line on Home.** Should the small line above a Home
   item's title name the kind of item (MOVIE, EPISODE, AUDIOBOOK,
   PODCAST)? If no, it names the Home section the row sits in (for example
   CONTINUE WATCHING). (Steps 2, 5.)
3. **Podcast name as the big title.** Should the Podcasts header show the
   podcast's name as the big title, with the episode name below it beside
   the cover, as Music shows the artist above the album? If yes, the core
   must carry the show name as its own fact. (Step 7.)
4. **Feeds header line.** A feed entry has no fact for the line under its
   title, so the Feeds header is one line shorter than the others. Should
   it show the feed's name and the entry date, which the list row already
   shows? (Step 7.)
5. **Heroes with nothing below the detail block.** Movies, Home, Podcasts,
   Feeds, and Shows Latest have no list under the detail block, so their
   Hero ends after the overview (and Movies after the cast, if question 7
   is yes). Is that right? If no, say what the list should hold. (Step 4
   onward.)
6. **Play on a TV series.** Music shows Play, Play next, and Add to queue
   beside the album art. A TV series shows no buttons beside its poster.
   Should it show them, for the selected season? (This needs a new core
   command.) (Step 8.)
7. **Cast on a movie.** The terminal shows a movie's director and cast
   under the overview, and the IMDb line is a link. Should the window show
   them too? (Step 2.)
8. **Season buttons size.** Should the season buttons in a TV series Hero
   be the same size as the pills at the top of the Library (a little
   taller, larger text, with the keyboard focus ring)? Recommended: yes.
   (Step 9.)
9. **Number column for chapters.** Should audiobook chapters show a number
   column, as tracks and episodes do? (Step 10.)
10. **Double-click on a track.** Should a double-click on a track play it,
    as a double-click on an episode or a chapter does? Today it does not.
    (Step 10.)

## Refactor plan

Rules for every step: consumers change to fit the shared piece; the
shared piece is not bent. No scripts, no lint suppression, no new test
unless it owns a named contract. Run `cargo fmt` for Rust changes. Keep
every file under 800 lines (`LibraryShowsHero.svelte` 300 and
`LibraryAudiobooksHero.svelte` 226 lines shrink). Update docs in the same
commit as the code (invariant 30 "How the code maintains it today", design
D15, `docs/architecture/gui-client.md` Hero list). Each step is one
commit.

Visual check (V) for a step: the user runs `make dev` (live; agents ask
the user first and never run it on their own), opens the destination and
Music in Wide (window over 880 px) and Narrow (480 to 880 px), and
compares, side by side: header band height and border, kicker, title size,
art box size and position, gap between art and text, action row, overview,
and list band. Use a landscape item and a portrait or square item where
the destination has both. Svelte-check and vitest passing is not a visual
check.

1. **Fix the shared action buttons' attribute prefix; Music uses them.**
   Files: `LibraryHeroActions.svelte` (empty `attr` gives
   `data-library-hero-*`, the frame's rule), `LibraryHero.svelte` (actions
   snippet renders `LibraryHeroActions` plus its own More control),
   `$lib/library/hero.ts` (one `own()` helper the four pieces import).
   Contract kept: Music's Hero buttons send the album commands under the
   same hooks; `LibraryHero.test.ts` passes unedited. V: Music unchanged.
2. **Core: carry the facts the header needs.** Change source-of-truth
   types before callers. Files: `src/app/gui_library/emby_projection.rs`,
   `src/app/gui_library/home.rs`, their bridge DTOs in
   `crates/mbv-gui/src/bridge/frame*`, and `library-types*.ts`. Add the
   item kind word to `GuiEmbyHero` and `GuiHomeHero` (per question 2);
   add `credits` and the IMDb link only if question 7 is yes. Contract: the
   Hero kind is the producer's item kind; extend the existing Emby and
   Home projection tests with one assertion each (no new test). Gate:
   `cargo nextest run -p mbv`.
3. **One header mapping in `$lib/library/hero.ts`** (after question 1).
   A function from `{title, metaRows, durationRow, progressRow}` and a
   kicker to the frame's `header` and `block`, replacing
   `heroMetaRows`' role split for headers. Contract: rows with no role go
   to the header facts, duration and progress rows to the block. Test:
   one `hero.test.ts` with `#[case]`-style table rows only for the
   distinct outcomes (no role rows; duration only; duration and progress).
4. **Movies onto the frame.** File: `LibraryEmbyHero.svelte`: paint
   `LibraryHeroFrame` with `attr="emby"`, `root="data-library-emby-hero"`,
   the step 3 mapping, and `LibraryHeroActions`. Test:
   `LibraryEmbyHero.test.ts` keeps its command and art tests; the summary
   hook moves to the block hook; add the header kicker-and-title assertion
   the invariant names (one per frame consumer). V: Movies next to Music,
   a landscape movie and a portrait one.
5. **Home onto the frame.** File: `LibraryHomeHero.svelte`, same as step
   4, with root `data-library-home-hero` (removes the clash with Music's
   `data-library-hero`). Test: `LibraryHomeHero.test.ts` as step 4;
   `LibraryPanel.test.ts` passes unedited. V: Home next to Music, one
   movie, one episode, one audiobook row.
6. **Delete `LibraryHeroSummary.svelte`.** It has no users after steps 4
   and 5. Fix the comment in `LibraryHeroActions.svelte:5`. Contract: none;
   svelte-check and the UI suite stay green. Update invariant 30 "Where it
   currently fails" (the Movies and Home bullet goes).
7. **Podcasts, Audiobooks, Feeds, Shows Latest take the step 3 mapping**
   (and the answers to questions 3 and 4). Files: the four Hero
   components; the core projections only if question 3 or 4 is yes. Test:
   extend each consumer's existing header test to the facts line. Fix the
   stale "Emby Hero look" comments. V: each next to Music; the header
   bands line up in height.
8. **Shows action row.** File: `LibraryShowsHero.svelte`. Latest episode:
   `LibraryHeroActions` with `showsPlay`, `showsPlayNext`, `showsQueue`
   (commands exist). Series: only if question 6 is yes, after a core
   command. Test: one vitest that the Latest Play press sends `showsPlay`
   with the episode id. V: Shows Latest next to Podcasts.
9. **One pill button.** Extract `LibraryPill.svelte` from
   `LibraryPillRow.svelte`; the row and the season pills paint it (after
   question 8). Test: the five pill-row vitests and the Shows Workspace
   vitest pass unedited. V: Shows series Hero, the season pills next to the
   top pill row.
10. **One Workspace table.** New `LibraryWorkspaceTable.svelte` and
    `LibraryRowActions.svelte`: header row, row grid, cursor fill, hover
    controls with one hover mechanism, playing glyph, duration cell,
    progress run, loading line. `LibraryTracklist.svelte`, the Shows
    episodes, and the Audiobooks chapters adopt it; columns and
    double-click follow questions 9 and 10. Music gains a loading line
    while its tracks have not landed. Test: the existing tracklist, Shows
    Workspace, and Audiobooks chapter vitests pass with only hook renames;
    one new vitest for the loading line (contract: a Workspace whose rows
    have not landed paints the loading line, not an empty header). V:
    Music, Shows, Audiobooks lists side by side.
11. **One flat item row.** New `LibraryItemRow.svelte`,
    `LibraryProgressRun.svelte`, `LibraryLetterHeading.svelte`, and the
    row fill function in `$lib/library/list.ts`. Home, Shows (flat),
    Emby, Audiobooks, Podcasts, and Feeds lists adopt them; row padding
    takes one value (`0 12px`); Emby and Audiobooks rows dim when played if the row
    carries the state, else leave as is and note it. Test: the list vitests
    pass with hook renames only. V: each list column next to Home's.
12. **Feeds list header and empty line.** File: `LibraryFeedsList.svelte`:
    `LibraryListHeader` (title `Feeds`, label the selected pill) and a
    `No entries` notice. Test: one vitest for the header (title and label).
    Right-click on Feeds rows stays out of scope (no core row-menu path for
    feeds; record it in design D15 as a gap). V: Feeds list next to
    Podcasts.
13. **Music list scroller (check first).** Read `LibraryVirtualList` and
    `LibraryList.svelte`: adopt the shared scroller only if it takes the
    tree's three row heights with no new prop. If it needs a new prop, stop
    and ask; do not add one. Test: `LibraryList.test.ts` passes unedited.
    V: Music list scroll and rail jump.
14. **Gates and the final look pass.** `pnpm -C crates/mbv-gui/ui test`,
    svelte-check, `make ui-lint`, `cargo nextest run -p mbv -p mbv-gui`,
    `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
    --all -- --check`, and `make check-code-file-lines` just before push.
    Then the user's final V pass: all seven destinations in a row, Wide
    and Narrow, against this audit's Findings tables. Report done only
    after that pass (invariant 30 rule 4).

## Other GUI screens

No real findings. `QueueRowMenu` is the one menu look for the Queue panel
(`QueuePanel.svelte:592`), the Library row menu (`LibraryPanel.svelte:416`),
and the Hero's More actions (`LibraryHero.svelte:196`); its optional props
set position only. `TrackChips`, `QueueHeader`, and `Panel` each have one
user. There is no Settings screen in the GUI front end.
