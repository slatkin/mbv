use super::cache::ImageCache;
use super::{CachedImage, mem_key};
use ratatui_image::picker::Picker;

impl ImageCache {
    /// Query the terminal for its image protocol and apply the configured
    /// protocol override when one is set.
    fn build_image_picker(&self) -> Picker {
        use ratatui_image::picker::ProtocolType;
        let protocol_override = self.protocol_override().map(str::to_owned);
        let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        let proto = protocol_override
            .as_deref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "sixel" => Some(ProtocolType::Sixel),
                "kitty" => Some(ProtocolType::Kitty),
                "iterm2" => Some(ProtocolType::Iterm2),
                "halfblocks" => Some(ProtocolType::Halfblocks),
                _ => None, // "auto" or unknown: use picker's detected protocol
            });
        if let Some(proto) = proto {
            picker.set_protocol_type(proto);
        }
        picker
    }

    /// Populate `image_picker` (terminal-detected, with the config override)
    /// and `halfblock_picker` (the #451 dimmed-backdrop fallback).
    ///
    /// MUST run before the `TuiRealm` crossterm listener starts
    /// (`Application::init`): `Picker::from_query_stdio` writes a `CSI 16 t`
    /// cell-size query to the terminal and reads the reply with a raw stdin
    /// read. If the listener is already draining stdin it eats the reply, the
    /// picker falls back to a wrong cell size, and Kitty renders images clipped
    /// on the right/bottom (#654).
    pub fn init_image_pickers(&mut self) {
        let picker = self.build_image_picker();
        log::debug!(
            target: "startup",
            "image picker: protocol={:?} font_size={:?}",
            picker.protocol_type(),
            picker.font_size()
        );
        self.initialize_image_pickers(picker, Picker::halfblocks());
    }

    /// The picker that encodes the given protocol suffix.
    #[must_use]
    pub fn picker_for_suffix(&self, suffix: &'static str) -> Option<&Picker> {
        if suffix == "halfblock" {
            self.halfblock_picker().or(self.image_picker())
        } else {
            self.image_picker()
        }
    }

    /// Builds a fresh cache entry for a just-fetched image: keeps the decoded
    /// source so it can be re-encoded with a different protocol picker later
    /// (#451), and encodes the protocol for the active suffix.
    /// `img: None` records a resolved-but-empty fetch (the "no art" marker
    /// renderers branch on).
    #[must_use]
    pub fn build_cached_image(
        &self,
        bare_key: &str,
        img: Option<image::DynamicImage>,
        suffix: &'static str,
    ) -> CachedImage {
        let mut entry = CachedImage {
            img,
            protocols: std::collections::HashMap::new(),
            cover_box: None,
            applied_logo_key: None,
        };
        if let Some(img) = entry.img.clone() {
            if let Some(picker) = self.picker_for_suffix(suffix) {
                let proto = self.build_protocol(bare_key, suffix, picker, img);
                entry.protocols.insert(suffix, proto);
            }
        }
        entry
    }

    #[must_use]
    pub fn build_protocol(
        &self,
        bare_key: &str,
        suffix: &'static str,
        picker: &Picker,
        img: image::DynamicImage,
    ) -> ratatui_image::thread::ThreadProtocol {
        #[cfg(any(test, feature = "test"))]
        self.record_protocol_build();
        let mem_key = mem_key(bare_key, suffix);
        let (req_tx, req_rx) = std::sync::mpsc::channel::<ratatui_image::thread::ResizeRequest>();
        let _ = self.resize_register_tx().send((mem_key, req_rx));
        ratatui_image::thread::ThreadProtocol::new(req_tx, Some(picker.new_resize_protocol(img)))
    }

    #[must_use]
    pub fn is_halfblock_configured(&self) -> bool {
        self.protocol_override()
            .is_some_and(|s| s.eq_ignore_ascii_case("halfblocks"))
            || self.image_picker().is_some_and(|p| {
                p.protocol_type() == ratatui_image::picker::ProtocolType::Halfblocks
            })
    }

    pub fn configured_protocol_name(&self) -> &'static str {
        use ratatui_image::picker::ProtocolType;
        match self.image_picker().map(Picker::protocol_type) {
            Some(ProtocolType::Sixel) => "sixel",
            Some(ProtocolType::Kitty) => "kitty",
            Some(ProtocolType::Iterm2) => "iterm2",
            Some(ProtocolType::Halfblocks) | None => "halfblock",
        }
    }

    #[must_use]
    pub fn images_enabled(&self) -> bool {
        self.protocol_enabled()
    }

    /// Resolve an optional Logo cache key to the key of a Logo that has decoded
    /// pixels to composite: a pending, absent, or failed Logo is not a
    /// decoration input, so the base-only protocol stays valid.
    pub fn ready_logo_key(&self, logo_cache_key: Option<&str>) -> Option<String> {
        logo_cache_key
            .filter(|key| self.image(key).is_some_and(|entry| entry.img.is_some()))
            .map(str::to_owned)
    }

    /// Paint the ready Logo at `logo_cache_key` over `img`, or return `img`
    /// unchanged when there is none (design D3).
    #[must_use]
    pub fn decorate_with_logo(
        &self,
        img: image::DynamicImage,
        logo_cache_key: Option<&str>,
    ) -> image::DynamicImage {
        let Some(logo) = logo_cache_key
            .and_then(|key| self.image(key))
            .and_then(|entry| entry.img.as_ref())
        else {
            return img;
        };
        crate::composite_landscape_logo(&img, logo)
    }
}
