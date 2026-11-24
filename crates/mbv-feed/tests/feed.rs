//! The public contracts of `mbv-feed`: the URL's platform, and the
//! subscription URL's normalization.
//!
//! The inline `mod feed` makes the child files resolve against `tests/feed/`
//! (one binary per crate; no `mod.rs`).

mod feed {
    mod normalize;
    mod platform;
}
