//! Tests of the layout reporter's merge (D9): the port's ordering contract —
//! the merged report carries the contributions in key-insertion order, the
//! TypeScript module's `Map` order, so a key registered later paints over an
//! earlier key's regions in the core's hit map (the hit map's last push
//! wins) — and the re-set semantics: re-reporting an existing key updates it
//! in place instead of moving it last. Headless documents whose root built
//! the reporter over a recorded channel; no window, no sleep.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{LayoutContribution, LayoutKey, LayoutReporter, use_layout_reporter};
use crate::bridge::layout::{
    LayoutRect, LayoutRegion, LayoutReport, QueuePlaybackTargetId, QueueTargetId, RegionTarget,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;

/// One region's contribution under a distinct target, so the assertions can
/// tell the keys apart.
fn contribution(target: RegionTarget) -> LayoutContribution {
    LayoutContribution {
        lists: Vec::new(),
        regions: vec![LayoutRegion {
            rect: LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            target,
        }],
    }
}

/// What the root hands the test: the reporter and the core-input receiver.
#[derive(Default)]
struct Handoff {
    reporter: Option<LayoutReporter>,
    inputs: Option<mpsc::Receiver<CoreInput>>,
}

#[derive(Clone)]
struct ReporterProps {
    handoff: Rc<RefCell<Handoff>>,
}

/// The root: builds the reporter over a channel the test reads.
fn reporter_app(props: ReporterProps) -> Element {
    let ReporterProps { handoff } = props;
    let (tx, rx) = mpsc::channel();
    provide_context(CoreLink::new(tx));
    let reporter = use_layout_reporter();
    let mut handoff = handoff.borrow_mut();
    handoff.reporter.replace(reporter);
    handoff.inputs.replace(rx);
    rsx! {}
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

/// A document whose root built the reporter over a channel the test reads.
fn built_reporter() -> (DioxusDocument, LayoutReporter, mpsc::Receiver<CoreInput>) {
    let handoff = Rc::new(RefCell::new(Handoff::default()));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            reporter_app,
            ReporterProps {
                handoff: Rc::clone(&handoff),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    poll_until_idle(&mut doc);
    let mut taken = handoff.borrow_mut();
    let reporter = taken
        .reporter
        .take()
        .expect("the root handed the reporter over");
    let inputs = taken
        .inputs
        .take()
        .expect("the root handed the receiver over");
    (doc, reporter, inputs)
}

/// The last merged report the reporter sent, draining the channel.
fn last_layout(inputs: &mpsc::Receiver<CoreInput>) -> LayoutReport {
    let mut last = None;
    for input in inputs.try_iter() {
        if let CoreInput::Layout(report) = input {
            last = Some(report);
        }
    }
    last.expect("the reporter sent a layout report")
}

/// The merged report's targets, in merge order.
fn merged_targets(report: &LayoutReport) -> Vec<RegionTarget> {
    report.regions.iter().map(|region| region.target).collect()
}

/// Contract (D9): the merged report carries the contributions in
/// key-insertion order — the TypeScript `Map`'s order, not the process's
/// hash order — and a re-report of an existing key updates it in place
/// instead of moving it last. The chooser's key is registered second and
/// must keep merging last over the surface's regions, so a press on a
/// chooser row resolves to the row, never to the region beneath it.
#[test]
fn the_merged_report_keeps_key_insertion_order_across_a_reset() {
    let (mut doc, mut reporter, inputs) = built_reporter();

    let surface = LayoutKey::next();
    let chooser = LayoutKey::next();
    reporter.report_layout(
        surface,
        contribution(RegionTarget::QueuePlayback(
            QueuePlaybackTargetId::VisualSlot,
        )),
    );
    reporter.report_layout(
        chooser,
        contribution(RegionTarget::QueuePlayback(
            QueuePlaybackTargetId::TargetChoice(0),
        )),
    );
    poll_until_idle(&mut doc);

    let report = last_layout(&inputs);
    assert_eq!(
        merged_targets(&report),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(0)),
        ]
    );

    // A re-set updates in place (the `Map`'s `set`): the surface key keeps
    // its position, so the chooser's rows still merge last.
    reporter.report_layout(
        surface,
        contribution(RegionTarget::Queue(QueueTargetId::Slot(1))),
    );
    poll_until_idle(&mut doc);

    let report = last_layout(&inputs);
    assert_eq!(
        merged_targets(&report),
        vec![
            RegionTarget::Queue(QueueTargetId::Slot(1)),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(0)),
        ]
    );
}
