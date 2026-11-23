//! The pushed protocol facts the projections read, and the neutral records
//! they leave for the shell (change `split-tui-gui-clients`, design D2 rule 3).
//!
//! The terminal image protocol store — the pickers, the resize worker, the
//! encodings — belongs to the TUI Client alone, so it lives on the shell
//! `Model`. The core keeps the decisions: which suffix the cache keys carry,
//! whether halfblocks make the artwork unable to carry the title, and which
//! entries need an encoding or a cleared one. The shell pushes the facts in
//! and drains the records out.

use crate::app::App;
use mbv_images::hash_image_bytes;

impl App {
    /// The suffix the artwork cache keys must carry: the halfblocks
    /// fallback's while a dimming backdrop forces it (#451), else the
    /// configured one. Until the pickers are initialized the configured
    /// suffix is read only, so a key built then matches the one the first
    /// paint resolves.
    #[must_use]
    pub fn image_protocol_suffix(&self) -> &'static str {
        if self.dim_halfblock_forced() {
            "halfblock"
        } else {
            self.configured_protocol_suffix
        }
    }

    /// Whether a dimming backdrop forces the halfblocks picker over a
    /// differently configured protocol (#451): the fallback measures cells on
    /// its hardcoded font grid, so geometry feedback loops must hold the
    /// configured protocol's geometry while it is up.
    #[must_use]
    pub fn dim_halfblock_forced(&self) -> bool {
        self.dim_backdrop_active
            && self.images.protocol_enabled()
            && self.image_pickers_ready
            && !self.halfblock_configured
    }

    /// Record whether the configured picker paints halfblocks — the title
    /// site's sizing gate — whether the pickers are initialized at all, and
    /// the configured picker's terminal font size.
    pub fn set_image_protocol_facts(
        &mut self,
        halfblock_configured: bool,
        ready: bool,
        font_size: (u16, u16),
    ) {
        self.halfblock_configured = halfblock_configured;
        self.image_pickers_ready = ready;
        self.image_font_size = font_size;
    }

    /// Record the configured protocol's suffix, which the artwork cache keys
    /// embed.
    pub fn set_configured_protocol_suffix(&mut self, suffix: &'static str) {
        self.configured_protocol_suffix = suffix;
    }

    /// Record whether a dimming backdrop is up, which forces the shell's
    /// halfblocks fallback over a differently configured picker (#451). The
    /// shell derives it from its overlay stack and pushes it before its sync
    /// pass; the artwork cache keys and the title site's geometry read it.
    pub fn set_dim_backdrop_active(&mut self, active: bool) {
        self.dim_backdrop_active = active;
    }

    /// Whether halfblocks leave the artwork unable to carry the now-playing
    /// title: the queue title site's `HalfblockConfigured` gate.
    #[must_use]
    pub(in crate::app) fn halfblock_configured(&self) -> bool {
        self.halfblock_configured
    }

    /// The configured picker's terminal cell, in pixels: the grid the queue
    /// card's title overlay is composed at and the hero cover fit's box pixels
    /// derive from (the constructor's default until the pickers initialize).
    #[must_use]
    pub(in crate::app) fn image_font_size(&self) -> (u16, u16) {
        self.image_font_size
    }

    /// Queue one completed fetch's decoded source for the shell to encode and
    /// register under `cache_key`. Does nothing when the source carries no
    /// pixels (a resolved-empty or interrupted fetch).
    pub(in crate::app) fn queue_protocol_encode(
        &mut self,
        cache_key: &str,
        img: Option<&image::DynamicImage>,
    ) {
        if let Some(img) = img {
            self.pending_protocol_encodes
                .push_back((cache_key.to_owned(), img.clone()));
        }
    }

    /// Take the next completed fetch awaiting its encoding, with the
    /// configured suffix its key carries and the entry's pixel hash, which
    /// tags the encoding.
    #[must_use]
    pub fn next_pending_protocol_encode(&mut self) -> Option<PendingProtocolEncode> {
        let (cache_key, img) = self.pending_protocol_encodes.pop_front()?;
        let pixel_hash = hash_image_bytes(&img);
        Some(PendingProtocolEncode {
            cache_key,
            suffix: self.configured_protocol_suffix,
            img,
            pixel_hash,
        })
    }

    /// Queue `cache_key` for the shell to drop its stored encoding, so the
    /// next paint re-encodes from the retained source.
    pub(in crate::app) fn queue_protocol_clear(&mut self, cache_key: &str) {
        self.pending_protocol_clears.push(cache_key.to_owned());
    }

    /// Take every cache key whose encoding the shell must drop.
    pub fn take_pending_protocol_clears(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_protocol_clears)
    }

    /// The pixel hash of `cache_key`'s source image, the tag an encoding is
    /// stored under, so a re-encode of the same pixels is reused.
    #[must_use]
    pub(in crate::app) fn source_image_hash(&self, cache_key: &str) -> Option<u64> {
        self.images
            .image(cache_key)
            .and_then(|entry| entry.img.as_ref())
            .map(hash_image_bytes)
    }
}

/// One completed fetch awaiting the shell's encoding: the decoded source and
/// the configured suffix its cache key carries.
#[derive(Debug)]
pub struct PendingProtocolEncode {
    pub cache_key: String,
    pub suffix: &'static str,
    pub img: image::DynamicImage,
    pub pixel_hash: u64,
}
