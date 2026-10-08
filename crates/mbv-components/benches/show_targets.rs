//! Baseline bench for `TvContent::show_targets` (issue #896, `M-HOTPATH`).
//!
//! A show-mode catalog with duplicate Emby ids, exercising the collision
//! disambiguation every show-target lookup runs.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use mbv_components::tv_content::TvContent;
use mbv_emby_model::EmbyItem;
use mbv_emby_model::test_support::make_item;

const SHOW_COUNT: usize = 200;
const DUPLICATE_IDS: usize = 20;

/// Shows sharing a small pool of ids under distinct names, mixed with
/// uniquely identified shows: the duplicate-id shape `stable_show_target`
/// resolves.
fn catalog() -> Vec<EmbyItem> {
    (0..SHOW_COUNT)
        .map(|index| {
            let mut item = make_item(&format!("Show {index:03}"), "Series");
            if index % 4 == 0 {
                item.id = format!("duplicate-{}", index % DUPLICATE_IDS);
            } else {
                item.id = format!("unique-{index}");
            }
            item
        })
        .collect()
}

fn bench_show_targets(c: &mut Criterion) {
    let items = catalog();
    c.bench_function("show_targets", |bencher| {
        bencher.iter(|| black_box(TvContent::show_targets(black_box(&items))));
    });
}

criterion_group!(benches, bench_show_targets);
criterion_main!(benches);
