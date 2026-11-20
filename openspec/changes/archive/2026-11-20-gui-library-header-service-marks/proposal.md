# Proposal

## Why

The GUI Library list header (the line above the `/` filter field) shows a text title that tells the user nothing new. "Movies", "TV", or "YouTube" repeats the active tab label. "Artists" names the list rows. On Shows, the title repeats the view dropdown value ("New", "Upcoming"). A mark of the destination's Service gives the user a fact that the tab bar does not show: where the list comes from.

## What Changes

- The list header of every Service destination shows a Service mark in place of the text title:
  - Emby destinations (Music, Shows, every plain Emby library, and an Emby homevideos feed view such as the "YouTube" tab): the Emby mark.
  - Audiobookshelf destinations (Audiobooks and Podcasts): the Audiobookshelf mark.
  - The Feeds tab: the mark of the Feed platform when the selection is not All and every checked FeedSubscription comes from the same known Feed platform. In every other case, the RSS mark.
- The known Feed platforms are YouTube, SoundCloud, Spreaker, Audioboom, Patreon, Substack, Spotify (Spotify for Creators feeds), Odysee, PeerTube, and Internet Archive. A subscription URL shows its platform: by its host for nine of them, and by its feed path for PeerTube, because every PeerTube instance has its own host.
- An Emby homevideos feed view keeps the Emby mark, also when its content is YouTube. The feed view is a generic Emby video library view and can grow past YouTube, so no YouTube rule ties to it (user decision).
- Every mark paints in its brand colour, except the RSS and Patreon marks. These two paint in the header text colour, which follows the theme. RSS has no brand colour, and Patreon's brand black does not show on a dark theme (user decision).
- Home keeps its "Home" title. The Playlists screen keeps its playlist title. The tab bar shows only an icon for these two, so their title still names the screen.
- The rest of the header does not change: the collection dropdown, the Shows view dropdown, the selection label, and the filter field stay where they are.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the list header's title line shows a Service mark for a Service destination in place of the text title. A new requirement states which mark each destination shows. A second new requirement states the Feeds selection rule. The requirement "The collection dropdown sits in the list header and no pill row remains" changes its Podcasts scenario from "the list title" to "the Service mark".

## Impact

- `crates/mbv-feed`: a new public `FeedPlatform` type that reads the Feed platform from a subscription URL.
- `src/app/gui_library/feeds_projection.rs` (core): one public function from the Feeds collection selector to the shared Feed platform of the checked subscriptions. `src/lib.rs` re-exports it and `FeedPlatform`.
- `crates/mbv-gui`: a new `window/service_mark.rs` module paints the marks. `components/library_list_header.rs` takes a heading that is a Service mark or a title. The seven header callers pass their heading. `bridge/frame/library/feeds.rs` carries the Feeds platform.
- `CONTEXT.md`: the new terms *Service mark* and *Feed platform*.
- No new third-party dependency. `mbv-feed` gets the workspace `rstest` as a dev-dependency. The icon paths are copied from simple-icons 16.38.0 (CC0), as the existing Emby mark is. The RSS mark is the vendored Lucide `Rss` icon (ISC).
- The TUI does not change. The subscribe step does not change: it still rejects a YouTube playlist feed URL (`normalize_feed_url` accepts only `channel_id=`). That gap is outside this change.
