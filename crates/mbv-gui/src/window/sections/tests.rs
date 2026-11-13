//! Headless tests of the section stores (D16): the contracts of
//! `sections.test.ts` that task 3.1 names — a section at its held revision
//! is ignored and a section at any other revision is written, a batch writes
//! all its sections in one pass, and a section the batch does not name
//! stays. The harness builds a `DioxusDocument` whose root applies the
//! fixture batches in one effect and paints the store values as text, so no
//! test opens a window or sleeps.

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::Sections;
use crate::bridge::frame::chrome::GuiQueueFooter;
use crate::bridge::frame::queue::{GuiQueue, GuiQueueSummary};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};

/// A `summary` section batch at `revision` carrying `remaining`.
fn summary_batch(revision: u64, remaining: usize) -> GuiSectionBatch {
    GuiSectionBatch {
        summary: Some(GuiSection {
            revision,
            value: GuiQueueSummary {
                remaining,
                ..summary_fresh()
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

/// A `footer` section batch at `revision` carrying `owner_live`.
fn footer_batch(revision: u64, owner_live: bool) -> GuiSectionBatch {
    GuiSectionBatch {
        footer: Some(GuiSection {
            revision,
            value: GuiQueueFooter {
                owner_live,
                ..fresh_footer()
            },
        }),
        ..GuiSectionBatch::default()
    }
}

/// A fresh summary, the same values the store starts from.
fn summary_fresh() -> GuiQueueSummary {
    GuiQueueSummary {
        remaining: 0,
        next_title: None,
        tail_secs: None,
        right_column: crate::bridge::frame::queue::GuiRightColumn::Length,
        filter: String::new(),
        filter_active: false,
    }
}

/// A fresh queue, the same values the store starts from.
fn fresh_queue() -> GuiQueue {
    GuiQueue {
        rows: Vec::new(),
        cursor: None,
        marks: Vec::new(),
        active: None,
        hover: None,
        drag: None,
    }
}

/// A fresh footer, the same values the store starts from.
fn fresh_footer() -> GuiQueueFooter {
    let pill = crate::bridge::frame::chrome::GuiFooterPill {
        text: String::new(),
        role: crate::bridge::frame::theme::GuiRole::TextPrimary,
        bold: false,
    };
    GuiQueueFooter {
        owner: pill.clone(),
        owner_live: false,
        playlist: pill,
        autosave: None,
        dirty: None,
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
    let owner_live = sections.footer.read().owner_live;
    rsx! {
        div { "cursor={cursor:?} remaining={remaining} owner_live={owner_live}" }
    }
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
    batch.footer = footer_batch(1, true).footer;

    let text = rendered(vec![batch]);

    assert!(text.contains("cursor=Some(2)"), "{text}");
    assert!(text.contains("remaining=3"), "{text}");
    assert!(text.contains("owner_live=true"), "{text}");
}

/// Contract 3 (task 3.1): a batch carries only the sections that changed —
/// a section it does not name keeps its value.
#[test]
fn a_section_the_batch_does_not_name_stays() {
    let text = rendered(vec![summary_batch(7, 3), queue_batch(1, Some(2))]);
    assert!(text.contains("cursor=Some(2)"), "{text}");
    assert!(text.contains("remaining=3"), "{text}");
}
