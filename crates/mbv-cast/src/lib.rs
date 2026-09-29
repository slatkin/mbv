pub mod client;
pub mod discovery;
pub mod dispatch;
mod error;

pub use client::{CastClient, CastMediaItem, CastPlaybackState, CastStatus};
pub use discovery::{CastReceiver, browse_cast_receivers, resolve_cast_receiver};
pub use dispatch::{
    CastDeviceProfile, CastDispatchItem, CastSubtitleKind, build_cast_device_profile,
    partition_cast_dispatch, resolve_audiobookshelf_book_dispatch,
    resolve_audiobookshelf_episode_dispatch, resolve_feed_dispatch,
};
pub use error::CastError;
