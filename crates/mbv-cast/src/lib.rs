//! Google Cast discovery, client, and per-Service playback dispatch.
//!
//! Receiver discovery, the `CastClient` session, and partitioning of `QueueItem`
//! values by Service live here. Queue authority stays with the Player owner in
//! `mbv-queue`; this crate only plays what it is given.

pub mod client;
pub mod discovery;
pub mod dispatch;
mod error;
pub mod transport;

#[doc(inline)]
pub use client::{
    CastClient, CastIdleReason, CastMediaItem, CastPlaybackState, CastStatus, ConnectMode,
};
#[doc(inline)]
pub use discovery::{CastReceiver, browse_cast_receivers, resolve_cast_receiver};
#[doc(inline)]
pub use dispatch::{
    CastDeviceProfile, CastDispatchItem, CastSubtitleKind, build_cast_device_profile,
    partition_cast_dispatch, resolve_audiobookshelf_book_dispatch,
    resolve_audiobookshelf_episode_dispatch, resolve_feed_dispatch,
};
#[doc(inline)]
pub use error::CastError;
#[doc(inline)]
pub use transport::{CastJob, CastTransport, spawn_cast_worker};
#[cfg(any(test, feature = "test"))]
#[doc(inline)]
pub use transport::{FakeCastTransport, spawn_fake_cast_worker};
