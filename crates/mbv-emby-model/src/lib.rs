//! Emby item models and shared playback-position units.

#[cfg(any(test, feature = "test"))]
pub mod test_support;

pub const TICKS_PER_SECOND: i64 = 10_000_000;
// Keep this equal to TICKS_PER_SECOND; the f64 form avoids repeated integer casts.
pub const TICKS_PER_SECOND_F64: f64 = 10_000_000.0;

/// Convert ticks to seconds. The i64-to-f64 conversion is exact for |ticks| < 2^53
/// (about 28,500 years of media).
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "i64 ticks to f64 seconds is exact for |ticks| < 2^53 (about 28,500 years of media)"
)]
pub fn ticks_to_seconds(ticks: i64) -> f64 {
    ticks as f64 / TICKS_PER_SECOND_F64
}

/// Convert an f64 to i64, mapping NaN to zero and saturating at i64 bounds.
///
/// Rust's `as` cast from float to int has been saturating (and NaN-mapping-to-zero)
/// since 1.45, so this is exactly that cast, exposed under its own name so call
/// sites document intent instead of re-deriving the cast's behavior.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "the `as` cast saturates/truncates by design; that is this function's contract"
)]
pub fn saturating_i64_from_f64(value: f64) -> i64 {
    value as i64
}

/// Convert seconds to ticks, truncating fractional ticks and saturating at i64 bounds.
#[must_use]
pub fn seconds_to_ticks(seconds: f64) -> i64 {
    saturating_i64_from_f64(seconds * TICKS_PER_SECOND_F64)
}

/// Convert a non-negative `i64` magnitude (a volume, a seconds count, a pixel
/// dimension) to `f64`, saturating at `u32::MAX` for values that don't fit.
/// mbv's magnitudes here never approach that bound, so this is effectively exact.
#[must_use]
pub fn i64_to_f64_saturating(value: i64) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

/// Inclusive lower-bound percentage of known runtime at which a saved
/// position qualifies for resume. Exactly this percent qualifies.
pub const RESUME_THRESHOLD_PERCENT: i64 = 6;

/// Shared resume-eligibility predicate used by both Emby items and feed
/// entries. A positive saved position with unknown runtime (`runtime_ticks
/// <= 0`) is always resumable. Zero and negative positions never qualify.
/// For a known runtime the position must be at least `RESUME_THRESHOLD_PERCENT`
/// (inclusive) of runtime. Uses `i128` multiplication to avoid overflow.
#[must_use]
pub fn should_resume(position_ticks: i64, runtime_ticks: i64) -> bool {
    if position_ticks <= 0 {
        return false;
    }
    if runtime_ticks > 0 {
        i128::from(position_ticks) * 100
            >= i128::from(runtime_ticks) * i128::from(RESUME_THRESHOLD_PERCENT)
    } else {
        true
    }
}

// Task 5.3d (Emby browser effect decoupling): `PartialEq` is required so the
// TuiRealm shell `Msg`/`ShellRequest` enums (which are `#[derive(PartialEq)]`
// — the Application requires `Msg: PartialEq`) can carry the component-resolved
// item as the explicit owned target of a typed effect. Additive derive only;
// no semantics change.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbyPerson {
    pub name: String,
    pub role: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbyLink {
    pub name: String,
    pub url: String,
}

/// One `ArtistItems` name/ID pair from a music album/item payload: an
/// album-artist display name and the stable artist item ID it maps to.
/// This pair is mbv's only source of stable artist identity; IDs from an
/// `/Artists` listing never enter this field.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbyArtistRef {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbyItem {
    pub id: String,
    pub name: String,
    pub item_type: String,
    pub is_folder: bool,
    #[serde(default)]
    pub child_count: Option<u32>,
    pub media_type: String,
    pub collection_type: String,
    pub runtime_ticks: i64,
    pub played: bool,
    pub playback_position_ticks: i64,
    pub series_id: String,
    pub series_name: String,
    pub album_id: String,
    pub album: String,
    pub index_number: i64,
    pub parent_index_number: i64,
    pub unplayed_item_count: u32,
    pub path: String,
    pub artist: String,
    /// `ArtistItems` name/ID pairs as returned on the payload, retained
    /// verbatim. `Default` so legacy serialized queue items (which predate
    /// the field) deserialize unchanged; serialized queue compatibility is
    /// unchanged.
    #[serde(default)]
    pub artist_items: Vec<EmbyArtistRef>,
    pub sort_name: String,
    pub production_year: u32,
    pub end_year: u32,
    pub overview: String,
    pub premiere_date: String,
    /// Full ISO 8601 (not truncated to a date), so destination Latest markers
    /// can compare it against the launch window at second precision.
    pub date_added: String,
    pub total_count: u32,
    pub container: String,
    pub video_info: String,
    pub audio_info: String,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub people: Vec<EmbyPerson>,
    #[serde(default)]
    pub external_urls: Vec<EmbyLink>,
    pub playlist_item_id: String,
    /// Declared image availability (task 5.3): the Emby default-DTO image
    /// tags the hero artwork policy reads to choose an artwork shape before
    /// any image is fetched. `Default` so older payloads deserialize
    /// unchanged.
    #[serde(default)]
    pub image_tags: EmbyImageTags,
}

/// Declared image availability for one item (task 5.3, design D5): the
/// provider metadata the artwork policy reads to decide availability before
/// any image is fetched, so the chosen shape never changes when the image
/// arrives. These are Emby's default item-DTO members (not `Fields`-gated
/// properties), so parsing them needs no extra `Fields` request.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbyImageTags {
    /// `ImageTags.Thumb`: a 16:9 landscape thumbnail is declared available.
    #[serde(default)]
    pub thumb: String,
    /// `ImageTags.Logo`: an optional semantic logo decoration is declared available.
    #[serde(default)]
    pub logo: String,
    /// `ImageTags.Primary`: a primary (poster/square) image is declared
    /// available.
    #[serde(default)]
    pub primary: String,
    /// `BackdropImageTags`: landscape backdrop tags.
    #[serde(default)]
    pub backdrops: Vec<String>,
    /// On episodes: the series' thumb tag (`ParentThumbImageTag`, falling
    /// back to `SeriesThumbImageTag`).
    #[serde(default)]
    pub series_thumb: String,
    /// On episodes: the series' backdrop tags (`ParentBackdropImageTags`).
    #[serde(default)]
    pub series_backdrops: Vec<String>,
}

impl EmbyItem {
    #[must_use]
    pub fn is_audio(&self) -> bool {
        self.media_type == "Audio" || self.item_type == "Audio"
    }

    /// Whether this is a music item: an Emby track, album or artist.
    ///
    /// Music is fire-and-forget in mbv — playback never resumes one
    /// (`resume_start_pos` gates on `!is_audio`), so its stored played/resume
    /// facts carry no meaning anywhere: a row derived from a music item is
    /// always ordinary, and only the row that is actually playing shows
    /// anything.
    #[must_use]
    pub fn is_music(&self) -> bool {
        self.item_type == "MusicAlbum" || self.item_type == "MusicArtist" || self.is_audio()
    }

    /// Resolves this item's stable artist item ID from its `ArtistItems`
    /// payload pairs. A pair is applicable only when its trimmed name
    /// case-insensitively equals the settled display artist, and only a
    /// single applicable pair resolves: zero matches, or several equal-name
    /// pairs, resolve nothing rather than picking an arbitrary first pair.
    /// Returns `None` for every absent, unmatched, or ambiguous case.
    #[must_use]
    pub fn matched_artist_item_id(&self, display_artist: &str) -> Option<&str> {
        let target = display_artist.trim().to_lowercase();
        if target.is_empty() {
            return None;
        }
        let mut matched: Option<&str> = None;
        for pair in &self.artist_items {
            if pair.id.is_empty() || pair.name.trim().to_lowercase() != target {
                continue;
            }
            if matched.is_some() {
                // Ambiguous: several pairs carry the display name with
                // distinct (or repeated) IDs. No arbitrary first pick.
                return None;
            }
            matched = Some(&pair.id);
        }
        matched
    }

    #[must_use]
    pub fn is_video(&self) -> bool {
        self.media_type == "Video"
    }

    #[must_use]
    pub fn resume_seconds(&self) -> f64 {
        ticks_to_seconds(self.playback_position_ticks)
    }

    #[must_use]
    pub fn should_resume(&self) -> bool {
        should_resume(self.playback_position_ticks, self.runtime_ticks)
    }

    #[must_use]
    pub fn runtime_seconds(&self) -> f64 {
        ticks_to_seconds(self.runtime_ticks)
    }

    #[must_use]
    pub fn file_name(&self) -> &str {
        if self.path.is_empty() {
            return &self.name;
        }
        let p = std::path::Path::new(&self.path);
        p.file_name().and_then(|f| f.to_str()).unwrap_or(&self.name)
    }

    #[must_use]
    pub fn sort_key(&self) -> &str {
        if !self.path.is_empty() {
            self.file_name()
        } else if !self.sort_name.is_empty() {
            &self.sort_name
        } else {
            &self.name
        }
    }

    #[must_use]
    pub fn playback_label(&self) -> String {
        if self.item_type == "Audio" && !self.artist.is_empty() {
            format!("{} - {}", self.artist, self.name)
        } else {
            self.display_name()
        }
    }

    #[must_use]
    pub fn folder(id: String, name: String, collection_type: String) -> Self {
        EmbyItem {
            id,
            name,
            item_type: "CollectionFolder".to_string(),
            is_folder: true,
            child_count: None,
            collection_type,
            media_type: String::new(),
            runtime_ticks: 0,
            played: false,
            playback_position_ticks: 0,
            series_id: String::new(),
            series_name: String::new(),
            album_id: String::new(),
            album: String::new(),
            index_number: 0,
            parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(),
            artist: String::new(),
            artist_items: Vec::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            overview: String::new(),
            premiere_date: String::new(),
            date_added: String::new(),
            total_count: 0,
            container: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genres: Vec::new(),
            people: Vec::new(),
            external_urls: Vec::new(),
            playlist_item_id: String::new(),
            image_tags: EmbyImageTags::default(),
        }
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        if self.item_type == "Episode" && !self.series_name.is_empty() {
            format!("{} {}", self.series_name, self.name)
        } else {
            self.name.clone()
        }
    }

    /// The two-tone title parts for a list row: the series title and, for
    /// episode rows, the episode title that paints after it in the accent
    /// role. Non-episode items return the display name with no second part.
    #[must_use]
    pub fn display_name_parts(&self) -> (String, Option<String>) {
        if self.item_type == "Episode" && !self.series_name.is_empty() {
            (self.series_name.clone(), Some(self.name.clone()))
        } else {
            (self.name.clone(), None)
        }
    }
}

#[cfg(test)]
mod tests;
