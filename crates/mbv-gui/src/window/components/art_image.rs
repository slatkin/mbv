//! The window's one art painter (`replace-tauri-with-blitz` D7, the port of
//! `ArtImage`): every `GuiArtRef` paints as an `img` whose `src` is the
//! reference's URL. A `loading` reference paints no image, and a missed load
//! paints the same loading fill; a failed load becomes
//! [`ArtEvent::Failed`] for the painted URL, and the natural size
//! (`capToNaturalSize`, `window/natural_size.rs`) comes from
//! [`ArtEvent::Loaded`] divided by the window's scale factor. The fill then
//! retries the same URL with capped backoff while mounted: the `mbv-art` miss
//! is a failed load, so a retry asks the core again (D7). A new URL starts a
//! fresh load. While the natural size is still unreported, the caller's
//! `bounds` cap the `img` provisionally, so its first painted frame is
//! already the final size — an `img` never paints above its own pixels, so
//! the bounds alone land on the same layout the loaded cap gives (the
//! 2026-11-20 hero flash).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::window::clock::{Clock, FiredTimers, TimerId};
use crate::window::natural_size::{NaturalCap, NaturalSizeBounds, capped_natural_size};
use crate::window::net::{ArtEvent, ArtStatus};

/// The first retry's wait; each further failure doubles it (D7).
const RETRY_BASE_MS: u64 = 1000;
/// The longest wait between two retries of one URL (D7).
const RETRY_CAP_MS: u64 = 30_000;

/// The art events the window drained (D7): the root's drain task forwards
/// every art answer here, and each mounted `ArtImage` reads the events for its
/// own URL. A [`ArtEvent::Loaded`] sets the URL's natural size; each
/// [`ArtEvent::Failed`] counts one more miss since the URL's last load.
#[derive(Clone, Copy, Debug)]
pub struct ArtEvents {
    statuses: Signal<HashMap<String, ArtStatus>>,
}

impl Default for ArtEvents {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtEvents {
    /// A store with no answers yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            statuses: Signal::new(HashMap::new()),
        }
    }

    /// One art answer arrived (D7): the root's drain task forwards it.
    pub fn push(&mut self, event: ArtEvent) {
        let mut statuses = self.statuses.write();
        match event {
            ArtEvent::Loaded { url, width, height } => {
                statuses.insert(url, ArtStatus::Loaded { width, height });
            }
            ArtEvent::Failed { url } => {
                let misses = match statuses.get(&url) {
                    Some(ArtStatus::Failed { misses }) => misses + 1,
                    _ => 1,
                };
                statuses.insert(url, ArtStatus::Failed { misses });
            }
        }
    }

    /// The last fact `url` reported, `None` while it reported nothing.
    #[must_use]
    pub fn status(&self, url: &str) -> Option<ArtStatus> {
        self.statuses.cloned().get(url).copied()
    }

    /// The last fact `url` reported without subscribing — the source-change
    /// reset reads it while it must not re-run on an art answer.
    #[must_use]
    pub fn status_peek(&self, url: &str) -> Option<ArtStatus> {
        self.statuses.peek().get(url).copied()
    }
}

/// The window's scale factor (D7): the root seeds it from `use_window()` and
/// rewrites it on every viewport change, so a `ScaleFactorChanged` recomputes
/// the natural-size caps. A context value, so a headless test injects one and
/// the component needs no window handle.
#[derive(Clone, Copy, Debug)]
pub struct ScaleFactor(pub Signal<f64>);

/// The load that failed: its URL, how many times it failed in a row since its
/// source changed, and whether its retry still waits.
#[derive(Debug, Clone, PartialEq)]
struct Failure {
    src: String,
    count: u64,
    waiting: bool,
}

/// The wait before the retry after the `count`-th failure in a row (D7): 1 s
/// doubling to the 30 s cap.
fn retry_delay(count: u64) -> Duration {
    let factor = 2u64.saturating_pow(u32::try_from(count.saturating_sub(1)).unwrap_or(u32::MAX));
    Duration::from_millis(RETRY_BASE_MS.saturating_mul(factor).min(RETRY_CAP_MS))
}

/// The props of the art painter (the `ArtImage` props, in order).
#[derive(Props, Clone, PartialEq)]
pub struct ArtImageProps {
    /// The reference the section carries, or `None` while the item has none.
    pub art: Option<GuiArtRef>,
    /// The image's alternative text; `""` for decorative art.
    pub alt: String,
    /// The box the image also stays inside, as `capToNaturalSize` takes it.
    #[props(default)]
    pub bounds: Option<NaturalSizeBounds>,
    /// Whether the image fills its box with no natural-size cap (the former client
    /// `bounds={false}`), as the queue still slot scales the artwork to the
    /// panel's width.
    #[props(default)]
    pub uncapped: bool,
    /// The `img` class names.
    #[props(default)]
    pub class: String,
    /// The `img` inline style.
    #[props(default)]
    pub style: String,
    /// The loading fill: painted while the reference has no URL, the image
    /// fails to load, or its retry waits.
    #[props(default)]
    pub fallback: Option<Element>,
}

impl std::fmt::Debug for ArtImageProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtImageProps")
            .field("art", &self.art)
            .field("alt", &self.alt)
            .field("bounds", &self.bounds)
            .field("uncapped", &self.uncapped)
            .field("class", &self.class)
            .field("style", &self.style)
            .finish_non_exhaustive()
    }
}

/// The painter's local state: the failure record and its armed retry, the
/// misses the source change last baselined, and the loaded natural size. The
/// signals need the component's scope, so the component builds one and the
/// effect bodies below take it by copy.
#[derive(Clone, Copy)]
struct ArtImageState {
    /// The load that failed, with its count and whether its retry waits.
    failure: Signal<Option<Failure>>,
    /// The armed retry timer, `None` while no retry waits.
    retry_timer: Signal<Option<TimerId>>,
    /// The misses of the painted URL the component last recorded.
    seen_misses: Signal<u64>,
    /// The misses of the painted URL when its source last changed: the count
    /// of the current failure streak is `seen - base`, as the former failure
    /// reset on a source change did.
    misses_base: Signal<u64>,
    /// The painted URL's natural size, in device pixels, from `Loaded`.
    natural: Signal<Option<(u32, u32)>>,
}

/// A new URL starts a fresh load: drop the old
/// failure and its pending retry, and baseline the misses the new source
/// already carries, so its old misses do not count into the new streak.
fn drop_old_failure(
    mut state: ArtImageState,
    art_events: &ArtEvents,
    clock: &dyn Clock,
    url: &str,
) {
    cancel_armed(state, clock);
    if state.failure.peek().is_some() {
        state.failure.set(None);
    }
    if state.natural.peek().is_some() {
        state.natural.set(None);
    }
    let misses = match art_events.status_peek(url) {
        Some(ArtStatus::Failed { misses }) => misses,
        _ => 0,
    };
    state.seen_misses.set(misses);
    state.misses_base.set(misses);
}

/// Cancel the armed retry, if one waits.
fn cancel_armed(mut state: ArtImageState, clock: &dyn Clock) {
    let armed = *state.retry_timer.peek();
    if let Some(timer) = armed {
        clock.cancel(timer);
        state.retry_timer.set(None);
    }
}

/// A failed load of the painted URL: record the failure — the count of the
/// current streak — and arm its retry, whose wait doubles from 1 s to the 30 s
/// cap.
fn record_failure(mut state: ArtImageState, art_events: &ArtEvents, clock: &dyn Clock, url: &str) {
    let Some(ArtStatus::Failed { misses }) = art_events.status(url) else {
        return;
    };
    if state.seen_misses.cloned() >= misses {
        return;
    }
    state.seen_misses.set(misses);
    let count = misses.saturating_sub(state.misses_base.cloned());
    state.failure.set(Some(Failure {
        src: url.to_owned(),
        count,
        waiting: true,
    }));
    cancel_armed(state, clock);
    let timer = clock.after(retry_delay(count));
    state.retry_timer.set(Some(timer));
}

/// The retry: the armed timer fired and the root forwarded its id, so the wait
/// ends and the `img` is recreated with the same `src`.
fn clear_waiting(mut state: ArtImageState, fired: &mut FiredTimers, timer: TimerId) {
    if !fired.take(timer) {
        return;
    }
    let current = state.failure.peek().clone();
    if let Some(current) = current {
        state.failure.set(Some(Failure {
            waiting: false,
            ..current
        }));
    }
    state.retry_timer.set(None);
}

/// The natural size (D7): the `Loaded` answer carries it, in device pixels.
fn loaded_natural(mut state: ArtImageState, art_events: &ArtEvents, url: &str) {
    if let Some(ArtStatus::Loaded { width, height }) = art_events.status(url) {
        state.natural.set(Some((width, height)));
    }
}

/// The URL to paint, `None` while the fill paints.
fn painted_url(src: Option<String>, failure: Option<Failure>) -> Option<String> {
    match (src, failure) {
        (Some(url), Some(current)) if current.waiting && current.src == url => None,
        (Some(url), _) => Some(url),
        (None, _) => None,
    }
}

/// The `img` inline style with the natural-size cap appended, in CSS pixels
/// (D7): the loaded size divided by the window's scale factor. With bounds the
/// image also never outgrows its parent's width. While the natural size is
/// still unreported the bounds cap provisionally: an `img` never paints above
/// its own pixels, so the bounds alone land on the same layout the loaded cap
/// gives, and the first painted frame is already the final size (the
/// 2026-11-20 hero flash: between the bytes' arrival and the cap's, an
/// uncapped `img` painted at its intrinsic size).
fn capped_style(style: &str, cap: Option<NaturalCap>, bounds: Option<NaturalSizeBounds>) -> String {
    let mut style = style.to_owned();
    if let Some(cap) = cap {
        if bounds.is_some() {
            let _ = write!(style, "max-width: min({}px, 100%); ", cap.max_width);
        } else {
            let _ = write!(style, "max-width: {}px; ", cap.max_width);
        }
        let _ = write!(style, "max-height: {}px;", cap.max_height);
    } else if let Some(bounds) = bounds {
        match bounds.width {
            Some(width) => {
                let _ = write!(style, "max-width: min({width}px, 100%); ");
            }
            None => {
                let _ = write!(style, "max-width: 100%; ");
            }
        }
        if let Some(height) = bounds.height {
            let _ = write!(style, "max-height: {height}px;");
        }
    }
    style
}

/// The art painter (D7). The `img` element carries no error or load element
/// handler (D8): a failure arrives only as [`ArtEvent::Failed`], and a load
/// only as [`ArtEvent::Loaded`]. The retry keeps the former rules: the fill
/// paints, the wait starts at 1 s and doubles to the 30 s cap, and when it
/// ends the `img` element is recreated with the same `src`, so Blitz requests
/// the URL again.
#[component]
pub fn ArtImage(props: ArtImageProps) -> Element {
    let state = ArtImageState {
        failure: use_signal(|| None),
        retry_timer: use_signal(|| None),
        seen_misses: use_signal(|| 0),
        misses_base: use_signal(|| 0),
        natural: use_signal(|| None),
    };

    let art_events = consume_context::<ArtEvents>();
    let mut fired = consume_context::<FiredTimers>();
    let clock = consume_context::<Arc<dyn Clock>>();
    let scale = consume_context::<ScaleFactor>();

    // The owned URL: the effects below capture it, and a `loading` reference
    // or a missing one paints the fill (it names no URL).
    let src = props
        .art
        .as_ref()
        .and_then(GuiArtRef::src)
        .map(str::to_owned);

    // A new URL starts a fresh load: drop the old failure and its pending
    // retry. Unmount cancels the same timer.
    let url = src.clone();
    let clock_reset = Arc::clone(&clock);
    use_effect(use_reactive((&url,), move |(url,)| {
        drop_old_failure(
            state,
            &art_events,
            &*clock_reset,
            url.as_deref().unwrap_or(""),
        );
    }));

    // A failed load of the painted URL arms its retry; the fill paints until
    // the retry timer clears the wait.
    let url = src.clone();
    let clock_arm = Arc::clone(&clock);
    use_effect(move || {
        let Some(url) = url.clone() else {
            return;
        };
        record_failure(state, &art_events, &*clock_arm, &url);
    });

    // The retry: the armed timer fired and the root forwarded its id, so the
    // wait ends and the `img` is recreated with the same `src`.
    use_effect(move || {
        let Some(timer) = state.retry_timer.cloned() else {
            return;
        };
        clear_waiting(state, &mut fired, timer);
    });

    // The natural size (D7): the `Loaded` answer carries it, in device pixels.
    // The effect reads the URL reactively too: Blitz serves an already-cached
    // URL from its image cache without a fetch, so no new `Loaded` arrives —
    // the URL change itself must re-read the store, or the cap is lost.
    let url = src.clone();
    use_effect(use_reactive((&url,), move |(url,)| {
        loaded_natural(state, &art_events, url.as_deref().unwrap_or(""));
    }));

    // No retry outlives the box.
    use_drop(move || {
        let clock = Arc::clone(&clock);
        cancel_armed(state, &*clock);
    });

    let painted = painted_url(src, state.failure.cloned());

    // The natural-size cap in CSS pixels: the loaded size divided by the
    // window's scale factor, recomputed when either changes (D7).
    let cap = state
        .natural
        .cloned()
        .filter(|_| !props.uncapped)
        .and_then(|(width, height)| {
            capped_natural_size(width, height, scale.0.cloned(), props.bounds)
        });
    // The provisional bounds cap while the natural size is unreported; an
    // uncapped image takes its caller's style alone (`cap` is already `None`
    // for it, but its bounds must not cap it either).
    let style = capped_style(&props.style, cap, props.bounds.filter(|_| !props.uncapped));

    rsx! {
        if let Some(painted) = painted {
            img { class: props.class, style: "{style}", src: painted, alt: props.alt }
        } else if let Some(fallback) = props.fallback {
            {fallback}
        }
    }
}

#[cfg(test)]
mod tests;
