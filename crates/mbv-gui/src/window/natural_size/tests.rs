//! Tests of the natural-size cap (D16): the contracts of
//! `natural-size.test.ts` — a loaded image is capped at its natural size in
//! device pixels, and an image that has not loaded yet caps nothing. Pure
//! functions over the measured size and the scale factor.

use super::capped_natural_size;

#[test]
fn a_loaded_image_is_capped_at_its_natural_size_in_device_pixels() {
    let cap = capped_natural_size(400, 300, 2.0, None).expect("the image loaded");

    assert_eq!(cap.max_width, 200.0);
    assert_eq!(cap.max_height, 150.0);
}

#[test]
fn an_image_that_has_not_loaded_yet_caps_nothing() {
    assert_eq!(capped_natural_size(0, 0, 1.0, None), None);
}

/// The cap applies once the load lands (the `Loaded` event carries the size):
/// the natural size at the scale factor, unbounded when the caller names no
/// bounds box.
#[test]
fn a_loaded_image_caps_at_its_natural_size_when_the_scale_factor_is_one() {
    let cap = capped_natural_size(640, 360, 1.0, None).expect("the image loaded");

    assert_eq!(cap.max_width, 640.0);
    assert_eq!(cap.max_height, 360.0);
}
