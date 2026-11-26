//! The backdrop worker (design D4 of `gui-hero-design-polish`): one long-lived
//! thread that encodes backdrop JPEGs off the core thread, and the bounded cache
//! of what it encoded. A repeated request for one cache key (a re-selected
//! Hero, a window reload) is answered from the cache, so the decode, blur,
//! and encode run once per key and the thread count never grows with the
//! request count.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

use super::{BackdropRequest, backdrop_jpeg};
use crate::art::{ArtReply, ArtResponse};

#[cfg(test)]
mod tests;

/// The most backdrop JPEGs the cache holds; the oldest entry leaves first. A backdrop
/// JPEG is at most 64 px on its long side, so the bound is a few hundred KB.
const BACKDROP_CACHE_CAPACITY: usize = 32;

/// The bounded first-in-first-out cache of encoded backdrop JPEGs, by cache key.
#[derive(Debug, Default)]
struct BackdropCache {
    entries: HashMap<String, Arc<[u8]>>,
    order: VecDeque<String>,
}

impl BackdropCache {
    fn get(&self, key: &str) -> Option<Arc<[u8]>> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: String, jpeg: Arc<[u8]>) {
        if self.entries.insert(key.clone(), jpeg).is_some() {
            return;
        }
        self.order.push_back(key);
        while self.order.len() > BACKDROP_CACHE_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

/// The one-shot art reply edge that answers `Missing` when its job never
/// runs it: a failed send drops the job, a panic in the backdrop work drops the
/// guard, and a `None` from [`backdrop_jpeg`] leaves the guard untouched — the
/// drop answers in each case, so every request is answered exactly once.
struct BackdropReply(Option<ArtReply>);

impl BackdropReply {
    fn answer(&mut self, jpeg: &[u8]) {
        if let Some(reply) = self.0.take() {
            reply(ArtResponse::Found {
                mime: "image/jpeg",
                bytes: jpeg.to_vec(),
            });
        }
    }
}

impl Drop for BackdropReply {
    fn drop(&mut self) {
        if let Some(reply) = self.0.take() {
            reply(ArtResponse::Missing);
        }
    }
}

/// One encode request: the cache key, its cached source bytes, and the reply
/// edge.
struct BackdropJob {
    request: BackdropRequest,
    source: Vec<u8>,
    reply: BackdropReply,
}

/// The handle the core thread keeps: the shared cache and the job channel.
#[derive(Debug)]
pub(crate) struct BackdropWorker {
    cache: Arc<Mutex<BackdropCache>>,
    jobs: Sender<BackdropJob>,
}

impl BackdropWorker {
    /// Start the `mbv-art-backdrop` thread. A failed spawn leaves a channel with
    /// no receiver, so every [`Self::submit`] answers `Missing`.
    pub(crate) fn spawn() -> Self {
        let cache = Arc::new(Mutex::new(BackdropCache::default()));
        let (jobs, inbox) = channel::<BackdropJob>();
        let worker_cache = Arc::clone(&cache);
        let _detached = std::thread::Builder::new()
            .name("mbv-art-backdrop".into())
            .spawn(move || {
                for mut job in inbox {
                    if let Some(jpeg) = encode_once(&worker_cache, &job) {
                        job.reply.answer(&jpeg);
                    }
                    // No JPEG: the guard's drop answers `Missing`.
                }
            });
        Self { cache, jobs }
    }

    /// The cached backdrop JPEG for `key`, so the core thread answers a repeat
    /// request without reading or copying the source bytes.
    pub(crate) fn cached(&self, request: &BackdropRequest) -> Option<Arc<[u8]>> {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&request.cache_id())
    }

    /// Queue one encode. The reply is answered once, by the worker or by the
    /// guard's drop.
    pub(crate) fn submit(&self, request: BackdropRequest, source: Vec<u8>, reply: ArtReply) {
        // A failed send returns the job; dropping it answers `Missing`.
        let _ = self.jobs.send(BackdropJob {
            request,
            source,
            reply: BackdropReply(Some(reply)),
        });
    }
}

/// The job's backdrop JPEG: the cached one when an earlier job for the same key
/// encoded it, else a fresh encode that enters the cache. `None` when the
/// source does not decode.
fn encode_once(cache: &Mutex<BackdropCache>, job: &BackdropJob) -> Option<Arc<[u8]>> {
    let cached = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&job.request.cache_id());
    if cached.is_some() {
        return cached;
    }
    let jpeg: Arc<[u8]> = backdrop_jpeg(&job.source, job.request.width, job.request.style)?.into();
    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(job.request.cache_id(), Arc::clone(&jpeg));
    Some(jpeg)
}
