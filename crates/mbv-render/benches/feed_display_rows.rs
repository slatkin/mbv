//! Baseline bench for `feed_display_rows` (issue #896, `M-HOTPATH`).
//!
//! Projects a feed catalog spread across every age group plus undated
//! entries: the shape the "All" feed list walks on each projection.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use mbv_queue::FeedEntry;
use mbv_render::feed_display_rows;

const NOW_SECS: u64 = 1_750_000_000;
const DAY_SECS: u64 = 86_400;
const ENTRY_COUNT: usize = 500;

/// A catalog spread across all age groups (`New`, `Recent`,
/// `OlderThanTwoWeeks`, `OlderThanMonth`) plus undated entries that fall
/// into `Unknown`.
fn catalog() -> Vec<FeedEntry> {
    let ages = [
        Some(NOW_SECS),
        Some(NOW_SECS - 5 * DAY_SECS),
        Some(NOW_SECS - 20 * DAY_SECS),
        Some(NOW_SECS - 60 * DAY_SECS),
        None,
    ];
    (0..ENTRY_COUNT)
        .map(|index| FeedEntry {
            guid: format!("guid-{index}"),
            title: format!("Episode {index}"),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: ages[index % ages.len()],
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played: false,
        })
        .collect()
}

fn bench_feed_display_rows(c: &mut Criterion) {
    let entries = catalog();
    c.bench_function("feed_display_rows", |bencher| {
        bencher.iter(|| black_box(feed_display_rows(black_box(&entries), NOW_SECS)));
    });
}

criterion_group!(benches, bench_feed_display_rows);
criterion_main!(benches);
