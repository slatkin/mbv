//! The launch copy (D5 of `replace-tauri-with-blitz`): the body of
//! `launch_cfg_with_props` from `dioxus-native` 0.8.0-alpha.3, with this
//! crate's window attributes, the `BlitzShellProxy` as a root context, the
//! window event handler wrapped for `WindowEvent::Viewport` (D9) and
//! `WindowEvent::Frame` (D10), the frame's change fact the drain's re-measure
//! reads (D12), the resolve that keeps hit testing off a removed node (gap
//! `G-Hit`), and — `This crate:` — the pointer target the window reads from
//! Blitz's hover node after each event (`gui-pointer-target-interaction`
//! design D1). The pinned crate source is the authority
//! for the copy; the spike's launch copy at commit
//! `2a0510d2a` is a reference.
//!
//! [`MbvApplication`] is the copy of `dioxus_application::DioxusNativeApplication`
//! from the same crate, owned here instead of borrowed: the window half has to
//! reach the `DioxusDocument` it drives, because a resize's size must be written
//! into the tree and rendered *before* the frame that resolves at the new size
//! (D9). `DioxusNativeApplication` keeps its `BlitzApplication` private, and a
//! `DioxusDocument` moved into a view cannot be reached from outside. The
//! differences from the pinned source are marked `This crate:` at each site.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use blitz_dom::{BaseDocument, Document as _};
use blitz_shell::{
    BlitzApplication, BlitzShellEvent, BlitzShellProxy, View, WindowConfig,
    create_default_event_loop,
};
use blitz_traits::net::NetProvider;
use dioxus_native::prelude::*;
use dioxus_native::winit::application::ApplicationHandler;
use dioxus_native::winit::dpi::LogicalSize;
use dioxus_native::winit::event::{StartCause, WindowEvent as WinitWindowEvent};
use dioxus_native::winit::event_loop::ActiveEventLoop;
use dioxus_native::winit::platform::wayland::WindowAttributesWayland;
use dioxus_native::winit::window::{WindowAttributes, WindowId};
use dioxus_native::{
    DioxusDocument, DioxusNativeEvent, DioxusNativeWindowRenderer, DocumentConfig, RendererOptions,
};

use crate::core_thread::CoreInput;
use crate::window::app::ROOT_ELEMENT_ID;
use crate::window::channel::{CoreLink, EventSlot, WindowEvent, WindowEventSender};
use crate::window::clock::Clock;
use crate::window::layout::{
    ViewportSize, apply_viewport_size, carries_unresolved_change, frame_carries_change,
    viewport_layout_size,
};
use crate::window::net::{MbvNetProvider, NetLoads};
use crate::window::pointer_target::{PointerTargetStore, hover_chain, pointer_target};

/// The Wayland app id of the window (D5). It is the GUI's current id, so the
/// desktop entry still matches.
const APP_ID: &str = "io.github.slatkin.mbv-gui";

/// The minimum surface width, in logical pixels (D5). The window sets
/// `minWidth` 400, so a floating window cannot shrink below the width where
/// the compact strip lays out.
const MIN_SURFACE_WIDTH: f64 = 400.0;

/// The minimum surface height, in logical pixels (D5). The window sets
/// `minHeight` 200.
const MIN_SURFACE_HEIGHT: f64 = 200.0;

/// The layout of the window the launch copy opens (D5): the title, the
/// minimum surface size the earlier window carried, and the Wayland app id.
/// Pure, so the attributes are checkable without a window.
fn window_attributes() -> WindowAttributes {
    WindowAttributes::default()
        .with_title("mbv")
        .with_min_surface_size(LogicalSize::new(MIN_SURFACE_WIDTH, MIN_SURFACE_HEIGHT))
        .with_platform_attributes(Box::new(
            WindowAttributesWayland::default().with_name(APP_ID, "mbv"),
        ))
}

/// Whether Blitz answers `event` by hit testing (gap `G-Hit`):
/// `blitz_shell::View::handle_winit_event` resolves each of these through
/// `BaseDocument::hit`, which walks `Node::paint_children` down to the node
/// under the point. Every other window event reaches no hit test.
fn hit_tests(event: &WinitWindowEvent) -> bool {
    matches!(
        event,
        WinitWindowEvent::PointerMoved { .. }
            | WinitWindowEvent::PointerButton { .. }
            | WinitWindowEvent::PointerLeft { .. }
            | WinitWindowEvent::MouseWheel { .. }
    )
}

/// The animation clock the launch copy's pre-hit resolves pass (gap `G-Hit`).
///
/// Blitz-shell starts its own clock when it creates the window's surfaces
/// (`View::resume`), and stylo dates a transition it starts from the resolve
/// time it is given and clamps the transition's progress to zero while the
/// resolve time is before that date (`style`'s `Transition::calculate_value`).
/// A clock that ran ahead of the shell's would therefore freeze every
/// transition the pre-hit resolve started until a frame's resolve caught up
/// with it, so this one is anchored after `View::resume` and only ever trails
/// the shell's. It reports 0.0 until then, as the shell's own clock does.
#[derive(Default)]
struct AnimationClock {
    started: Option<Instant>,
}

impl AnimationClock {
    /// The shell created the window's surfaces, which started its clock: this
    /// one starts after that point, so the two agree to within the window's
    /// setup time.
    fn shell_started(&mut self) {
        self.started = Some(Instant::now());
    }

    /// The time a resolve passes, in seconds since the shell created the
    /// window's surfaces.
    fn time(&self) -> f64 {
        self.started
            .map_or(0.0, |started| started.elapsed().as_secs_f64())
    }
}

/// Resolve the document before an input event reaches hit testing (gap
/// `G-Hit`): `paint_children` refreshes only at resolve, so between a DOM
/// change and the next frame it still names the nodes the change removed, and
/// a hit test over them unwraps a missing node id and panics (blitz-dom
/// `node.rs`, `Node::hit_inner`). The frame's own resolve follows within a
/// frame; this one is gated on the document's damage so mouse movement over an
/// unchanged tree costs one flag read. Returns whether it resolved, so the next
/// frame carries the change this resolve applied and its measure re-runs
/// (D12).
fn resolve_before_hit_testing(
    document: &Rc<RefCell<BaseDocument>>,
    animation: &AnimationClock,
) -> bool {
    let mut document = document.borrow_mut();
    if !carries_unresolved_change(&document) {
        return false;
    }
    document.resolve(animation.time());
    true
}

/// The dioxus-native application, owned by this crate (D5): the copy of
/// `DioxusNativeApplication` (`dioxus-native` 0.8.0-alpha.3
/// `src/dioxus_application.rs`), so the window's views are reachable and a
/// resize can be rendered into the tree before the frame that resolves at it.
///
/// Every field and method below is the pinned source's unless the comment says
/// `This crate:`. Those are: the `event_handlers` field (its type,
/// `event_handlers::WindowEventHandlers`, is `pub(crate)` in the pinned crate,
/// and this window registers no winit event handler — nothing here calls
/// `use_window_event` or `use_back_button`), `add_window` (this window opens
/// exactly one, which `new` carries as the pending window), and the hooks the
/// window half adds (D9 size application, gap `G-Hit` resolve, D10 frames, and
/// the pointer target of `gui-pointer-target-interaction` D1).
/// The window events the window half consumes report after the shell has
/// handled them (D9, D10).
struct MbvApplication {
    /// The source's `pending_window`.
    pending_window: Option<WindowConfig<DioxusNativeWindowRenderer>>,
    /// The source's `inner`: the Blitz window set, its proxy, and its event
    /// queue. `This crate:` the source keeps it private, and the viewport
    /// handling below reads `inner.windows` directly.
    inner: BlitzApplication<DioxusNativeWindowRenderer>,
    /// The document, shared with the shell's copy (gap `G-Hit`): the launch
    /// copy resolves it before a hit-testing event.
    document: Rc<RefCell<BaseDocument>>,
    /// The animation clock those resolves pass (gap `G-Hit`): anchored when
    /// the shell created its surfaces, so it never runs ahead of the clock the
    /// shell dates its own resolves with.
    animation: AnimationClock,
    /// The document's id, so a net answer requests the frame it needs (D12).
    doc_id: usize,
    events: WindowEventSender,
    /// `This crate:` the root's layout-size signal, written at every resize
    /// before the frame that resolves at it (D9).
    viewport_size: ViewportSize,
    /// Whether a viewport change has landed since the last presented frame
    /// (D9): the frame fact reports it so the measure follows the frame that
    /// resolves at the new size, even when the resize changed no markup.
    viewport_changed: bool,
    /// Whether the pre-hit resolve applied a change since the last presented
    /// frame (gap `G-Hit`, D12): that resolve is not a frame, so the next frame
    /// reports the change and the measure re-runs.
    resolved_early: bool,
    /// The net answers the document has fetched since the last presented frame
    /// (D7, D12): Blitz applies a font or an image inside a resolve and lays out
    /// again in the same pass, so both change measured rects with no tree change
    /// to read before the resolve. The provider sets this, the frame takes it,
    /// and the loop presents the frame it needs.
    loads: NetLoads,
    /// Whether that frame was already asked for. One ask per pending answer
    /// keeps the idle loop from redrawing again and again until the frame takes
    /// it.
    load_framed: bool,
    /// `This crate:` the root's pointer-target signal
    /// (`gui-pointer-target-interaction` D1), written after each window event
    /// from the node Blitz resolved as hovered, and only when it changed.
    pointer_target: PointerTargetStore,
}

impl MbvApplication {
    /// `This crate:` the source's `new`, over the document the window half also
    /// holds.
    fn new(
        proxy: BlitzShellProxy,
        event_queue: Receiver<BlitzShellEvent>,
        config: WindowConfig<DioxusNativeWindowRenderer>,
        document: Rc<RefCell<BaseDocument>>,
        doc_id: usize,
        events: WindowEventSender,
        viewport_size: ViewportSize,
        loads: NetLoads,
        pointer_target: PointerTargetStore,
    ) -> Self {
        Self {
            pending_window: Some(config),
            inner: BlitzApplication::new(proxy, event_queue),
            document,
            animation: AnimationClock::default(),
            doc_id,
            events,
            viewport_size,
            viewport_changed: false,
            resolved_early: false,
            loads,
            load_framed: false,
            pointer_target,
        }
    }

    /// Present the frame a net answer needs (D12): the answer arrives outside
    /// any frame, and the frame that reports it resolves the layout it changed
    /// and bumps the layout epoch. One frame per answer; the frame takes the
    /// flag.
    fn present_load_frame(&mut self) {
        if !self.loads.pending() {
            self.load_framed = false;
            return;
        }
        if self.load_framed {
            return;
        }
        self.load_framed = true;
        self.inner.proxy.send_event(BlitzShellEvent::RequestRedraw {
            doc_id: self.doc_id,
        });
    }

    /// The source's `handle_dioxus_native_event`, with one arm: `This crate:`
    /// the source's devtools arm is `#[cfg(feature = "hot-reload")]`, and this
    /// crate enables neither that feature nor devtools, so only the head
    /// element route remains.
    fn handle_dioxus_native_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        event: &DioxusNativeEvent,
    ) {
        match event {
            DioxusNativeEvent::CreateHeadElement {
                name,
                attributes,
                contents,
                window,
            } => {
                if let Some(window) = self.inner.windows.get_mut(window) {
                    let doc = window.downcast_doc_mut::<DioxusDocument>();
                    doc.create_head_element(name, attributes, contents);
                    window.poll();
                }
            }
        }
    }
}

impl ApplicationHandler for MbvApplication {
    /// The source's `macos_handler`: the shell's own.
    #[cfg(target_os = "macos")]
    fn macos_handler(&mut self) -> Option<&mut dyn ApplicationHandlerExtMacOS> {
        self.inner.macos_handler()
    }

    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.resumed(event_loop);
    }

    fn suspended(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }

    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.destroy_surfaces(event_loop);
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        self.inner.new_events(event_loop, cause);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.about_to_wait(event_loop);
        // `This crate:` the loop is idle, so a net answer that landed since the
        // last frame needs one more presented frame (D12).
        self.present_load_frame();
    }

    /// The source's `can_create_surfaces`, over this crate's root contexts.
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(config) = self.pending_window.take() {
            let mut window = View::init(config, event_loop, &self.inner.proxy);
            let winit_window = Arc::clone(&window.window);
            let renderer = window.renderer.clone();
            let window_id = window.window_id();
            let doc = window.downcast_doc_mut::<DioxusDocument>();

            // `This crate:` the source provides the `dioxus_document::Document`
            // context and its winit event handlers here. Both types are
            // `pub(crate)` in the pinned crate (`contexts::DioxusNativeDocument::new`
            // and `event_handlers::WindowEventHandlers`), so this copy cannot
            // build them; the window consumes neither (it asks no document for a
            // head element and registers no winit event handler). The source's
            // history context is absent too: nothing in this window routes, and
            // `dioxus-history` is not a dependency here. The source's `tracing`
            // debug line above this block is omitted with them.

            // Add shell provider
            let shell_provider = Arc::<dyn blitz_traits::shell::ShellProvider>::clone(
                &doc.inner.borrow().shell_provider,
            );
            doc.vdom
                .in_scope(ScopeId::ROOT, move || provide_context(shell_provider));

            // Add renderer
            doc.vdom
                .in_scope(ScopeId::ROOT, move || provide_context(renderer));

            // Add winit window
            doc.vdom
                .in_scope(ScopeId::ROOT, move || provide_context(winit_window));

            // Queue rebuild
            doc.initial_build();

            // And then request redraw
            window.request_redraw();

            self.inner.windows.insert(window_id, window);
        }

        self.inner.can_create_surfaces(event_loop);
        // `This crate:` the shell created the window and resolved it for the
        // first time, so its animation clock has started: anchor ours after it
        // (gap `G-Hit`).
        self.animation.shell_started();
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WinitWindowEvent,
    ) {
        // D5: send after the shell has handled the event, so the window's
        // layout reads the size it now has (D9).
        let viewport = matches!(
            &event,
            WinitWindowEvent::SurfaceResized(_) | WinitWindowEvent::ScaleFactorChanged { .. }
        );
        if viewport {
            self.viewport_changed = true;
        }
        let redraw = matches!(event, WinitWindowEvent::RedrawRequested);
        // Gap `G-Hit`: the hit test runs inside the shell's handler, so the
        // resolve that makes `paint_children` current runs before it.
        if hit_tests(&event) && resolve_before_hit_testing(&self.document, &self.animation) {
            self.resolved_early = true;
        }
        // The frame fact is read before the shell resolves (D12): a change
        // resolved after it — a batch or a pointer event the drain applies in
        // the same turn — must not be consumed by this frame, or its own frame
        // would report nothing and no measure would follow the resolve.
        let changed = if redraw {
            let loads_landed = self.loads.take();
            frame_carries_change(
                &self.document.borrow(),
                self.viewport_changed,
                self.resolved_early,
            ) || loads_landed
        } else {
            false
        };
        // The source's `event_handlers.apply_event` would run here; this copy
        // holds no handlers (see the struct's note).
        self.inner.window_event(event_loop, window_id, event);
        // `This crate:` D8. A click Blitz clears the DOM focus of, and the
        // removal of a focused node, would otherwise leave the window with no
        // keyboard at all (see [`restore_root_focus`]).
        restore_root_focus(&mut self.document.borrow_mut());
        // `This crate:` `gui-pointer-target-interaction` D1. Blitz resolved the
        // hovered node inside the shell's handler (a frame resolves and
        // refreshes it, a scroll too), so the pointer target is read after it
        // and written into the root's signal while the window still paints the
        // node it names.
        self.apply_pointer_target(window_id);
        if viewport {
            self.apply_viewport(window_id);
        }
        // `This crate:` D12. Every presented frame reports its change fact: a
        // change no frame has resolved yet is answered by a frame's bump. An
        // idle window presents none: nothing here asks for one, and
        // blitz-shell's poll asks for a redraw on its own when the tree or a
        // net answer gives it work.
        if redraw {
            self.viewport_changed = false;
            self.resolved_early = false;
            let _ = self.events.unbounded_send(WindowEvent::Frame { changed });
        }
    }

    /// The source's `proxy_wake_up`, over the shell's own events (`This crate:`
    /// the embedder route is the source's).
    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        while let Ok(event) = self.inner.event_queue.try_recv() {
            match event {
                BlitzShellEvent::Embedder(event) => {
                    if let Some(event) = event.downcast_ref::<DioxusNativeEvent>() {
                        self.handle_dioxus_native_event(event_loop, event);
                    }
                }
                event => self.inner.handle_blitz_shell_event(event_loop, event),
            }
        }
    }
}

/// Give the root element the DOM focus back when it sits outside the window's
/// tree ([`ROOT_ELEMENT_ID`], D8).
///
/// The shell delivers a keydown to the document's focused node, and the
/// window's one `keydown` handler lives on the root. Blitz clears the focus
/// after a click that matches no text input, checkbox, radio, summary, label
/// or link (blitz-dom's `handle_click`'s `if !matched { clear_focus() }`) —
/// a press on a queue row is such a click — and the focus then lands on the
/// document's `<html>` element, which carries no Dioxus id: the keydown
/// reaches no handler and every key in the window is dead from the first row
/// click on (live run of `replace-tauri-with-blitz`). A focused node the
/// click removed (a closed search field) leaves the focus the same way. A
/// node inside the root — a text input a click landed on, a field a shortcut
/// armed — keeps the focus, so only the outside ones are answered here.
fn restore_root_focus(document: &mut BaseDocument) {
    let Some(root) = document.get_element_by_id(ROOT_ELEMENT_ID) else {
        return;
    };
    if focus_within(document, root) {
        return;
    }
    document.set_focus_to(root);
}

/// Whether the document's focus sits inside `root`, the window's own tree: the
/// focused node or one of its ancestors is `root`. Focus on the document's
/// `<html>` (`NodeId` 2 in a document built by `blitz-dom`) is not inside it,
/// and neither is no focus at all.
fn focus_within(document: &BaseDocument, root: blitz_dom::NodeId) -> bool {
    let mut candidate = document.get_focussed_node_id();
    while let Some(node) = candidate {
        if node == root {
            return true;
        }
        candidate = document.get_node(node).and_then(|node| node.parent);
    }
    false
}

impl MbvApplication {
    /// `This crate:` the pointer target (`gui-pointer-target-interaction` D1).
    ///
    /// The node under the pointer is Blitz's own: `BaseDocument::resolve`
    /// refreshes the hovered node from the last pointer position, so the walk
    /// below reads the row that shows the `:hover` look even after a scroll
    /// under a still pointer. The chain is read from the document the shell
    /// shares with this copy, and the value is written from the vdom's runtime
    /// only when it changed, as a resize writes the viewport size (D9).
    fn apply_pointer_target(&mut self, window_id: WindowId) {
        let target = {
            let document = self.document.borrow();
            pointer_target(&hover_chain(&document))
        };
        if let Some(view) = self.inner.windows.get_mut(&window_id) {
            let doc = view.downcast_doc_mut::<DioxusDocument>();
            self.pointer_target.set(&doc.vdom, target);
        }
    }

    /// `This crate:` apply a resize to the window's tree (D9).
    ///
    /// The shell has already put the new size on the document's viewport
    /// (`blitz_shell::View::handle_winit_event`), without touching the tree, so
    /// the size has to reach the tree here: the size signal is written from the
    /// vdom's runtime and the tree is rendered, all before the redraw the
    /// resize asked for resolves. That frame then lays out the arrangement,
    /// panel state, and column widths the new size picks, and a continuous drag
    /// presents every frame instead of holding them for a channel hop that lands
    /// a turn late.
    fn apply_viewport(&mut self, window_id: WindowId) {
        let size = viewport_layout_size(self.document.borrow().viewport());
        if let Some(view) = self.inner.windows.get_mut(&window_id) {
            let doc = view.downcast_doc_mut::<DioxusDocument>();
            apply_viewport_size(doc, self.viewport_size.clone(), size);
        }
        // The art caps divide by the scale factor (D7), which the drain writes
        // from the window on this event. The layout size does not travel here;
        // it reached the tree above.
        let _ = self.events.unbounded_send(WindowEvent::Viewport);
    }
}

/// Open the window and run its event loop until the window closes (D5). `app`
/// is the root component, `core` is the core thread's channel (D4), and
/// `events` with `event_receiver` are the window channel's two ends: every
/// producer pushes through the sender, and the root drains the receiver.
///
/// # Panics
///
/// Panics when the window event loop fails to run (surface creation failed,
/// for example a host with no GPU, gap G14).
pub fn launch(
    app: fn() -> Element,
    core: Sender<CoreInput>,
    events: WindowEventSender,
    event_receiver: UnboundedReceiver<WindowEvent>,
    clock: Arc<dyn Clock>,
) {
    let event_loop = create_default_event_loop();
    let winit_proxy = event_loop.create_proxy();
    let (proxy, event_queue) = BlitzShellProxy::new(winit_proxy);

    // Build the virtual DOM first; the window-bound contexts attach as root
    // contexts, as the original copy does. The net provider is this crate's
    // (D7), and the root's context copy lets the document's fetch paths find
    // it, as the original copy's provider context does.
    let vdom = VirtualDom::new(app);
    // The net answers the document fetches (D7, D12): the provider sets the
    // flag and this loop presents the frame whose report resolves and measures
    // them.
    let loads = NetLoads::new();
    let net_provider: Arc<dyn NetProvider> = Arc::new(MbvNetProvider::new(
        core.clone(),
        events.clone(),
        loads.clone(),
    ));
    vdom.provide_root_context(CoreLink::new(core));
    vdom.provide_root_context(Arc::clone(&net_provider));
    vdom.provide_root_context(clock);
    vdom.provide_root_context(EventSlot::new(event_receiver));
    // The root hands its layout-size signal back through this handle; a resize
    // writes the new size into it before the frame that resolves at it (D9).
    let viewport_size = ViewportSize::new();
    vdom.provide_root_context(viewport_size.clone());
    // The root hands its pointer-target signal back through this handle; the
    // window writes the hovered node's target into it after each event
    // (`gui-pointer-target-interaction` D1).
    let pointer_target_store = PointerTargetStore::new();
    vdom.provide_root_context(pointer_target_store.clone());
    vdom.provide_root_context(proxy.clone());

    let doc = DioxusDocument::new(
        vdom,
        DocumentConfig {
            net_provider: Some(net_provider), // D7
            navigation_provider: None,        // D5: no navigation provider
            font_ctx: None,                   // D5
            ..Default::default()
        },
    );
    let doc_id = doc.id();
    let document = Rc::clone(&doc.inner);
    let renderer = DioxusNativeWindowRenderer::with_options(RendererOptions {
        base_color: None,
        alpha_mode: None,
        features: None,
        limits: None,
    });
    let config = WindowConfig::with_attributes(Box::new(doc) as _, renderer, window_attributes());

    let application = MbvApplication::new(
        proxy,
        event_queue,
        config,
        document,
        doc_id,
        events,
        viewport_size,
        loads,
        pointer_target_store,
    );
    event_loop
        .run_app(application)
        .expect("the window event loop runs");
}

#[cfg(test)]
mod tests;
