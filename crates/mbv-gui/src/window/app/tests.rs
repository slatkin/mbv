//! Headless tests of the window root (D16): the effects it mounts with, and
//! the stylesheet it paints. The effect contract is the one the review of
//! task 4.1 pinned — the `uiResetEpoch` effect settles. Every signal write
//! marks its subscribers (an equal value included), so an effect that reads
//! a signal in its body and then writes it re-queues itself forever, and the
//! scheduler's task/effect drain never empties: the live window's render
//! loop would spin at 100 % CPU after the first frame. No scheduler entry
//! can bound that drain, so the host's effect body carries a run-count
//! tripwire: a self-queued run past the bound panics out of the scheduler,
//! and the test fails fast without hanging. The idle assertion itself drives
//! only `now_or_never(wait_for_work)` passes, which never enter the render
//! loop. The host runs the exact effect body the root runs
//! ([`note_panel_ui_reset`]). The stylesheet contract is the one the live run
//! of PR 1003 broke: the committed sheet must reach the document. No test
//! opens a window, sleeps, or reaches the core.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use futures_util::FutureExt;
use mbv_config::GuiPanelWidths;

use super::note_panel_ui_reset;
use crate::bridge::frame::GuiPanelToggles;
use crate::window::input::f32_from_f64;
use crate::window::resize::PanelWidthsStore;
use crate::window::sections::Sections;

mod art;
mod header;
mod paint;
mod resize;

/// The pass bound for the idle assertion: a settling effect goes `Pending`
/// within a few passes; a self-triggering one never does.
const PASS_BOUND: usize = 16;

/// The run bound the effect's tripwire enforces: a settling effect runs once
/// per fold; the ninth run in one phase means the effect re-queued itself.
const RUN_BOUND: usize = 8;

/// What the root hands the test: the section stores.
type FixtureHandles = Sections;

#[derive(Clone)]
struct FixtureProps {
    runs: Rc<Cell<usize>>,
    handle: Rc<std::cell::RefCell<Option<FixtureHandles>>>,
}

/// The effect's host: the same `uiResetEpoch` fold the root's effect runs,
/// over a width store whose saver records nothing.
fn reset_host(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let widths = use_hook(|| PanelWidthsStore::new(GuiPanelWidths::default(), Arc::new(|_| {})));
    let mut store = widths;
    let host_sections = sections.clone();
    let runs = props.runs;
    use_effect(move || {
        let count = runs.get() + 1;
        runs.set(count);
        assert!(
            count <= RUN_BOUND,
            "the uiResetEpoch effect re-queued itself: run {count} of at most {RUN_BOUND}"
        );
        let _reset = note_panel_ui_reset(&mut store, &host_sections);
    });
    props.handle.borrow_mut().replace(sections);
    rsx! {
        div { "the reset effect's host" }
    }
}

struct Fixture {
    vdom: VirtualDom,
    sections: Sections,
    runs: Rc<Cell<usize>>,
}

impl Fixture {
    /// Whether the effect settled in this phase: the scheduler reached a
    /// pass with no dirty scope and no ready task, within the pass bound. A
    /// `Ready` pass runs at most one queued task or effect — never the
    /// render loop — so a self-triggering effect is caught by the tripwire
    /// in the host's effect body, not by a spin here.
    fn settles(&mut self) -> bool {
        self.runs.set(0);
        for _ in 0..PASS_BOUND {
            let work = self.vdom.wait_for_work();
            if std::pin::pin!(work).now_or_never().is_none() {
                return true;
            }
        }
        false
    }

    /// Write the `panel` section's toggles, as the drain task would.
    fn set_panel(&mut self, toggles: GuiPanelToggles) {
        let mut panel = self.sections.panel;
        self.vdom.in_runtime(move || panel.set(toggles));
    }
}

fn built() -> Fixture {
    let runs = Rc::new(Cell::new(0));
    let handle = Rc::new(std::cell::RefCell::new(None));
    let mut vdom = VirtualDom::new_with_props(
        reset_host,
        FixtureProps {
            runs: Rc::clone(&runs),
            handle: Rc::clone(&handle),
        },
    );
    vdom.rebuild_in_place();
    let sections = handle
        .borrow_mut()
        .take()
        .expect("the root handed the stores over");
    Fixture {
        vdom,
        sections,
        runs,
    }
}

/// Contract (review of task 4.1): the `uiResetEpoch` effect settles. The
/// first fold only seeds the store, and a new epoch — the core's Reset UI
/// State — folds again; neither the seed's write nor a reset's write may
/// re-queue the effect that read it, at mount or after a section change.
#[test]
fn the_ui_reset_effect_settles() {
    let mut fixture = built();
    assert!(fixture.settles(), "the effect did not settle at mount");
    fixture.set_panel(GuiPanelToggles {
        collapsed: false,
        queue_folded: false,
        ui_reset_epoch: 1,
    });
    assert!(fixture.settles(), "the effect did not settle after a reset");
}

/// The root's own markup, in the shape the window paints it: the `style`
/// element the root renders ([`super::root_style_element`], the cell-unit
/// rule then the generated stylesheet), then the root `main`, the Queue
/// row's button with its own class string, a text line, and the divider's
/// `hover:` fill.
///
/// Ids are the test's own: the window's class strings carry none.
fn stylesheet_probe() -> Element {
    rsx! {
        {super::root_style_element()}
        main { id: "probe-root", class: "h-dvh w-full flex bg-background text-foreground",
            div { id: "probe-row",
                class: "flex w-full items-center overflow-hidden text-left whitespace-nowrap",
                span { id: "probe-text", class: "tabular-nums", "Up next · 210" }
                button { id: "probe-button",
                    "type": "button",
                    class: "flex w-full items-center overflow-hidden text-left whitespace-nowrap",
                    "Ctrl K"
                }
                button { id: "probe-seek",
                    "type": "button",
                    // The real transport button pins this box; a text-sized
                    // probe collapses without fonts and the hover point
                    // misses it.
                    style: "width: 44px; height: 44px;",
                    "data-target": "seekBack",
                    "−10"
                }
            }
            button { id: "probe-filled",
                "type": "button",
                // The shorthand the real rows set inline: it resets
                // `background-image` to `none` unless the veil's rule wins.
                style: "width: 80px; height: 30px; background: #3d484d;",
                "Row"
            }
            label { id: "probe-frame",
                "data-hover-frame": "",
                style: "width: 80px; height: 30px;",
                input { "type": "text" }
            }
            div { id: "probe-hover",
                class: "hover:bg-[var(--canvas-border)]",
                style: "width: 120px; height: 40px;",
            }
        }
    }
}

struct Stylesheet {
    doc: DioxusDocument,
}

impl Stylesheet {
    /// Build the probe over the real stylesheet, then resolve its style and
    /// layout one time, as the renderer's first frame does. Nothing writes a
    /// signal, so no effect runs and the document cannot re-queue itself.
    fn built() -> Self {
        let mut doc = DioxusDocument::new(
            VirtualDom::new(stylesheet_probe),
            DocumentConfig {
                viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
                ua_stylesheets: vec![blitz_dom::UserAgentStylesheet::default_sheet()],
                ..DocumentConfig::default()
            },
        );
        doc.initial_build();
        doc.inner.borrow_mut().resolve(0.0);
        Self { doc }
    }

    /// The resolved value of `property` on the element `selector` names, as
    /// `getComputedStyle()` reports it.
    fn resolved(&self, selector: &str, property: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.resolved_style_value(id, property)
    }

    /// Put the pointer over the middle of the element `selector` names and
    /// restyle, as the window does on a pointer move; returns the element's
    /// resolved value of `property` under the pointer.
    fn hovered(&mut self, selector: &str, property: &str) -> String {
        let mut dom = self.doc.inner.borrow_mut();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        let rect = dom
            .get_client_bounding_rect(id)
            .expect("the probe is laid out");
        let x = f32_from_f64(rect.x + rect.width / 2.0);
        let y = f32_from_f64(rect.y + rect.height / 2.0);
        assert!(dom.set_hover_to(x, y), "{selector} takes the hover");
        dom.resolve(0.0);
        dom.resolved_style_value(id, property)
    }
}

/// Contract (live run of PR 1003): the window's real stylesheet applies. The
/// root paints one `style` element with the generated Tailwind sheet in it,
/// and the window renders unstyled when that sheet does not reach Blitz's
/// style engine — no preflight reset on a button, no `display: flex` on a
/// flex row, and the fallback serif face instead of the bundled sans. This
/// loads the committed stylesheet through the root's own style element, so a
/// stylesheet that the engine never receives, or one it parses empty, fails
/// here and not in a live run.
#[test]
fn the_window_stylesheet_applies_preflight_utilities_and_the_font() {
    let sheet = Stylesheet::built();
    // The utilities layer: `flex` on the root, and the Queue row's own
    // `whitespace-nowrap` on its button.
    assert_eq!(sheet.resolved("#probe-root", "display"), "flex");
    assert_eq!(sheet.resolved("#probe-button", "white-space"), "nowrap");
    // The theme layer, through a utility: a real colour, not the unset one.
    assert_ne!(
        sheet.resolved("#probe-root", "background-color"),
        "rgba(0, 0, 0, 0)"
    );
    // Preflight: the reset removes the user-agent button fill and border.
    assert_eq!(
        sheet.resolved("#probe-button", "background-color"),
        "rgba(0, 0, 0, 0)"
    );
    assert_eq!(sheet.resolved("#probe-button", "border-top-width"), "0px");
    // Preflight's root font, through the theme's `--font-sans`, inherited by
    // the text the window paints.
    let font = sheet.resolved("#probe-text", "font-family");
    assert!(
        font.contains("IBM Plex Sans"),
        "the root font is the bundled sans face: {font}"
    );
}

/// Contract (task 11.1, look parity with the former window): a `hover:`
/// utility paints under the pointer through the window's own stylesheet. The
/// document carries the committed sheet in the root's style element, as the
/// window does; the test resolves layout, hovers the divider's box, and reads
/// the fill, so the class paints the canvas border only while the pointer is
/// on it. It fails when the rule never reaches the document — the sheet drops
/// the class, or a gate around it stops matching — which is the look a port
/// of the former hover state must not lose. The divider is the one hover
/// utility the test can drive without a library frame (the Table's glow
/// button needs one).
#[test]
fn a_hover_utility_paints_under_the_pointer() {
    let mut sheet = Stylesheet::built();
    assert_eq!(
        sheet.resolved("#probe-hover", "background-color"),
        "rgba(0, 0, 0, 0)",
        "the hover fill waits for the pointer"
    );
    assert_eq!(
        sheet.hovered("#probe-hover", "background-color"),
        "rgb(61, 72, 77)",
        "the hovered element paints the canvas border"
    );
}

/// Contract (2026-11-19, the hover veil painted nothing): a button answers
/// the pointer through the shared rule, even when it paints its own fill with
/// the inline `background:` shorthand, as every list row does. The veil is the
/// one-colour gradient over that fill. The earlier rule resolved to no style
/// at all on a hovered button — one selector in its list, `label:has(>
/// input:hover)`, is not valid in Blitz's Stylo and dropped the whole list —
/// so this test reads the resolved `background-image` under the pointer.
#[test]
fn a_filled_button_paints_the_hover_veil_under_the_pointer() {
    let mut sheet = Stylesheet::built();
    assert_eq!(
        sheet.resolved("#probe-filled", "background-image"),
        "none",
        "the veil waits for the pointer"
    );
    let hovered = sheet.hovered("#probe-filled", "background-image");
    assert!(
        hovered.starts_with("linear-gradient("),
        "the hovered button paints the veil: {hovered}"
    );
    assert_eq!(
        sheet.resolved("#probe-filled", "background-color"),
        "rgb(61, 72, 77)",
        "the veil sits over the button's own fill"
    );
}

/// Contract (2026-11-19): the Library filter field's frame (a `label` around
/// its `input`, marked `data-hover-frame`) takes the same veil under the
/// pointer, since `label:has(> input:hover)` cannot select it.
#[test]
fn the_filter_frame_paints_the_hover_veil_under_the_pointer() {
    let mut sheet = Stylesheet::built();
    let hovered = sheet.hovered("#probe-frame", "background-image");
    assert!(
        hovered.starts_with("linear-gradient("),
        "the hovered frame paints the veil: {hovered}"
    );
}

/// Contract (2026-11-16): the transport's seek labels answer the pointer in
/// the accent. The seek buttons rest in the role colour and turn
/// `--canvas-accent` under the pointer, so a press target reads as live; the
/// rule is the stylesheet's, not an inline style, so the hover cannot lose
/// to the button's own paint. It fails when the rule drops from the sheet or
/// stops matching — the inert label look the buttons must not go back to.
#[test]
fn the_transport_seek_labels_answer_the_pointer_in_the_accent() {
    let mut sheet = Stylesheet::built();
    let accent = "rgb(167, 192, 128)";
    assert_ne!(
        sheet.resolved("#probe-seek", "color"),
        accent,
        "the resting label keeps the role colour"
    );
    assert_eq!(
        sheet.hovered("#probe-seek", "color"),
        accent,
        "the hovered label paints the canvas accent"
    );
}
