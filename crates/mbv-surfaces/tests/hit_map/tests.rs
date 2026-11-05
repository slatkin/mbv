//! Public-API tests for the semantic hit map (task 4.2): hit, miss, and
//! resolve-against-last-frame, plus the paint-on-top overlap rule.

use mbv_surfaces::{HitMap, HitRegion};
use mbv_ui_msg::Position;

fn region(x: f32, y: f32, width: f32, height: f32) -> HitRegion {
    HitRegion::new(x, y, width, height)
}

#[test]
fn a_press_inside_a_painted_region_resolves_to_its_target() {
    let mut map = HitMap::new();
    map.push(region(10.0, 4.0, 20.0, 1.0), "slot-a");

    assert_eq!(map.resolve(Position { x: 15.0, y: 4.5 }), Some(&"slot-a"));
}

#[test]
fn a_press_outside_every_painted_region_resolves_to_no_target() {
    let mut map = HitMap::new();
    map.push(region(10.0, 4.0, 20.0, 1.0), "slot-a");

    assert_eq!(map.resolve(Position { x: 5.0, y: 4.5 }), None);
    assert_eq!(map.resolve(Position { x: 30.0, y: 4.5 }), None);
    assert_eq!(map.resolve(Position { x: 15.0, y: 3.0 }), None);
    assert_eq!(map.resolve(Position { x: 15.0, y: 5.0 }), None);
}

#[test]
fn an_empty_map_resolves_no_target() {
    let map = HitMap::<&str>::new();

    assert!(map.is_empty());
    assert_eq!(map.resolve(Position { x: 0.0, y: 0.0 }), None);
}

/// `client-interaction-core`, "A stale hit map": the core resolves against
/// the regions of the last painted frame and never against coordinates it
/// computes itself. The held frame-one map keeps resolving frame-one targets
/// after a newer frame's map exists.
#[test]
fn resolution_uses_the_last_painted_frame_not_the_current_content() {
    let mut last_frame = HitMap::new();
    last_frame.push(region(0.0, 0.0, 10.0, 1.0), "old-slot");
    let held = last_frame.clone();

    let mut next_frame = HitMap::new();
    next_frame.push(region(0.0, 0.0, 10.0, 1.0), "new-slot");

    assert_eq!(
        held.resolve(Position { x: 5.0, y: 0.5 }),
        Some(&"old-slot"),
        "the held map still resolves the frame it was painted in"
    );
    assert_eq!(
        next_frame.resolve(Position { x: 5.0, y: 0.5 }),
        Some(&"new-slot")
    );
}

/// A later push paints on top of an earlier one, so the last region painted
/// over a point wins the resolution.
#[test]
fn the_last_region_painted_over_a_point_wins() {
    let mut map = HitMap::new();
    map.push(region(0.0, 0.0, 10.0, 10.0), "background");
    map.push(region(2.0, 2.0, 4.0, 4.0), "overlay");

    assert_eq!(map.resolve(Position { x: 4.0, y: 4.0 }), Some(&"overlay"));
    assert_eq!(
        map.resolve(Position { x: 8.0, y: 8.0 }),
        Some(&"background")
    );
}

/// The region iteration exposes the recorded regions in push order with
/// their targets, so a Client composing a surface report from several
/// painted layers can read the map it holds.
#[test]
fn region_iteration_yields_the_pushed_regions_in_order() {
    let mut map = HitMap::new();
    map.push(region(0.0, 0.0, 10.0, 1.0), "first");
    map.push(region(0.0, 1.0, 10.0, 1.0), "second");

    let regions: Vec<_> = map.iter().collect();
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].1, &"first");
    assert_eq!(regions[1].1, &"second");
}
