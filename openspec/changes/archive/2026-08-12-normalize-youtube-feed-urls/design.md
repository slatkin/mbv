## Context

Feed add already runs a network fetch on a background thread before it commits:
`submit_feed_add` (in `src/app/feeds_manage_actions.rs`) spawns a thread that
calls `fetch_and_parse_entries(url, kind)` and returns a `FeedAddResult`
carrying the `url`. `drain_feed_add_results` pushes that `url` verbatim into
`config.feeds` and persists it. So there is already an async seam where a URL
can be transformed, and the result already carries the URL that gets stored.

See proposal.md — Why for motivation.

## Goals / Non-Goals

**Goals:**
- Resolve a YouTube channel URL to its RSS feed URL at subscribe time, once,
  and persist the resolved URL.
- Keep the change confined to the two feed files; no config, protocol, or UI
  change.

**Non-Goals:**
- A general provider-normalization framework. This is YouTube-only, expressed
  as a single function that non-YouTube URLs fall straight through.
- Re-resolving on every refresh. Resolution happens once, at add time.

## Decisions

**Resolve inside the existing add thread, return the resolved URL.**
`normalize_feed_url(typed) -> Result<String, String>` runs first in the thread
`submit_feed_add` already spawns; its output feeds `fetch_and_parse_entries`
and is placed in `FeedAddResult.url`, so `drain_feed_add_results` persists the
resolved URL with no further change. Alternative — normalize at fetch time on
every refresh — was rejected: it re-scrapes on each poll and stores the
unresolved URL in config, which reads wrong.

**Two resolution paths, keyed on whether the id is already present.**
- `youtube.com/channel/UC…` — the channel id is in the path; rewrite to
  `feeds/videos.xml?channel_id=<id>` with a pure string transform, no network.
- `youtube.com/@handle`, `/c/<name>`, `/user/<name>` — no id in the URL; fetch
  the channel page and read the RSS URL it advertises. A single scrape path
  covers all three forms, so they need no per-form special-casing.

**Scrape the advertised RSS link, not scraped ids.** YouTube channel pages
carry `<link rel="alternate" type="application/rss+xml" href="…channel_id=UC…">`
in the head. Reading that href is more robust than digging a `channelId` out of
`ytInitialData`, where the same key also appears for unrelated recommended
channels. Reuse the existing TLS `ureq` agent from `fetch_feed_body`.

**Failure aborts the add.** A recognized-but-unresolvable YouTube URL returns
`Err`, which flows through the existing add-failure toast in
`drain_feed_add_results`; nothing is saved. No fall-back to the unresolved URL,
which would just fail to fetch forever and confuse the user.

**Non-YouTube passes through.** Any URL whose host is not a YouTube host
returns `Ok(url_unchanged)`, so existing RSS/Atom subscriptions are untouched.

## Risks / Trade-offs

- **YouTube changes its page markup** → the scrape stops finding the RSS link.
  Mitigation: key on the standard `application/rss+xml` alternate link (stable,
  and what feed readers rely on generally); on miss, fail loudly so the user
  sees it rather than silently saving a broken URL.
- **Channel page fetch adds latency to add** → only for handle/custom URLs, and
  add is already async with a "Fetching feed…" toast, so the extra round trip
  is within the existing UX.
