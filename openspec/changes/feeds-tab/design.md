## Context

See proposal.md — Why. Depends on #470 (`QueueItem::Feed(FeedEntry)`). Relevant current state:

- **Config is hand-parsed toml, not derived serde.** `Config` (`config_types_paths.rs:4`) derives only `Debug, Clone`. Loading is section/key extraction in `config_parse.rs` (`load_config` / `parse_config`); saving is a read-modify-write merge onto the existing toml `Value` in `config_save.rs` (preserves comments and unknown keys). New config data is added by hand on both sides — there is no auto-serialization.
- **Tabs are data-driven off Emby libraries.** `chrome_tabs.rs:124` builds `all_names` as `once("Home").chain(self.libs.iter().map(name))`. The selected tab is an index (`library_tab`) into that list, and content routing keys off that index. There is no notion of a non-library tab today.
- **The idle-feed parser is title+link only.** `feed_parse.rs::fetch_and_parse_rss` returns `IdleFeedItem { title, link }`; it does not read `<enclosure>`, `<guid>`, MIME, or duration.
- **A feed-style view already exists.** `render/home_feed.rs` renders a feed list with a pill-bar group selector (`render_pill_bar`).

## Goals / Non-Goals

**Goals:**
- Subscriptions in `config.toml`, portable, no daemon, no redb.
- A Feeds tab that reads subscriptions from config, fetches/parses on demand, and plays entries through the #470 queue.
- Reuse the existing parser and feed-view rendering rather than inventing new ones.

**Non-Goals (design-level):**
- No playback state of any kind (position, played, watched filter) — #472.
- No generalizing the renderer off `EmbyItem`; render the concrete `FeedEntry`.
- No auto-refresh. Entries refresh only on the user's `r` keypress on the Feeds tab; no fetch-on-open, no timer.

## Decisions

**1. `[[feeds]]` array-of-tables in config, added by hand on both sides.**
Add `feeds: Vec<FeedSubscription>` to `Config`, where `FeedSubscription { name: String, url: String, kind: FeedKind }`. Parse a `[[feeds]]` array in `config_parse.rs` (mirror the existing `get_str`/array patterns); write it in `config_save.rs` by building `toml::Value::Array` of tables and merging (same read-modify-write that preserves the rest of the file). Rejected: switching `Config` to derived serde — far larger blast radius and would reformat users' hand-edited `config.toml`.

**2. `FeedKind` inferred from enclosure MIME, defaulting to Video.**
On parse, infer audio vs video from the enclosure MIME type; when absent, default Video. The subscription's stored `kind` is the user-declared default; per-entry kind can refine it from the enclosure. Keep inference in one helper so #472 can reuse it.

**3. Extend the parser in place; keep the idle-feed path working.**
`fetch_and_parse_rss` gains extraction of `guid`, enclosure URL, MIME, and duration (→ ticks at parse time). The idle feed only needs title+link, so either return a richer struct that the idle path ignores, or add a sibling `fetch_and_parse_entries` that yields `FeedEntry` and have the idle path keep its lean shape. Prefer the sibling function to avoid disturbing idle-feed behavior; share the tag-extraction helpers (`extract_tag`, `extract_atom_link`).

**4. Feeds tab is a synthetic tab appended after libraries.**
The one real integration subtlety: `all_names` and tab routing are indexed off `self.libs`. Model the Feeds tab as an explicit extra tab position — append its name after the library names when `!config.feeds.is_empty()`, and branch content routing on "index == feeds tab" before the library-index path. If deleting the last subscription while this tab is selected, reset the selection to Home. Rejected: faking a pseudo-library entry — it would leak into every place that treats a tab as an Emby library (fetching library items, hero, filters).

**5. Render `FeedEntry` directly, reusing the feed-view layout.**
Reuse `home_feed.rs`'s pill-bar + list layout, but feed it concrete `FeedEntry` values (groups = subscriptions, plus an "All" group sorted by pub date desc, missing dates last). Do not adapt the Emby-shaped state to carry feed data and do not generalize the renderer off `EmbyItem`.

**6. Play path builds `QueueItem::Feed`, with capability-gated transient ctrl state.**
"Play" on an entry constructs a `FeedEntry` → `QueueItem::Feed`, appends to the bound queue, and plays. A capability-supporting Player owner may receive that request through the additive `feed-playback` ctrl command. The daemon is authoritative for a mixed remote queue: its atomic `CtrlState` snapshot includes a `feed_items` tail only for capability-supporting peers. Queue persistence uses the tagged `QueueItem` shape, so Feed entries restore with their identity and no playback-progress state. The queue invariant is Emby items first and Feed items last; clients reconstruct a slot-identical queue by concatenating the Emby wire items and Feed tail. While the tail is nonempty, the daemon rejects Emby mutations that could place Emby content after it (append, move, replace, adopt). A player Feed-removal event updates the daemon's tail and the reconnect snapshot before it broadcasts the next state. This avoids a separate event's ordering race and avoids unknown absolute mixed indices.

**7. Management overlay follows the existing overlay pattern.**
Add/remove/edit in an overlay modeled on `src/app/render/overlays/`. Add fetches+parses first (decision 3); on failure, surface via the existing status/notify path and do not save. On success, append to `config.feeds` and persist via the `config_save.rs` merge.

**8. Manual refresh only.**
Entries are fetched on the user's `r` keypress while the Feeds tab is active; there is no fetch-on-open and no timer. Before the first `r`, the tab shows an empty/"press r to load" state. This keeps startup and tab switches free of network work and matches the existing single-key action idiom.

**9. The management overlay is opened from F2 Settings.**
The Feeds tab is correctly hidden with no subscriptions, so the overlay cannot rely on a tab-local key as its only entry point. Add a `Manage feeds` Settings row whose activation opens the management overlay. Rejected: an always-visible empty Feeds tab and a new global shortcut; both create a less coherent navigation surface than the existing configuration panel.

**10. Editing changes only a subscription's display name and kind.**
The URL is editable while adding a subscription, but read-only in the edit path. A changed URL is a new subscription, so users remove the old one and add the new URL. This retains the domain identity rule without inventing invisible delete-and-recreate behavior.

**11. Subscription mutations discard fetched, in-memory entry lists.**
After add, remove, or edit, resynchronize the tab from `config.feeds`, clear its per-subscription and All-group entries, clamp group/cursor/scroll state, and do not fetch automatically. An in-flight add result carries an editor generation and is ignored unless the same editor and URL are still current. This prevents stale results from saving cancelled/changed subscriptions and prevents entries fetched for one subscription being shown under another after a deletion.

## Risks / Trade-offs

- **Tab-index routing assumes every tab is a library** → decision 4 makes the Feeds tab an explicit branch checked before the library-index lookup; audit each site that maps `library_tab` → library so the feeds index can't be read as a library.
- **Parser brittleness on real-world feeds** (missing enclosures, odd duration formats) → default kind Video, treat missing enclosure by falling back to link, treat unparseable duration as unknown (`None`); never fail the whole feed on one bad field.
- **File-size cap (800 lines).** `config_types_paths.rs` is already 677 lines and `feed_actions.rs` 679 — adding here risks the cap. Put new subscription types in a small new module and new feed-tab state/actions in their own files rather than growing the existing ones.
