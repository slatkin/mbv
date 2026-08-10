## 1. URL normalization

- [x] 1.1 Add `normalize_feed_url(input: &str) -> Result<String, String>` to `src/app/feed_parse.rs`. Non-YouTube host → return input unchanged.
- [x] 1.2 Recognize YouTube hosts (`youtube.com`, `www.youtube.com`, `m.youtube.com`). URLs already of the `feeds/videos.xml?channel_id=…` form pass through unchanged.
- [x] 1.3 `youtube.com/channel/UC…` → rewrite to `https://www.youtube.com/feeds/videos.xml?channel_id=<id>` with no network call.
- [x] 1.4 `youtube.com/@handle`, `/c/<name>`, `/user/<name>` → fetch the channel page (reuse the TLS `ureq` agent from `fetch_feed_body`) and extract the `<link rel="alternate" type="application/rss+xml" href="…">` URL.
- [x] 1.5 Return `Err` when a recognized YouTube channel URL cannot be resolved (page fetch fails or no RSS link found).

## 2. Wire into add-feed

- [x] 2.1 In `submit_feed_add`'s spawned thread (`src/app/feeds_manage_actions.rs`), call `normalize_feed_url` first; on `Err`, send the error through the existing `FeedAddResult.result` failure path.
- [x] 2.2 On success, use the resolved URL for `fetch_and_parse_entries` and set `FeedAddResult.url` to the resolved URL so `drain_feed_add_results` persists it.

## 3. Tests

- [x] 3.1 Unit-test the pure paths of `normalize_feed_url`: `channel/UC…` rewrite, already-a-feed pass-through, non-YouTube pass-through. (No network/E2E tests — the scrape path is covered by manual verification.)

## 4. Verify

- [x] 4.1 `cargo test -p mbv-core` and the app crate's feed tests pass; `cargo clippy --workspace --all-targets` clean.
- [x] 4.2 Manual: add `https://www.youtube.com/@ChineseCookingDemystified`, confirm config stores the `channel_id` feed URL and entries load.
