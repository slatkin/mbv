//! The glow cache's contract: it is bounded, and the oldest entry leaves
//! first (`gui-hero-design-polish` design D4). No thread, no image.

use std::sync::Arc;

use super::{GLOW_CACHE_CAPACITY, GlowCache};

fn png(byte: u8) -> Arc<[u8]> {
    Arc::from(vec![byte])
}

#[test]
fn the_cache_drops_its_oldest_entry_past_the_capacity() {
    let mut cache = GlowCache::default();
    for index in 0..=GLOW_CACHE_CAPACITY {
        cache.insert(format!("key{index}"), png(0));
    }

    assert!(cache.get("key0").is_none());
    assert!(cache.get("key1").is_some());
    assert!(cache.get(&format!("key{GLOW_CACHE_CAPACITY}")).is_some());
}

#[test]
fn a_repeated_key_does_not_grow_the_cache() {
    let mut cache = GlowCache::default();
    cache.insert("key".into(), png(1));
    cache.insert("key".into(), png(2));

    assert_eq!(cache.order.len(), 1);
    assert_eq!(cache.get("key").as_deref(), Some(&[2u8][..]));
}
