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
use futures_util::StreamExt;

use super::super::ROOT_STYLE;
use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::layout::LayoutSize;
use crate::core_thread::CoreInput;
use crate::window::arrangement::{GuiArrangement, library_arrangement};
use crate::window::channel::{CoreLink, EventSlot, ViewportPending, WindowEvent};
use crate::window::layout::{
    LayoutEpoch, apply_viewport_size, frame_carries_change, note_presented_frame,
    track_container_size, use_layout_reporter, viewport_layout_size,
};
use crate::window::resize::{DraggedWidths, panel_widths};

/// The toggles the fixture's panel section holds: the expanded panel, its queue
/// painted.
fn expanded() -> GuiPanelToggles {
    GuiPanelToggles {
        collapsed: false,
        queue_folded: false,
        ui_reset_epoch: 0,
    }
}

/// What the root takes: the drain's event slot, the viewport handoff, and the
/// cell the root hands its layout epoch back through.
#[derive(Clone)]
struct Props {
    slot: EventSlot,
    pending: ViewportPending,
    epoch: Rc<RefCell<Option<LayoutEpoch>>>,
}

/// The root, shaped like the window's: one layout size the drain and the
/// measurement both write, and the arrangement, panel visibility, and column
/// widths derived from it the way `App` derives them. The Library panel is the
/// active destination and holds focus, so Mini shows it alone.
fn resize_app(props: Props) -> Element {
    let Props {
        slot,
        pending,
        epoch,
    } = props;
    let (core, _inputs) = channel::<CoreInput>();
    provide_context(CoreLink::new(core));
    provide_context(use_layout_reporter());
    let layout_epoch = use_hook(LayoutEpoch::new);
    provide_context(layout_epoch);
    epoch.borrow_mut().replace(layout_epoch);
    let container: Signal<LayoutSize> = use_signal(LayoutSize::default);
    let mut container_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    track_container_size(container_mounted, container);

    // The drain's viewport arm (D9): the size reaches the tree before the frame
    // that resolves at it, and the handoff clears so the launch copy presents.
    use_future(move || {
        let slot = slot.clone();
        let pending = pending.clone();
        async move {
            let Some(mut events) = slot.take() else {
                return;
            };
            while let Some(event) = events.next().await {
                if let WindowEvent::Viewport { size } = event {
                    apply_viewport_size(container, size, &pending);
                }
            }
        }
    });

    let size = container.cloned();
    let widths = panel_widths(size, &expanded(), &DraggedWidths::default());
    let arrangement = library_arrangement(size, widths.queue);
    let queue_visible = arrangement != GuiArrangement::Mini;

    let stylesheet = ROOT_STYLE.as_str();
    rsx! {
        style { "{stylesheet}" }
        main { class: "h-dvh w-full overflow-hidden",
            div {
                class: "relative flex h-full min-h-0 w-full min-w-0 overflow-hidden",
                "data-arrangement": arrangement.name(),
                onmounted: move |event| container_mounted.set(Some(event)),
                if queue_visible {
                    div {
                        class: "flex h-full min-h-0 flex-col flex-none",
                        "data-queue-column": "",
                        style: "width: {widths.queue}px;",
                    }
                }
                div {
                    class: "flex h-full min-h-0 flex-col flex-1",
                    "data-library-column": "",
                }
            }
        }
    }
}

/// The window under test: the document, the drain's sender, the viewport
/// handoff, and the arrangements of the frames presented so far.
struct Window {
    doc: DioxusDocument,
    pending: ViewportPending,
    sender: UnboundedSender<WindowEvent>,
    epoch: LayoutEpoch,
    arrangements: Rc<RefCell<Vec<String>>>,
    /// Whether a resize's frame was already held, as the launch copy tracks it.
    held: bool,
    /// Whether a viewport change landed since the last presented frame.
    viewport_changed: bool,
    /// How many frames were presented.
    presented: usize,
}

impl Window {
    /// A window opened at `width` x `height`, the way the launch copy opens one:
    /// the shell's first `SurfaceResized` queues the size, its frame is held, and
    /// the frame the poll asks for presents the first arrangement.
    fn built(width: u32, height: u32) -> Self {
        let (sender, receiver) = futures_channel::mpsc::unbounded();
        let pending = ViewportPending::new();
        let epoch = Rc::new(RefCell::new(None));
        let mut doc = DioxusDocument::new(
            VirtualDom::new_with_props(
                resize_app,
                Props {
                    slot: EventSlot::new(receiver),
                    pending: pending.clone(),
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
            pending,
            sender,
            epoch: epoch.borrow().expect("the root provided the layout epoch"),
            arrangements: Rc::new(RefCell::new(Vec::new())),
            held: false,
            viewport_changed: false,
            presented: 0,
        };
        window.poll();
        window.resize(width, height);
        window.redraw();
        window.redraw();
        window
    }

    /// One `SurfaceResized`, as the launch copy handles it (D9): the shell
    /// applies the physical size to the document's viewport, then the copy
    /// queues the event the drain reads.
    fn resize(&mut self, width: u32, height: u32) {
        self.doc.inner_mut().viewport_mut().window_size = (width, height);
        self.viewport_changed = true;
        self.held = false;
        self.pending.queue();
        let size = viewport_layout_size(self.doc.inner().viewport());
        let _ = self.sender.unbounded_send(WindowEvent::Viewport { size });
    }

    /// One `RedrawRequested`, as the launch copy answers it (D9): a frame is
    /// held while the drain has not read the queued size, and it presents
    /// otherwise.
    fn redraw(&mut self) {
        if self.pending.is_pending() && !self.held {
            self.held = true;
            self.poll();
            return;
        }
        self.held = false;
        self.present_frame();
    }

    /// The poll a held frame queues: the shell drains the window channel, applies
    /// the render's mutations, and asks for the redraw that follows.
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
        self.held = false;
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

    /// The arrangement of every frame presented so far, in order.
    fn presented_arrangements(&self) -> Vec<String> {
        self.arrangements.borrow().clone()
    }
}

/// Contract (the live resize jerk of `e978b31aa`, D9): the first frame at the new
/// size already lays out the arrangement that size picks. A resize from the Wide
/// arrangement to Mini crosses the Library column's threshold and drops the Queue
/// column; the frame the resize asked for is held until the size reaches the
/// tree, so no frame resolves at the new size with the old arrangement.
#[test]
fn the_first_frame_at_a_new_size_paints_its_arrangement() {
    let mut window = Window::built(1600, 900);
    assert_eq!(window.arrangement(), "wide");
    assert!(window.paints_the_queue_column());

    window.resize(800, 900);
    window.redraw();
    assert_eq!(
        window.presented, 1,
        "the frame the resize asked for is held until the size reaches the tree"
    );

    window.redraw();
    assert_eq!(window.arrangement(), "mini");
    assert!(!window.paints_the_queue_column());
    assert_eq!(window.presented_arrangements(), vec!["wide", "mini"]);
}
