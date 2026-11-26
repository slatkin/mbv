//! Headless tests of the art painter (D16): the contract of task 3.6 — a
//! failed URL paints the fill, and after the recording clock fires, the `img`
//! is recreated and Blitz asks for the URL again. The store, the timer router,
//! the clock, and the scale factor travel in as the root's contexts, so no
//! test opens a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use blitz_dom::{Document as _, local_name};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{ArtEvents, ArtImage};
use crate::art::{GuiArtRef, art_url};
use crate::window::clock::{Clock, FiredTimers, RecordingClock};

/// The reference a mounted `ArtImage` paints: the cached image the URL names.
fn cover() -> GuiArtRef {
    GuiArtRef::Image {
        src: art_url("card/cover"),
    }
}

/// The props a test hands its document: the art to paint and the recording
/// clock the component consumes. The store and the timer router the root's
/// drain task would forward into are created in the root (their signals need
/// its scope) and handed back out through the handle.
#[derive(Clone)]
struct FixtureProps {
    art: Option<GuiArtRef>,
    clock: Arc<RecordingClock>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The context handles the root hands the test: the store, the timer router,
/// and the art reference the fixture paints.
type FixtureHandles = (ArtEvents, FiredTimers, Signal<Option<GuiArtRef>>);

fn art_app(props: FixtureProps) -> Element {
    let FixtureProps { art, clock, handle } = props;
    let art = use_signal(move || art);
    let events = use_hook(ArtEvents::new);
    let fired = use_hook(FiredTimers::new);
    provide_context(events);
    provide_context(clock as Arc<dyn Clock>);
    provide_context(fired);
    handle.borrow_mut().replace((events, fired, art));
    let current = art.cloned();
    rsx! {
        div {
            ArtImage {
                art: current,
                alt: "cover",
                fallback: rsx! { "FILL" },
            }
        }
    }
}

/// A document whose root mounts one `ArtImage` over the shared handles.
struct Fixture {
    doc: DioxusDocument,
    /// The `ArtEvents` copy the test drives (the store is `Copy`).
    events: ArtEvents,
    /// The `FiredTimers` copy the test drives.
    fired: FiredTimers,
    /// The art reference the fixture paints, so a test changes the URL.
    art: Signal<Option<GuiArtRef>>,
    clock: Arc<RecordingClock>,
}

impl Fixture {
    /// The `src` the `img` element paints, `None` while the fill paints.
    fn painted(&self) -> Option<String> {
        let dom = self.doc.inner();
        let node = dom.query_selector("img").expect("the selector is valid")?;
        let node = dom.get_node(node).expect("the node is in the document");
        node.attr(local_name!("src")).map(str::to_owned)
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed.
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// Run `f` inside the document's runtime, as the root's drain task does:
    /// a write outside it reaches no subscriber.
    fn in_runtime(&mut self, f: impl FnOnce()) {
        self.doc.vdom.in_runtime(f);
    }
}

fn built(art: Option<GuiArtRef>) -> Fixture {
    let clock = Arc::new(RecordingClock::new());
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            art_app,
            FixtureProps {
                art,
                clock: Arc::clone(&clock),
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let (events, fired, art) = handle
        .borrow()
        .as_ref()
        .expect("the root handed the store and the timer router over")
        .to_owned();
    Fixture {
        doc,
        events,
        fired,
        art,
        clock,
    }
}

/// A deadline far past every timer the test arms, for
/// [`RecordingClock::fire_due`].
fn long_past() -> Instant {
    Instant::now() + Duration::from_secs(3600)
}

/// Contract (task 3.6): the image paints first, a `Failed` answer paints the
/// fill, and when the recording clock fires the armed retry the `img` is
/// recreated with the same `src` — so Blitz requests the URL again (D7).
#[test]
fn a_failed_url_paints_the_fill_until_the_retry_fires() {
    let mut fixture = built(Some(cover()));
    fixture.poll_until_idle();
    let src = art_url("card/cover");
    assert_eq!(fixture.painted().as_deref(), Some(src.as_str()));

    // The miss: the fill paints and the retry arms at 1 s.
    fixture
        .events
        .push(crate::window::net::ArtEvent::Failed { url: src.clone() });
    fixture.poll_until_idle();
    assert_eq!(fixture.painted(), None, "the fill paints while waiting");

    let armed = fixture.clock.pending();
    assert_eq!(armed, 1, "the retry armed one timer");
    let fired_ids = fixture.clock.fire_due(long_past());
    assert_eq!(fired_ids.len(), 1);

    // The root's drain task forwards the passed timer to its surface.
    let fired = fixture.fired;
    fixture.in_runtime(|| {
        let mut fired = fired;
        for id in fired_ids {
            fired.push(id);
        }
    });
    fixture.poll_until_idle();

    assert_eq!(
        fixture.painted().as_deref(),
        Some(src.as_str()),
        "the img is recreated with the same src"
    );
}

/// Contract (D7): a `loading` reference paints the fill — it names no URL.
#[test]
fn a_loading_reference_paints_the_fill() {
    let mut fixture = built(Some(GuiArtRef::Loading));
    fixture.poll_until_idle();
    assert_eq!(fixture.painted(), None);
}

/// The `style` attribute the painted `img` carries, `None` while the fill
/// paints.
fn img_style(fixture: &Fixture) -> Option<String> {
    let dom = fixture.doc.inner();
    let node = dom.query_selector("img").expect("the selector is valid")?;
    let node = dom.get_node(node).expect("the node is in the document");
    node.attr(local_name!("style")).map(str::to_owned)
}

/// Contract (D7): art never paints above its own pixels — a URL change
/// re-reads the store, so an already-cached URL (Blitz answers it from its
/// image cache without a fetch, so no new `Loaded` arrives) still caps at its
/// own natural size. The switch paints `second`, whose `Loaded` the store
/// already holds from an earlier load.
#[test]
fn a_url_change_re_reads_the_natural_size_of_the_new_url() {
    let mut fixture = built(Some(GuiArtRef::Image {
        src: art_url("card/first"),
    }));
    fixture.poll_until_idle();
    let loaded = |url: &str, width: u32, height: u32| crate::window::net::ArtEvent::Loaded {
        url: url.to_owned(),
        width,
        height,
    };
    let first = art_url("card/first");
    let second = art_url("card/second");
    let mut events = fixture.events;
    fixture.in_runtime(|| {
        events.push(loaded(&first, 400, 300));
        events.push(loaded(&second, 100, 50));
    });
    fixture.poll_until_idle();
    assert!(
        img_style(&fixture).is_some_and(|style| style.contains("max-width: 400px")),
        "{:?}",
        img_style(&fixture)
    );

    // The switch to a URL the store already holds: Blitz serves it from its
    // image cache without a fetch, so the store's own `Loaded` size is the
    // only fact the cap can come from.
    let mut art = fixture.art;
    fixture.in_runtime(|| {
        art.set(Some(GuiArtRef::Image {
            src: second.clone(),
        }));
    });
    fixture.poll_until_idle();

    assert_eq!(fixture.painted().as_deref(), Some(second.as_str()));
    assert!(
        img_style(&fixture).is_some_and(|style| style.contains("max-width: 100px")),
        "{:?}",
        img_style(&fixture)
    );
}
