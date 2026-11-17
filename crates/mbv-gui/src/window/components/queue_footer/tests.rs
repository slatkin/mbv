//! Headless tests of the Queue footer (D16): the contracts of
//! `QueueFooter.test.ts` — the queue source and its dirty mark beside it, the
//! totals at the line's right, the canvas band's paint, the icon-free source,
//! the autosave and dirty icons, the armed prefix badge, the live toast
//! painting inline and dropping the badge (the line's pill-drop rule), and the
//! regionless report. The core channel is a mock pair; no test opens a window,
//! sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::{Document as _, UserAgentStylesheet, local_name, traversal::TreeTraverser};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::QueueFooter;
use crate::bridge::frame::chrome::{
    GuiFooterPill, GuiQueueFooter, GuiStatusBar, GuiToast, GuiToastSeverity,
};
use crate::bridge::frame::queue::{GuiQueueHeaderFacts, GuiQueueSummary};
use crate::bridge::frame::theme::GuiRole;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::layout::LayoutEpoch;
use crate::window::sections::Sections;

/// The footer frame the fixture paints: the `mix` playlist source, no autosave
/// or dirty indicator.
fn footer_frame() -> GuiQueueFooter {
    GuiQueueFooter {
        playlist: GuiFooterPill {
            text: "mix".to_owned(),
            role: GuiRole::TextMetadata,
            bold: false,
        },
        autosave: None,
        dirty: None,
    }
}

/// The summary the fixture paints: a nine-hour shown queue, no rows marked, no
/// unknown runtime.
fn summary() -> GuiQueueSummary {
    GuiQueueSummary {
        remaining: 0,
        next_title: None,
        tail_secs: None,
        filter: String::new(),
        filter_active: false,
        total_secs: 9 * 60 * 60,
        total_partial: false,
        marked_secs: None,
        marked_partial: false,
        header: GuiQueueHeaderFacts {
            entries: 1,
            shown_name: "Default".to_owned(),
            close_disabled: true,
        },
    }
}

/// The idle Status bar: nothing armed, no toast.
fn idle_status() -> GuiStatusBar {
    GuiStatusBar {
        prefix_armed: false,
        toast: None,
    }
}

/// The autosave and dirty pills the icons contract paints.
fn active_pills() -> (Option<GuiFooterPill>, Option<GuiFooterPill>) {
    (
        Some(GuiFooterPill {
            text: "AUTOSAVE".to_owned(),
            role: GuiRole::Accent,
            bold: false,
        }),
        Some(GuiFooterPill {
            text: "UNSAVED".to_owned(),
            role: GuiRole::TextFocusAccent,
            bold: true,
        }),
    )
}

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads (the same rules the Queue panel's and the
/// player panel's harnesses use), so the toast's `Truncate` measure reads a
/// laid-out width.
const FIXTURE_CSS: &str = concat!(
    ".flex { display: flex; }",
    ".items-center { align-items: center; }",
    ".min-w-0 { min-width: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".shrink-0 { flex-shrink: 0; }",
    ".block { display: block; }",
    ".truncate { width: 100%; }",
);

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The context handles the root hands the test: the section stores and the
/// layout epoch.
type FixtureHandles = (Sections, LayoutEpoch);

/// The root: paints the footer line over the section stores, as `Panel`
/// does.
fn footer_app(props: FixtureProps) -> Element {
    let FixtureProps { sender, handle } = props;
    provide_context(CoreLink::new(sender));
    provide_context(crate::window::layout::use_layout_reporter());
    provide_context(use_hook(LayoutEpoch::new));
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let epoch = use_hook(LayoutEpoch::new);
    handle.borrow_mut().replace((sections, epoch));
    rsx! {
        QueueFooter { library_visible: false }
    }
}

struct Fixture {
    doc: DioxusDocument,
    receiver: Receiver<CoreInput>,
    sections: Sections,
    epoch: LayoutEpoch,
}

impl Fixture {
    /// The whole text the document paints, one node per line.
    fn text(&self) -> String {
        let dom = self.doc.inner();
        let mut out = String::new();
        for id in TreeTraverser::new(&dom) {
            if let Some(node) = dom.get_node(id) {
                out.push_str(&node.text_content());
                out.push('\n');
            }
        }
        out
    }

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        let dom = self.doc.inner();
        matches!(dom.query_selector(selector), Ok(Some(_)))
    }

    /// The node ids the selector names, in tree order.
    fn all(&self, selector: &str) -> Vec<blitz_dom::NodeId> {
        let dom = self.doc.inner();
        dom.query_selector_all(selector)
            .expect("the selector is valid")
            .into_iter()
            .collect()
    }

    /// The `style` attribute of the first node the selector names.
    fn style_of(&self, selector: &str) -> String {
        self.attr_of(selector, local_name!("style"))
            .unwrap_or_else(|| panic!("{selector} paints"))
    }

    /// The attribute of the first node the selector names.
    fn attr_of(&self, selector: &str, attribute: blitz_dom::LocalName) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")?;
        let node = dom.get_node(id)?;
        node.attr(attribute).map(str::to_owned)
    }

    /// The text of the first node the selector names.
    fn text_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .text_content()
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed (a fixed poll count with a tripwire: never an unbounded wait).
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// Paint the frame's footer, summary, and Status bar sections, resolve the
    /// layout, and let the re-measures land.
    fn show(&mut self, footer: GuiQueueFooter, summary: GuiQueueSummary, status: GuiStatusBar) {
        let mut sections = self.sections.clone();
        self.doc.vdom.in_runtime(move || {
            sections.footer.set(footer);
            sections.summary.set(summary);
            sections.status.set(status);
        });
        self.poll_until_idle();
        self.lay_out();
    }

    /// Resolve the document's style and layout, as the renderer's frame
    /// would, and let the `Truncate` re-measures the epoch bump drives land.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// The last merged layout report.
    fn last_report(&self) -> Option<crate::bridge::layout::LayoutReport> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Layout(report) => Some(report),
                _ => None,
            })
            .last()
    }
}

/// A fixture over the footer line alone, laid out into a 640×900 viewport
/// with the queue tree's utilities, so the toast's `Truncate` measure reads
/// a laid-out width and the toast paints whole.
fn built() -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let config = DocumentConfig {
        viewport: Some(Viewport::new(640, 900, 1.0, ColorScheme::Light)),
        ua_stylesheets: vec![
            UserAgentStylesheet::default_sheet(),
            UserAgentStylesheet::parse(FIXTURE_CSS),
        ],
        ..DocumentConfig::default()
    };
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            footer_app,
            FixtureProps {
                sender,
                handle: Rc::clone(&handle),
            },
        ),
        config,
    );
    doc.initial_build();
    let (sections, epoch) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the section stores over");
    let mut fixture = Fixture {
        doc,
        receiver,
        sections,
        epoch,
    };
    fixture.poll_until_idle();
    fixture.lay_out();
    fixture
}

/// Contract (`QueueFooter.test.ts`, `gui-named-queues` task 8.5): the footer
/// renders the queue source and its dirty mark beside it, and carries no
/// Player-owner label and no liveness dot (design D9).
#[test]
fn the_footer_renders_the_queue_source_beside_its_dirty_mark() {
    let mut fixture = built();
    let mut frame = footer_frame();
    let (autosave, dirty) = active_pills();
    frame.autosave = autosave;
    frame.dirty = dirty;
    fixture.show(frame, summary(), idle_status());

    let text = fixture.text();
    assert!(text.contains("mix"), "{text}");
    assert!(text.contains("UNSAVED"), "{text}");
    let source_at = text.find("mix").expect("the source paints");
    let dirty_at = text.find("UNSAVED").expect("the dirty mark paints");
    assert!(source_at < dirty_at, "the dirty mark follows the source");
    assert!(!fixture.presents("[data-owner-label]"));
    assert!(!fixture.presents("[data-owner-live]"));
}

/// Contract (`QueueFooter.test.ts`, the canvas footer band U4.3): the line
/// paints the 40 px band, 16 px side padding, the canvas's mono type, the
/// 1 px top border, and the canvas's line colour — the source text as a
/// secondary title.
#[test]
fn the_line_paints_the_canvas_footer_band() {
    let mut fixture = built();
    fixture.show(footer_frame(), summary(), idle_status());

    assert!(
        fixture.presents("[data-pointer-zone=\"queue\"]"),
        "the band carries the Queue zone marker (`gui-pointer-target-interaction` 3.3)"
    );
    let line = fixture.style_of("[data-footer-line]");
    assert!(line.contains("height: 40px"), "{line}");
    assert!(line.contains("padding-inline: 16px"), "{line}");
    assert!(line.contains("font-size: 12px"), "{line}");
    assert!(
        line.contains("border-top: 1px solid var(--canvas-border)"),
        "{line}"
    );
    let source = fixture.style_of("[data-footer-source]");
    assert!(
        source.contains("color: var(--canvas-type-secondary)"),
        "{source}"
    );
}

/// Contract (`QueueFooter.test.ts`, `gui-named-queues` task 8.5, spec
/// `gui-client` "The footer totals the marked rows", "No row is marked", "A
/// runtime is unknown"): the line's right group paints the marked rows' total
/// beside the shown queue's total, the marked total only while rows are marked
/// ('Sel' names it), and a sum over an unknown runtime carries a `+` as a lower
/// bound.
#[rstest]
#[case::marked(Some(3_600), false, false, Some("1:00:00"), "9:00:00")]
#[case::no_marks(None, false, false, None, "9:00:00")]
#[case::unknown_runtimes(Some(3_600), true, true, Some("1:00:00+"), "9:00:00+")]
fn the_footer_totals_the_marked_rows(
    #[case] marked_secs: Option<i64>,
    #[case] marked_partial: bool,
    #[case] total_partial: bool,
    #[case] expected_marked: Option<&str>,
    #[case] expected_total: &str,
) {
    let mut fixture = built();
    let summary = GuiQueueSummary {
        total_partial,
        marked_secs,
        marked_partial,
        ..summary()
    };
    fixture.show(footer_frame(), summary, idle_status());

    assert_eq!(fixture.text_of("[data-footer-total]"), expected_total);
    match expected_marked {
        Some(marked) => {
            assert_eq!(
                fixture.text_of("[data-footer-marked-label]"),
                "Sel",
                "the marked total is named"
            );
            assert_eq!(fixture.text_of("[data-footer-marked-value]"), marked);
        }
        None => assert!(!fixture.presents("[data-footer-marked]")),
    }
}

/// Contract (`QueueFooter.test.ts`): the source text paints as plain text with
/// no leading icon — the canvas paints none.
#[test]
fn the_source_text_paints_with_no_leading_icon() {
    let mut fixture = built();
    fixture.show(footer_frame(), summary(), idle_status());

    assert_eq!(fixture.all("[data-footer-line] svg").len(), 0);
}

/// Contract (`QueueFooter.test.ts`): the active footer states paint their
/// Lucide icons — the autosave glyph and the dirty glyph, one each.
#[test]
fn the_active_footer_states_paint_their_lucide_icons() {
    let mut fixture = built();
    let mut frame = footer_frame();
    let (autosave, dirty) = active_pills();
    frame.autosave = autosave;
    frame.dirty = dirty;
    fixture.show(frame, summary(), idle_status());

    assert_eq!(fixture.all("[data-footer-line] svg").len(), 2);
}

/// Contract (`QueueFooter.test.ts`): an armed frame renders the prefix
/// badge.
#[test]
fn an_armed_frame_renders_the_prefix_badge() {
    let mut fixture = built();
    fixture.show(
        footer_frame(),
        summary(),
        GuiStatusBar {
            prefix_armed: true,
            toast: None,
        },
    );

    assert!(fixture.text().contains("PREFIX"));
}

/// Contract (`QueueFooter.test.ts`): a live toast paints inline in the line
/// and drops the prefix badge — the line's pill-drop rule, the TUI's pill
/// drop carried into the footer.
#[test]
fn a_live_toast_paints_inline_in_the_line_and_drops_the_prefix_badge() {
    let mut fixture = built();
    fixture.show(
        footer_frame(),
        summary(),
        GuiStatusBar {
            prefix_armed: true,
            toast: Some(GuiToast {
                text: "Scanning 'Movies'...".to_owned(),
                severity: GuiToastSeverity::Neutral,
            }),
        },
    );

    let text = fixture.text();
    assert!(text.contains("Scanning 'Movies'..."), "{text}");
    assert!(!text.contains("PREFIX"), "the prefix badge drops: {text}");
}

/// Contract (`QueueFooter.test.ts`): the line reports no region — its pills
/// are informational, so a reported target whose press can never fire would
/// lie in the layout report; the report still accounts for the mounted
/// surface.
#[test]
fn the_line_reports_no_region() {
    let mut fixture = built();
    fixture.show(footer_frame(), summary(), idle_status());

    let report = fixture.last_report().expect("the mounted surface reports");
    assert!(report.lists.is_empty(), "{report:?}");
    assert!(report.regions.is_empty(), "{report:?}");
}
