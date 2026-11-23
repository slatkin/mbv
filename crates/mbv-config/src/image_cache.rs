//! The on-disk image cache: one file per cache key under the cache directory.
//!
//! The fetch writes an entry, a later fetch reads it, and both the
//! `mpris:artUrl` builder and the Audiobookshelf invalidation name the same
//! files through the helpers here, so a write and a read never disagree about
//! the filename.

use std::path::PathBuf;

use crate::types_paths::cache_dir;

/// The directory holding the on-disk image cache entries.
#[must_use]
pub fn image_disk_cache_dir() -> PathBuf {
    cache_dir().join("images")
}

/// The on-disk cache generation, folded into every image cache filename. The
/// files hold the server's raw bytes, so a change to the fetch's resolution
/// or quality makes every entry the previous generation wrote stale; bumping
/// this name stops those entries from satisfying fetches, and the mtime
/// eviction (`evict_old_image_cache`) reclaims their files.
///
/// Generation 2 is the 1280-pixel, quality-90 fetch.
const IMAGE_DISK_CACHE_GENERATION: &str = "v2";

/// The image cache's filename for `key`: the key's path-safe form and the
/// cache generation. Reads, writes, and the `mpris:artUrl` path all derive the
/// name here, so a fetch never writes one file while a later read looks for
/// another.
fn image_cache_filename(key: &str) -> String {
    format!("{}.{IMAGE_DISK_CACHE_GENERATION}", safe_cache_filename(key))
}

/// The cached bytes for `key`, when an entry is present. A hit refreshes the
/// entry's mtime so the eviction measures last use.
#[must_use]
pub fn read_image_disk_cache(key: &str) -> Option<Vec<u8>> {
    let path = image_disk_cache_dir().join(image_cache_filename(key));
    let bytes = std::fs::read(&path).ok()?;
    touch_image_disk_cache(&path);
    Some(bytes)
}

/// Path to the on-disk cached image file for `key`, if one is already
/// present -- without reading its bytes. Used to build `mpris:artUrl`
/// `file://` URIs (see `mbv-desktop::mpris::resolve_art_url`), which need the
/// path itself, not the decoded image data.
#[cfg(not(test))]
#[must_use]
pub fn image_disk_cache_path(key: &str) -> Option<PathBuf> {
    let path = image_disk_cache_dir().join(image_cache_filename(key));
    if !path.is_file() {
        return None;
    }
    touch_image_disk_cache(&path);
    Some(path)
}

/// Stores `bytes` as the cache entry for `key`. Best effort: failures are
/// ignored because a miss only costs a later fetch.
pub fn write_image_disk_cache(key: &str, bytes: &[u8]) {
    let dir = image_disk_cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(image_cache_filename(key)), bytes);
}

/// Removes every cache entry whose filename starts with `prefix`'s path-safe
/// form. Best effort: failures are ignored.
pub fn clear_image_disk_cache_prefix(prefix: &str) {
    let Ok(entries) = std::fs::read_dir(image_disk_cache_dir()) else {
        return;
    };
    let prefix = safe_cache_filename(prefix);
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Spawns a background sweep that deletes every entry whose mtime is older
/// than 30 days, so the cache reclaims the files of the previous generation.
#[cfg(not(test))]
pub fn evict_old_image_cache() {
    std::thread::spawn(|| {
        let dir = image_disk_cache_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let cutoff = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_hours(720))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata()
                && meta.modified().is_ok_and(|m| m < cutoff)
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    });
}

/// Best-effort mtime refresh marking a cache file as recently used, so the
/// 30-day mtime eviction (`evict_old_image_cache`) measures last use rather
/// than first write. Reads never update mtime on their own, so without this
/// every regularly-viewed image still ages out and the cache wipes itself.
/// Throttled to one write per file per day; all failures ignored.
fn touch_image_disk_cache(path: &std::path::Path) {
    let stale = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .is_ok_and(|modified| {
            std::time::SystemTime::now()
                .duration_since(modified)
                .is_ok_and(|age| age.as_secs() >= 24 * 3600)
        });
    if stale {
        // Read-only open: refreshing mtime needs no write access to the file.
        if let Ok(file) = std::fs::File::open(path) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
    }
}

fn safe_cache_filename(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
