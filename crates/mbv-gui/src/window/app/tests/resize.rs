//! The window's first frame at a new size (D9): the live resize jerk. The
//! window reported a squashed or stretched view that then snapped into place,
//! because the size the panels and the arrangement are picked from reached the
//! tree after the frame that resolved at the new size: Blitz applies a resize to
//! the document's viewport inside its own window-event handler, without touching
//! the tree, so the first frame at the new size laid out the old arrangement and
//! the corrected one appeared a frame or two later. The launch copy now waits
//! for the drain to write the size before it presents a frame, and the drain
//! writes the size it read from the viewport ([`apply_viewport_size`]).
//!
//! This test runs the window's own sequence headless over a root that derives
//! its arrangement, panel visibility, and column widths from one size signal,
//! exactly as the root does: the shell's resize, the frame the launch copy holds,
//! the poll a held frame queues, and the redraw that poll asks for. No test opens
//! a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::channel;

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::super::ROOT_STYLE;
use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::layout::LayoutSize;
use crate::core_thread::CoreInput;
use crate::window::arrangement::{GuiArrangement, library_arrangement};
use crate::window::channel::CoreLink;
use crate::window::layout::{
    LayoutEpoch, ViewportSize, apply_viewport_size, frame_carries_change, note_presented_frame,
    track_container_size, use_layout_reporter, viewport_layout_size, window_layout_size,
};
use crate::window::resize::{DraggedWidths, panel_widths};

/// The window's height in the fixture: the Wide minimum height is 600, so every
/// size below keeps the height out of the arrangement's decision.
const HEIGHT: u32 = 900;

/// The toggles the fixture's panel section holds: the expanded panel, its queue
/// painted.
fn expanded() -> GuiPanelToggles {
    GuiPanelToggles {
        collapsed: false,
        queue_folded: false,
        ui_reset_epoch: 0,
    }
}

/// What the root takes: the viewport handle the launch copy writes a resize
/// through, and the cell the root hands its layout epoch back through.
#[derive(Clone)]
struct Props {
    viewport: ViewportSize,
    epoch: Rc<RefCell<Option<LayoutEpoch>>>,
}

/// The root, shaped like the window's: one layout size the drain and the
/// measurement both write, and the arrangement, panel visibility, and column
/// widths derived from it the way `App` derives them. Mini shows the Queue
/// panel alone (`gui-pointer-target-interaction` design D5).
fn resize_app(props: Props) -> Element {
    let Props { viewport, epoch } = props;
    let (core, _inputs) = channel::<CoreInput>();
    provide_context(CoreLink::new(core));
    provide_context(use_layout_reporter());
    let layout_epoch = use_hook(LayoutEpoch::new);
    provide_context(layout_epoch);
    epoch.borrow_mut().replace(layout_epoch);
    let container: Signal<LayoutSize> = use_signal(|| window_layout_size((0, 0), 1.0));
    let mut container_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    track_container_size(container_mounted, container);
    viewport.provide(container);

    let size = container.cloned();
    let widths = panel_widths(size, &expanded(), &DraggedWidths::default());
    let arrangement = library_arrangement(size, widths.queue);
    let queue_visible =
        arrangement != GuiArrangement::Mini || super::super::mini_shows_queue(arrangement);
    let library_visible =
        arrangement != GuiArrangement::Mini || !super::super::mini_shows_queue(arrangement);

    let stylesheet = ROOT_STYLE.as_str();
    rsx! {
        style { "{stylesheet}" }
        main { class: "h-dvh w-full overflow-hidden",
            div {
                class: "relative flex h-full min-h-0 w-full min-w-0 overflow-hidden",
                "data-arrangement": arrangement.name(),
                onmounted: move |event| container_mounted.set(Some(event)),
                // The Library column sits left of the Queue column, as the root paints them.
                if library_visible {
                    div {
                        class: "flex h-full min-h-0 flex-col flex-1",
                        "data-library-column": "",
                    }
                }
                if queue_visible {
                    div {
                        class: super::super::queue_column_class(true, false),
                        "data-queue-column": "",
                        style: "width: {widths.queue}px;",
                    }
                }
            }
        }
    }
}

/// The window under test: the document, the drain's sender, the viewport handle
/// the launch copy writes a resize through, and the arrangements of the frames
/// presented so far.
struct Window {
    doc: DioxusDocument,
    viewport: ViewportSize,
    epoch: LayoutEpoch,
    arrangements: Rc<RefCell<Vec<String>>>,
    /// Whether a viewport change landed since the last presented frame.
    viewport_changed: bool,
    /// How many frames were presented.
    presented: usize,
}

impl Window {
    /// A window opened at `width` x `height`, the way the launch copy opens one:
    /// the shell's first `SurfaceResized` applies the size and the copy writes it
    /// into the tree, and the frame that resize asked for presents it.
    fn built(width: u32, height: u32) -> Self {
        let viewport = ViewportSize::new();
        let epoch = Rc::new(RefCell::new(None));
        let mut doc = DioxusDocument::new(
            VirtualDom::new_with_props(
                resize_app,
                Props {
                    viewport: viewport.clone(),
                    epoch: Rc::clone(&epoch),
                },
            ),
            DocumentConfig {
                viewport: Some(Viewport::new(width, height, 1.0, ColorScheme::Light)),
                ..DocumentConfig::default()
            },
        );
        doc.initial_build();
        let mut window = Self {
            doc,
            viewport,
            epoch: epoch.borrow().expect("the root provided the layout epoch"),
            arrangements: Rc::new(RefCell::new(Vec::new())),
            viewport_changed: false,
            presented: 0,
        };
        window.resize(width, height);
        window.redraw();
        window
    }

    /// One `SurfaceResized`, as the launch copy handles it (D9): the shell
    /// applies the physical size to the document's viewport, then the copy
    /// writes that size into the tree and renders it, before any frame resolves.
    fn resize(&mut self, width: u32, height: u32) {
        self.doc.inner_mut().viewport_mut().window_size = (width, height);
        self.viewport_changed = true;
        let size = viewport_layout_size(self.doc.inner().viewport());
        apply_viewport_size(&mut self.doc, self.viewport.clone(), size);
    }

    /// One event-loop iteration of a continuous drag, in winit-wayland's order
    /// (`event_loop/mod.rs`: the proxy wake-up first, then the compositor's
    /// resize, then the redraw that resize asked for).
    fn drag_iteration(&mut self, width: u32) {
        self.poll();
        self.resize(width, HEIGHT);
        self.redraw();
    }

    /// Drag the window through `widths`, one event-loop iteration per size, and
    /// return the arrangement each iteration's *frame* painted: the loop is the
    /// drag itself, and the frame's arrangement is the tree as its resolve saw
    /// it, before any later poll renders the next size.
    fn drag_through(&mut self, widths: &[u32]) -> Vec<String> {
        let first = self.presented;
        for width in widths {
            self.drag_iteration(*width);
        }
        self.arrangements_since(first)
    }

    /// One `RedrawRequested`, as the launch copy answers it (D9): every redraw
    /// presents, because the size a resize carries reached the tree before it.
    fn redraw(&mut self) {
        self.present_frame();
    }

    /// The proxy wake-up: the shell drains the window channel and applies the
    /// render's mutations.
    fn poll(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("the document did not become idle");
    }

    /// One presented frame, as the launch copy runs it: the change fact is read
    /// before the resolve, the shell resolves and presents, and the frame reports
    /// the fact the drain bumps on.
    fn present_frame(&mut self) {
        let changed = {
            let document = self.doc.inner.borrow();
            frame_carries_change(&document, self.viewport_changed, false)
        };
        self.viewport_changed = false;
        self.doc.inner.borrow_mut().resolve(0.0);
        self.arrangements.borrow_mut().push(self.arrangement());
        let epoch = self.epoch;
        self.doc
            .vdom
            .in_runtime(move || note_presented_frame(epoch, changed));
        self.presented += 1;
        self.poll();
    }

    /// The arrangement the document paints now.
    fn arrangement(&self) -> String {
        let document = self.doc.inner();
        let node = document
            .query_selector("[data-arrangement]")
            .expect("the selector is valid")
            .expect("the container paints");
        document
            .get_node(node)
            .and_then(|node| node.attr(LocalName::from("data-arrangement")))
            .unwrap_or_default()
            .to_owned()
    }

    /// Whether the Queue column paints.
    fn paints_the_queue_column(&self) -> bool {
        matches!(
            self.doc.inner().query_selector("[data-queue-column]"),
            Ok(Some(_))
        )
    }

    /// Whether the Library column paints.
    fn paints_the_library_column(&self) -> bool {
        matches!(
            self.doc.inner().query_selector("[data-library-column]"),
            Ok(Some(_))
        )
    }

    /// The arrangement of every frame presented so far, in order.
    fn presented_arrangements(&self) -> Vec<String> {
        self.arrangements.borrow().clone()
    }

    /// The arrangements painted by the frames presented from `first` on.
    fn arrangements_since(&self, first: usize) -> Vec<String> {
        self.arrangements.borrow()[first..].to_vec()
    }
}

/// Contract (the live resize jerk of `e978b31aa`, D9): the first frame at the new
/// size already lays out the arrangement that size picks. A resize from the Wide
/// arrangement to Mini crosses the Library column's threshold, and Mini shows the
/// Queue panel alone (`gui-pointer-target-interaction` design D5): the frame the
/// resize asked for presents that arrangement — no frame resolves at the new size
/// with the old one.
#[test]
fn the_first_frame_at_a_new_size_paints_its_arrangement() {
    let mut window = Window::built(1600, 900);
    assert_eq!(window.arrangement(), "wide");
    assert!(window.paints_the_queue_column());
    assert!(window.paints_the_library_column());

    window.resize(800, 900);
    window.redraw();

    assert_eq!(window.arrangement(), "mini");
    assert!(window.paints_the_queue_column());
    assert!(!window.paints_the_library_column());
    assert_eq!(window.presented_arrangements(), vec!["wide", "mini"]);
}

/// The sizes a continuous drag steps through, one event-loop iteration each:
/// Wide down to Mini, so the arrangement changes while the pointer moves.
const DRAG_WIDTHS: [u32; 8] = [1500, 1400, 1300, 1200, 1100, 1000, 900, 800];

/// The arrangement the window picks at `width` x `HEIGHT`, read through the same
/// thresholds the window reads.
fn arrangement_at(width: u32) -> String {
    let size = window_layout_size((width, HEIGHT), 1.0);
    let widths = panel_widths(size, &expanded(), &DraggedWidths::default());
    library_arrangement(size, widths.queue).name().to_owned()
}

/// Contract (review of `fecfd22a1` and `90fc94733`, D9): a continuous drag
/// presents a frame at every event-loop iteration, and each frame lays out that
/// iteration's size. winit dispatches the proxy wake-up before the compositor's
/// resize and that resize's redraw, so a size that reaches the tree by a channel
/// hop — or a frame held for one — leaves the drag painting a stale arrangement
/// or nothing at all.
#[test]
fn a_continuous_drag_presents_each_iterations_arrangement() {
    let mut window = Window::built(1600, HEIGHT);

    let painted = window.drag_through(&DRAG_WIDTHS);

    assert_eq!(window.presented, 1 + DRAG_WIDTHS.len());
    assert_eq!(
        painted,
        DRAG_WIDTHS
            .iter()
            .map(|width| arrangement_at(*width))
            .collect::<Vec<_>>()
    );
}
