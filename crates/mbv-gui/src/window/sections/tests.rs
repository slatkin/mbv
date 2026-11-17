//! Headless tests of the section stores (D16): the contracts of
//! `sections.test.ts` that task 3.1 names — a section at its held revision
//! is ignored and a section at any other revision is written, a batch writes
//! all its sections in one pass, and a section the batch does not name
//! stays. The harness builds a `DioxusDocument` whose root applies the
//! fixture batches in one effect and paints the store values as text, so no
//! test opens a window or sleeps.

use std::cell::RefCell;
use std::rc::Rc;

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{Sections, fresh_footer, fresh_queue, fresh_summary};
use crate::bridge::frame::chrome::{GuiFooterPill, GuiQueueFooter};
use crate::bridge::frame::queue::{GuiQueue, GuiQueueSummary};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};

/// A `summary` section batch at `revision` carrying `remaining`.
fn summary_batch(revision: u64, remaining: usize) -> GuiSectionBatch {
    GuiSectionBatch {
        summary: Some(GuiSection {
            revision,
            value: GuiQueueSummary {
                remaining,
                ..fresh_summary()
            },
        }),
        ..GuiSectionBatch::default()
    }
}

/// A `queue` section batch at `revision` carrying `cursor`.
fn queue_batch(revision: u64, cursor: Option<u64>) -> GuiSectionBatch {
    GuiSectionBatch {
        queue: Some(GuiSection {
            revision,
            value: GuiQueue {
                cursor,
                ..fresh_queue()
            },
        }),
        ..GuiSectionBatch::default()
    }
}

/// A `footer` section batch at `revision` carrying `source` as its playlist
/// text.
fn footer_batch(revision: u64, source: &str) -> GuiSectionBatch {
    GuiSectionBatch {
        footer: Some(GuiSection {
            revision,
            value: GuiQueueFooter {
                playlist: GuiFooterPill {
                    text: source.to_owned(),
                    ..fresh_footer().playlist
                },
                ..fresh_footer()
            },
        }),
        ..GuiSectionBatch::default()
    }
}

/// The root: creates the stores once, applies the fixture batches in one
/// effect, and paints the facts the assertions read.
fn sections_app(batches: Vec<GuiSectionBatch>) -> Element {
    let sections = use_hook(Sections::new);
    let mut applied = sections.clone();
    use_effect(move || {
        for batch in &batches {
            applied.apply_batch(batch);
        }
    });
    let cursor = sections.queue.read().cursor;
    let remaining = sections.summary.read().remaining;
    let source = sections.footer.read().playlist.text.clone();
    rsx! {
        div { "cursor={cursor:?} remaining={remaining} source={source}" }
    }
}

/// The root that hands its stores to the test, so the test reads the same
/// signals a batch writes.
fn stores_app(props: StoreProps) -> Element {
    let sections = use_hook(Sections::new);
    let stores = props.stores;
    stores.borrow_mut().replace(sections);
    rsx! {
        div {}
    }
}

/// The props of [`stores_app`]: where the root puts its stores.
#[derive(Clone)]
struct StoreProps {
    stores: Rc<RefCell<Option<Sections>>>,
}

/// A document whose root applied `batches`, and the text it paints.
fn rendered(batches: Vec<GuiSectionBatch>) -> String {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(sections_app, batches),
        DocumentConfig::default(),
    );
    doc.initial_build();
    poll_until_idle(&mut doc);
    main_text(&doc)
}

/// Poll the document until its effects have run and their re-renders landed.
fn poll_until_idle(doc: &mut DioxusDocument) {
    for _ in 0..8 {
        if !doc.poll(None) {
            return;
        }
    }
    panic!("document did not become idle");
}

/// The text the root paints, under the document's `main` element.
fn main_text(doc: &DioxusDocument) -> String {
    let inner = doc.inner();
    let node = inner
        .query_selector("div")
        .expect("the selector is valid")
        .expect("the root div exists");
    let mut text = String::new();
    let mut stack = vec![node];
    while let Some(id) = stack.pop() {
        let node = inner.get_node(id).expect("the node is in the document");
        if let Some(data) = node.text_data() {
            text.push_str(&data.content);
        }
        stack.extend(node.children.iter().rev().copied());
    }
    text
}

/// Contract 1, ignored half (task 3.1): a re-sent section whose revision the
/// window holds — a `sync` snapshot's copy — writes nothing, even though the
/// carried value differs.
#[test]
fn a_section_at_its_held_revision_is_ignored() {
    let text = rendered(vec![summary_batch(7, 3), summary_batch(7, 9)]);
    assert!(text.contains("remaining=3"), "{text}");
}

/// Contract 1, written half (task 3.1): a section at any other revision —
/// lower or higher — is written.
#[rstest::rstest]
#[case::lower_revision(6)]
#[case::higher_revision(8)]
fn a_section_at_any_other_revision_is_written(#[case] revision: u64) {
    let text = rendered(vec![summary_batch(7, 3), summary_batch(revision, 5)]);
    assert!(text.contains("remaining=5"), "{text}");
}

/// Contract 2 (task 3.1): one batch carries several sections, and all of
/// them are written — in one pass, the single render ADR 0038 requires.
#[test]
fn a_batch_writes_all_its_sections_in_one_pass() {
    let mut batch = summary_batch(7, 3);
    batch.queue = queue_batch(1, Some(2)).queue;
    batch.footer = footer_batch(1, "mix").footer;

    let text = rendered(vec![batch]);

    assert!(text.contains("cursor=Some(2)"), "{text}");
    assert!(text.contains("remaining=3"), "{text}");
    assert!(text.contains("source=mix"), "{text}");
}

/// Contract (`share-gui-sections-by-rc` task 1.3, the spec's "A repaint
/// copies no section content"): a batch that does not name a section leaves
/// that section's handle as it was, so the read before and the read after it
/// are pointer-equal — no repaint copies the section's content. A batch that
/// carries a new revision of the section writes a new handle.
#[test]
fn a_batch_that_does_not_name_the_queue_keeps_its_handle() {
    let stores = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            stores_app,
            StoreProps {
                stores: Rc::clone(&stores),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    poll_until_idle(&mut doc);
    let sections = stores
        .borrow()
        .clone()
        .expect("the root provides its stores");
    let mut applied = sections.clone();
    doc.vdom
        .in_runtime(move || applied.apply_batch(&summary_batch(7, 3)));
    let before = sections.queue.peek().clone();
    let mut applied = sections.clone();
    doc.vdom
        .in_runtime(move || applied.apply_batch(&summary_batch(8, 5)));
    let after = sections.queue.peek().clone();
    assert_eq!(
        before, after,
        "a batch that named only the summary rebuilt the queue handle"
    );
    let mut applied = sections.clone();
    doc.vdom
        .in_runtime(move || applied.apply_batch(&queue_batch(1, Some(2))));
    assert_ne!(
        after,
        sections.queue.peek().clone(),
        "a new queue revision kept the old queue handle"
    );
}

/// Contract 3 (task 3.1): a batch carries only the sections that changed —
/// a section it does not name keeps its value.
#[test]
fn a_section_the_batch_does_not_name_stays() {
    let text = rendered(vec![summary_batch(7, 3), queue_batch(1, Some(2))]);
    assert!(text.contains("cursor=Some(2)"), "{text}");
    assert!(text.contains("remaining=3"), "{text}");
}
