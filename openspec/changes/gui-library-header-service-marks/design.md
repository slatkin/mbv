# Design

## Context

See proposal.md for the why. The current state:

- Every GUI Library destination paints one shared header, `crates/mbv-gui/src/window/components/library_list_header.rs`. Its title line holds a `title: String` span (`data-library-list-title`), then the collection dropdown, the Shows view dropdown, or a selection label. The `/` filter field is under the line.
- Seven callers pass the title: `library_list.rs` (Music, "Artists"), `library_emby_list.rs` (the library name), `library_shows_list.rs` ("Series" or the view label, with the `data-library-shows-list-title` hook), `library_audiobooks_list.rs` ("Audiobooks"), `library_podcasts_list.rs` ("Podcasts"), `library_feeds_list.rs` ("Feeds"), and `library_home_list.rs` ("Home"). The Playlists screen (`playlists_panel.rs`) does not use this header.
- `window/brand_icons.rs` already paints simple-icons marks for the playback-target chooser (`TargetBrand`: Emby, Cast, mbvd). It sets the colour on the svg's computed `color` and fills the path with `currentColor`, because usvg cannot resolve `var()` in a `fill` attribute.
- `window/icons.rs` already has the Lucide `Rss` icon.
- The Feeds collection selector (`GuiCollections`, target `Feeds`) keys each entry by its subscription URL and carries `all` and each entry's `checked` flag. `GuiFeedsList::from_projection` (`bridge/frame/library/feeds.rs`) copies it into the frame.
- `crates/mbv-feed/src/lib.rs` has a private `url_authority_and_path` that splits a URL into its host and its path with query.

## Goals / Non-Goals

**Goals:**

- The Feed platform rule lives in `mbv-feed` as one pure function of the URL, so the core and any later Client read the same answer.
- The "every checked subscription shares one platform" rule lives in the core, next to the Feeds projection, as one pure function.
- One painter paints every Service mark.

**Non-Goals:**

- No change to the subscribe step. `normalize_feed_url` still accepts only YouTube `channel_id=` feeds.
- No mark in the tab bar, in the Feeds dropdown rows, or in a row.
- No detection by a network request or by feed content. A Substack on a custom domain, or an Audiobookshelf feed behind a reverse proxy, shows the RSS mark.
- No TUI change.

## Decisions

### D1. `FeedPlatform` in `mbv-feed`, read from the URL alone

A new module `crates/mbv-feed/src/platform.rs` holds:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedPlatform {
    YouTube, SoundCloud, Spreaker, Audioboom, Patreon,
    Substack, Spotify, Odysee, PeerTube, InternetArchive,
}

impl FeedPlatform {
    /// The platform `url` shows, or `None` for a generic feed.
    pub fn of_url(url: &str) -> Option<Self>;
}
```

`lib.rs` adds `mod platform;` and `pub use self::platform::FeedPlatform;`, and makes `url_authority_and_path` `pub(crate)` so that the new module reuses it.

The host rule: lowercase the host. A host matches a domain when it equals the domain or ends with `.` plus the domain. So `feeds.soundcloud.com` matches `soundcloud.com`, and `notyoutube.com` does not match `youtube.com`.

| Platform | Domain | Feed form, checked in the service's help pages on 2026-11-20 |
|---|---|---|
| YouTube | `youtube.com` | `www.youtube.com/feeds/videos.xml?channel_id=…` |
| SoundCloud | `soundcloud.com` | `feeds.soundcloud.com/users/soundcloud:users:<id>/sounds.rss` |
| Spreaker | `spreaker.com` | `www.spreaker.com/show/<id>/episodes/feed` |
| Audioboom | `audioboom.com` | `audioboom.com/channels/<id>.rss` |
| Patreon | `patreon.com` | `www.patreon.com/rss/<creator>?auth=…` |
| Substack | `substack.com` | `<name>.substack.com/feed` |
| Spotify | `anchor.fm` | `anchor.fm/s/<id>/podcast/rss` (Spotify for Creators) |
| Odysee | `odysee.com` | `odysee.com/$/rss/@<channel>` |
| Internet Archive | `archive.org` | `archive.org/services/collection-rss.php?collection=<id>` |

The PeerTube rule: every instance has its own host, so the rule reads the path and the query (PeerTube `server/core/controllers/feeds/` and `middlewares/validators/feeds.ts`, `develop`). A URL is a PeerTube feed when its path is `/feeds/videos.xml`, `/feeds/videos.rss`, `/feeds/videos.atom`, `/feeds/videos.json`, or `/feeds/podcast/videos.xml`, and its query has a parameter named `accountId`, `accountName`, `videoChannelId`, `videoChannelName`, or `playlistId`. The host rules apply first, so a YouTube `/feeds/videos.xml?channel_id=` URL is YouTube. An instance-wide PeerTube feed with no filter parameter is generic.

Alternative: put the platform on `FeedSubscription` in `mbv-config`. Rejected: the URL alone decides it, so a stored copy can only go stale.

### D2. The checked-platform rule in the core

`src/app/gui_library/feeds_projection.rs` gets:

```rust
/// The Feed platform every checked subscription shares, or `None` for All,
/// for an empty check set, for a mix, or for a generic subscription.
#[must_use]
pub fn gui_feeds_platform(collections: &GuiCollections) -> Option<FeedPlatform>;
```

It returns `None` when `collections.all` is true or no entry is checked. Otherwise it maps each checked entry's `key` (the subscription URL) through `FeedPlatform::of_url`. It returns the platform when every result is the same `Some(platform)`, and `None` in every other case. `src/app/gui_library.rs`, `src/app.rs`, and `src/lib.rs` re-export it the same way they re-export `GuiFeedsProjection`. `src/lib.rs` also re-exports `mbv_feed::FeedPlatform`, so `mbv-gui` gets no new direct dependency.

Alternative: a field on the projection. Rejected: the Feeds projection is the shared `GuiEntryListProjection`, and Podcasts would carry a field that it never fills.

### D3. The frame carries the platform

`GuiFeedsList` (`bridge/frame/library/feeds.rs`) gets `pub platform: Option<FeedPlatform>`, set in `from_projection` from `mbv::gui_feeds_platform(&projection.collections)`. The list part already changes whenever its collections change, so the mark follows a selection change with no new change-detection code.

### D4. `ServiceMark` and one painter

A new module `crates/mbv-gui/src/window/service_mark.rs` (registered in `window.rs`) holds:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceMark {
    Emby, Audiobookshelf, Rss, Feed(FeedPlatform),
}

impl ServiceMark {
    /// The Feeds tab's mark: the shared platform, else RSS.
    pub fn feeds(platform: Option<FeedPlatform>) -> Self;
}

#[component]
pub fn ServiceMarkIcon(mark: ServiceMark, #[props(default = 18)] size: u16) -> Element;
```

Every match on `ServiceMark` and `FeedPlatform` is exhaustive, with no wildcard.

- `Rss` paints the Lucide `Rss` component from `window/icons.rs` (stroke icon).
- Every other mark paints a filled simple-icons path on a 24 × 24 view box, in the `TargetBrandIcon` way: `fill="currentColor"` and the colour on the svg's `style: "color: …"`.
- The svg carries `role="img"`, an `aria-label` with the Service or platform name, and `data-library-list-mark="<slug>"` with the simple-icons slug (`rss` for RSS). Tests read that hook.
- The Emby path moves to one `pub(crate) const EMBY_PATH` in `service_mark.rs`. `brand_icons.rs` `TargetBrand::Emby` reads it, so the path exists once.

Paths and colours. The paths come from `https://cdn.jsdelivr.net/npm/simple-icons@16.38.0/icons/<slug>.svg` (simple-icons 16.38.0, the latest release on 2026-11-20, CC0). Copy each `<path d>` as it is:

| Mark | Slug | Colour |
|---|---|---|
| Emby | `emby` | `#52B54B` |
| Audiobookshelf | `audiobookshelf` | `#82612C` |
| YouTube | `youtube` | `#FF0000` |
| SoundCloud | `soundcloud` | `#FF5500` |
| Spreaker | `spreaker` | `#F5C300` |
| Audioboom | `audioboom` | `#007CE2` |
| Patreon | `patreon` | `var(--canvas-heading)` (user decision: the brand black does not show on a dark theme) |
| Substack | `substack` | `#FF6719` |
| Spotify | `spotify` | `#1ED760` |
| Odysee | `odysee` | `#EF1970` |
| PeerTube | `peertube` | `#F1680D` |
| Internet Archive | `internetarchive` | `#666666` |
| RSS | Lucide `rss` | `var(--canvas-heading)` (user decision) |

`var(--canvas-heading)` is the old title's colour. It goes on the computed `color`, which the serialiser resolves before usvg reads it, so `var()` is safe there. Size 18 px fits the 14 px title line's height. The module doc names the simple-icons CC0 source, as `brand_icons.rs` does.

### D5. The header takes a heading

`LibraryListHeaderProps` replaces `title: String` and `title_attr` with:

```rust
pub enum LibraryListHeading {
    /// A Service destination: its mark, no text.
    Mark(ServiceMark),
    /// Home: its text title.
    Title(String),
}
pub heading: LibraryListHeading,
```

`Title` paints the existing span with `data-library-list-title`. `Mark` paints `ServiceMarkIcon` at the same place in the line. The dropdowns, the label, and the filter field do not change.

The callers:

| Caller | Heading |
|---|---|
| `library_list.rs` (Music) | `Mark(Emby)` |
| `library_emby_list.rs` (plain Emby and feed-group mode) | `Mark(Emby)` |
| `library_shows_list.rs` | `Mark(Emby)` |
| `library_audiobooks_list.rs` | `Mark(Audiobookshelf)` |
| `library_podcasts_list.rs` | `Mark(Audiobookshelf)` |
| `library_feeds_list.rs` | `Mark(ServiceMark::feeds(frame.platform))`, `Rss` while no frame |
| `library_home_list.rs` | `Title("Home")` |

`title_attr` had one user, Shows, so it goes. The Shows `list_title` computation becomes unused and goes.

### D6. Tests (contract → owner)

- *A URL shows its Feed platform* → `mbv-feed`, public API, so a new `crates/mbv-feed/tests/feed_platform.rs` with one named `#[case]` table. One case for each platform's feed form from the D1 table, a PeerTube URL on a self-hosted instance, a PeerTube instance-wide feed with no filter (generic), a lookalike host (`notyoutube.com`, generic), and a generic podcast host. `mbv-feed` gets `rstest.workspace = true` under `[dev-dependencies]` (the workspace pin, `rstest` 0.28 with `default-features = false`, which the other crates use, so no new version).
- *The Feeds mark follows the checked subscriptions* → the core, `src/app/tests/gui_library_feeds.rs`, through the existing App fixture and `toggle_subscription`. One table over the fixture's own YouTube and generic URLs: only YouTube checked → `Some(YouTube)`, YouTube plus generic → `None`. One test: All over YouTube-only subscriptions → `None`.
- *The header paints a mark or a title* → `components/library_list_header/tests.rs`. Change `the_title_line_paints_the_title_and_the_trigger` to a `Mark` heading: the mark hook is present, no title hook, and the trigger is present. Add one test: a `Title` heading paints the title text and no mark.
- Per-destination tests that read `[data-library-list-title]` (Feeds, Audiobooks, Emby, Podcasts) change to read the `data-library-list-mark` value (`rss`, `audiobookshelf`, `emby`, `audiobookshelf`). Home's test stays. No new per-destination test.

## Risks / Trade-offs

- [A platform changes its feed host, for example a move off `anchor.fm`] → The subscription shows the RSS mark. Nothing breaks. The D1 table is the one place to update.
- [A self-hosted feed reuses PeerTube's path and parameter names] → It shows the PeerTube mark. The parameter names are specific to PeerTube, so the risk is small.
- [Internet Archive grey (#666666) is dim on the dark theme] → It is the brand colour that the user's rule names. The colour is in one table row if a change is wanted.
- [A Substack or a Patreon feed on a custom domain] → It shows the RSS mark, as a generic feed does.
