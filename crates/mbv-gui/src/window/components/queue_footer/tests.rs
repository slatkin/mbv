//! Headless tests of the Queue footer (D16): the contracts of
//! `QueueFooter.test.ts` — the queue source and the session marker with the
//! dot after the label, the liveness dot dropped while the shown Owner's
//! link is down, the canvas band's paint, the icon-free source and marker,
//! the autosave and dirty icons, the armed prefix badge, the live toast
//! painting inline and dropping the badge (the line's pill-drop rule), and
//! the regionless report. The core channel is a mock pair; no test opens a
//! window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::{Document as _, UserAgentStylesheet, local_name, traversal::TreeTraverser};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::QueueFooter;
use crate::bridge::frame::chrome::{
    GuiFooterPill, GuiQueueFooter, GuiStatusBar, GuiToast, GuiToastSeverity,
};
use crate::bridge::frame::theme::GuiRole;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::layout::LayoutEpoch;
use crate::window::sections::Sections;

/// The footer frame the fixture paints: the `woims` owner, live, fed from
/// the `mix` playlist, no autosave or dirty indicator.
fn footer_frame() -> GuiQueueFooter {
    GuiQueueFooter {
        owner: GuiFooterPill {
            text: "woims".to_owned(),
            role: GuiRole::TextFocusAccent,
            bold: false,
        },
        owner_live: true,
        playlist: GuiFooterPill {
            text: "mix".to_owned(),
            role: GuiRole::TextMetadata,
            bold: false,
        },
        autosave: None,
        dirty: None,
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

    /// The `style` attribute of the parent of the first node the selector
    /// names (the owner group's own fill reads from its label's parent).
    fn style_of_parent_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .expect("the labelled node paints");
        let parent = dom
            .get_node(id)
            .expect("the node is in the document")
            .parent
            .expect("the label has a parent");
        dom.get_node(parent)
            .expect("the parent is in the document")
            .attr(local_name!("style"))
            .map(str::to_owned)
            .unwrap_or_default()
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

    /// Paint the frame's footer and Status bar sections, resolve the
    /// layout, and let the re-measures land.
    fn show(&mut self, footer: GuiQueueFooter, status: GuiStatusBar) {
        let mut sections = self.sections.clone();
        self.doc.vdom.in_runtime(move || {
            sections.footer.set(footer);
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

/// Contract (`QueueFooter.test.ts`): the footer renders the queue source and
/// the session marker — the playlist text, the owner label, and the
/// liveness dot after the label, as the canvas paints the marker.
#[test]
fn the_footer_renders_the_queue_source_and_the_session_marker() {
    let mut fixture = built();
    fixture.show(footer_frame(), idle_status());

    let text = fixture.text();
    assert!(text.contains("mix"), "{text}");
    assert!(text.contains("woims"), "{text}");
    assert!(fixture.presents("[data-owner-live]"));
    assert_eq!(fixture.text_of_dot(), "●");
    // The dot follows the label: the label's position in the painted text
    // precedes the dot's.
    let label_at = text.find("woims").expect("the label paints");
    let dot_at = text.find('●').expect("the dot paints");
    assert!(label_at < dot_at, "{text}");
}

/// The liveness dot's text (the helper the marker contracts read).
impl Fixture {
    fn text_of_dot(&self) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector("[data-owner-live]")
            .expect("the selector is valid")
            .expect("the dot paints");
        dom.get_node(id)
            .expect("the node is in the document")
            .text_content()
    }
}

/// Contract (`QueueFooter.test.ts`): an owner whose link is down paints no
/// liveness dot — the dot marks the live Owner (design D4a).
#[test]
fn an_owner_whose_link_is_down_paints_no_liveness_dot() {
    let mut fixture = built();
    let mut down = footer_frame();
    down.owner_live = false;
    fixture.show(down, idle_status());

    assert!(!fixture.presents("[data-owner-live]"));
}

/// Contract (`QueueFooter.test.ts`, the canvas footer band U4.3): the line
/// paints the 40 px band, 16 px side padding, the canvas's mono type, the
/// 1 px top border, and the canvas's two line colours — the source text as
/// a secondary title and the owner marker in its sand.
#[test]
fn the_line_paints_the_canvas_footer_band() {
    let mut fixture = built();
    fixture.show(footer_frame(), idle_status());

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
    let owner_group = fixture.style_of_parent_of("[data-owner-label]");
    assert!(
        owner_group.contains("color: var(--canvas-owner)"),
        "{owner_group}"
    );
}

/// Contract (`QueueFooter.test.ts`): the source text and the owner marker
/// paint as plain text with no leading icon — the canvas paints neither.
#[test]
fn the_source_text_and_the_owner_marker_paint_with_no_leading_icon() {
    let mut fixture = built();
    fixture.show(footer_frame(), idle_status());

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
    fixture.show(frame, idle_status());

    assert_eq!(fixture.all("[data-footer-line] svg").len(), 2);
}

/// Contract (`QueueFooter.test.ts`): an armed frame renders the prefix
/// badge.
#[test]
fn an_armed_frame_renders_the_prefix_badge() {
    let mut fixture = built();
    fixture.show(
        footer_frame(),
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
    fixture.show(footer_frame(), idle_status());

    let report = fixture.last_report().expect("the mounted surface reports");
    assert!(report.lists.is_empty(), "{report:?}");
    assert!(report.regions.is_empty(), "{report:?}");
}
