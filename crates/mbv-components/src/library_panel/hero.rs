//! Transitional re-export (task 1.7): the artwork policies, the hero
//! producers, and their `HeroContentData`/`HeroCredit` now live in
//! `mbv_ui_model::library_hero::producers` so both Clients read them from the
//! shared leaf crate. The paint code stays in `mbv-components`. Task 1.8
//! deletes this module and the re-exports at the old paths.

#[doc(inline)]
pub use mbv_ui_model::library_hero::producers::{
    HeroContentData, HeroCredit, abs_book_artwork_policy, abs_episode_artwork_policy,
    emby_artwork_policy, emby_hero_meta_rows_plain, feed_artwork_policy, hero_content_abs_book,
    hero_content_abs_episode, hero_content_emby, hero_content_feed, hero_content_music_album,
    hero_content_queue, hero_content_series_with_episode, hero_content_twitch, music_album_artwork,
};
