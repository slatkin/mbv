//! Tests of the natural-size cap (D16): the contracts of
//! `natural-size.test.ts` — a loaded image is capped at its natural size in
//! image pixels, and an image that has not loaded yet caps nothing. Pure
//! functions over the measured size.

use super::{NaturalSizeBounds, capped_natural_size};

#[test]
fn a_loaded_image_is_capped_at_its_natural_size_in_device_pixels() {
    let cap = capped_natural_size(400, 300, None).expect("the image loaded");

    assert_eq!(cap.max_width, 400.0);
    assert_eq!(cap.max_height, 300.0);
}

#[test]
fn an_image_that_has_not_loaded_yet_caps_nothing() {
    assert_eq!(capped_natural_size(0, 0, None), None);
}

/// The cap applies once the load lands (the `Loaded` event carries the size):
/// the natural size, unbounded when the caller names no
/// bounds box.
#[test]
fn a_loaded_image_caps_at_its_natural_size_when_no_bounds_are_named() {
    let cap = capped_natural_size(640, 360, None).expect("the image loaded");

    assert_eq!(cap.max_width, 640.0);
    assert_eq!(cap.max_height, 360.0);
}

/// A height cap narrows the painted width with the aspect, so a box that hugs
/// the image sees that width, not the wider width cap (hero frame wider on
/// the right than its art).
#[test]
fn the_fitted_width_follows_the_height_cap_at_the_natural_aspect() {
    let bounds = NaturalSizeBounds {
        width: Some(300.0),
        height: Some(450.0),
    };
    let cap = capped_natural_size(1000, 1500, Some(bounds)).expect("the image loaded");

    assert_eq!(cap.fitted_width, 300.0);

    let wide = NaturalSizeBounds {
        width: Some(300.0),
        height: Some(100.0),
    };
    let cap = capped_natural_size(1000, 1500, Some(wide)).expect("the image loaded");

    assert!((cap.fitted_width - 66.666_67).abs() < 0.01);
}
