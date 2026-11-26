//! The glow worker (design D4 of `gui-hero-design-polish`): one long-lived
//! thread that encodes glow PNGs off the core thread, and the bounded cache
//! of what it encoded. A repeated request for one cache key (a re-selected
//! Hero, a window reload) is answered from the cache, so the decode, blur,
//! and encode run once per key and the thread count never grows with the
//! request count.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

use super::glow_png;
use crate::art::{ArtReply, ArtResponse};

#[cfg(test)]
mod tests;

/// The most glow PNGs the cache holds; the oldest entry leaves first. A glow
/// PNG is at most 64 px on its long side, so the bound is a few hundred KB.
const GLOW_CACHE_CAPACITY: usize = 32;

/// The bounded first-in-first-out cache of encoded glow PNGs, by cache key.
#[derive(Debug, Default)]
struct GlowCache {
    entries: HashMap<String, Arc<[u8]>>,
    order: VecDeque<String>,
}

impl GlowCache {
    fn get(&self, key: &str) -> Option<Arc<[u8]>> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: String, png: Arc<[u8]>) {
        if self.entries.insert(key.clone(), png).is_some() {
            return;
        }
        self.order.push_back(key);
        while self.order.len() > GLOW_CACHE_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

/// The one-shot art reply edge that answers `Missing` when its job never
/// runs it: a failed send drops the job, a panic in the glow work drops the
/// guard, and a `None` from [`glow_png`] leaves the guard untouched — the
/// drop answers in each case, so every request is answered exactly once.
struct GlowReply(Option<ArtReply>);

impl GlowReply {
    fn answer(&mut self, png: &[u8]) {
        if let Some(reply) = self.0.take() {
            reply(ArtResponse::Found {
                mime: "image/png",
                bytes: png.to_vec(),
            });
        }
    }
}

impl Drop for GlowReply {
    fn drop(&mut self) {
        if let Some(reply) = self.0.take() {
            reply(ArtResponse::Missing);
        }
    }
}

/// One encode request: the cache key, its cached source bytes, and the reply
/// edge.
struct GlowJob {
    key: String,
    source: Vec<u8>,
    reply: GlowReply,
}

/// The handle the core thread keeps: the shared cache and the job channel.
#[derive(Debug)]
pub(crate) struct GlowWorker {
    cache: Arc<Mutex<GlowCache>>,
    jobs: Sender<GlowJob>,
}

impl GlowWorker {
    /// Start the `mbv-art-glow` thread. A failed spawn leaves a channel with
    /// no receiver, so every [`Self::submit`] answers `Missing`.
    pub(crate) fn spawn() -> Self {
        let cache = Arc::new(Mutex::new(GlowCache::default()));
        let (jobs, inbox) = channel::<GlowJob>();
        let worker_cache = Arc::clone(&cache);
        let _detached = std::thread::Builder::new()
            .name("mbv-art-glow".into())
            .spawn(move || {
                for mut job in inbox {
                    if let Some(png) = encode_once(&worker_cache, &job) {
                        job.reply.answer(&png);
                    }
                    // No PNG: the guard's drop answers `Missing`.
                }
            });
        Self { cache, jobs }
    }

    /// The cached glow PNG for `key`, so the core thread answers a repeat
    /// request without reading or copying the source bytes.
    pub(crate) fn cached(&self, key: &str) -> Option<Arc<[u8]>> {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key)
    }

    /// Queue one encode. The reply is answered once, by the worker or by the
    /// guard's drop.
    pub(crate) fn submit(&self, key: String, source: Vec<u8>, reply: ArtReply) {
        // A failed send returns the job; dropping it answers `Missing`.
        let _ = self.jobs.send(GlowJob {
            key,
            source,
            reply: GlowReply(Some(reply)),
        });
    }
}

/// The job's glow PNG: the cached one when an earlier job for the same key
/// encoded it, else a fresh encode that enters the cache. `None` when the
/// source does not decode.
fn encode_once(cache: &Mutex<GlowCache>, job: &GlowJob) -> Option<Arc<[u8]>> {
    let cached = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&job.key);
    if cached.is_some() {
        return cached;
    }
    let png: Arc<[u8]> = glow_png(&job.source)?.into();
    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(job.key.clone(), Arc::clone(&png));
    Some(png)
}
