//! The launch copy (D5 of `replace-tauri-with-blitz`): the body of
//! `launch_cfg_with_props` from `dioxus-native` 0.8.0-alpha.3, with this
//! crate's window attributes, the `BlitzShellProxy` as a root context, the
//! window event handler wrapped for `WindowEvent::Viewport` (D9) and
//! `WindowEvent::Frame` (D10), the frame's change fact the drain's re-measure
//! reads (D12), and the resolve that keeps hit testing off a removed node (gap
//! `G-Hit`). The pinned crate source is the authority
//! for the copy; the spike's `crates/mbv-native/src/launch.rs` at commit
//! `2a0510d2a` is a reference.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use blitz_dom::{BaseDocument, Document as _};
use blitz_shell::{BlitzShellEvent, BlitzShellProxy, WindowConfig, create_default_event_loop};
use blitz_traits::net::NetProvider;
use dioxus_native::prelude::*;
use dioxus_native::winit::application::ApplicationHandler;
use dioxus_native::winit::dpi::LogicalSize;
use dioxus_native::winit::event::{StartCause, WindowEvent as WinitWindowEvent};
use dioxus_native::winit::event_loop::ActiveEventLoop;
use dioxus_native::winit::platform::wayland::WindowAttributesWayland;
use dioxus_native::winit::window::{WindowAttributes, WindowId};
use dioxus_native::{
    DioxusDocument, DioxusNativeApplication, DioxusNativeWindowRenderer, DocumentConfig,
    RendererOptions,
};

use crate::core_thread::CoreInput;
use crate::window::channel::{CoreLink, EventSlot, WindowEvent, WindowEventSender};
use crate::window::clock::Clock;
use crate::window::layout::{carries_unresolved_change, frame_carries_change};
use crate::window::net::{MbvNetProvider, NetLoads};

/// The Wayland app id of the window (D5). It is the GUI's current id, so the
/// desktop entry still matches.
const APP_ID: &str = "io.github.slatkin.mbv-gui";

/// The minimum surface width, in logical pixels (D5). The Tauri window set
/// `minWidth` 400, so a floating window cannot shrink below the width where
/// the compact strip lays out.
const MIN_SURFACE_WIDTH: f64 = 400.0;

/// The minimum surface height, in logical pixels (D5). The Tauri window set
/// `minHeight` 200.
const MIN_SURFACE_HEIGHT: f64 = 200.0;

/// The layout of the window the launch copy opens (D5): the title, the
/// minimum surface size the Tauri window carried, and the Wayland app id.
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

/// The dioxus-native application with its window event handler wrapped (D5):
/// every event delegates, and the window events the window half consumes
/// report after the shell has handled them (D9, D10).
struct MbvApplication {
    inner: DioxusNativeApplication,
    proxy: BlitzShellProxy,
    /// The document, shared with the shell's copy (gap `G-Hit`): the launch
    /// copy resolves it before a hit-testing event.
    document: Rc<RefCell<BaseDocument>>,
    /// The animation clock those resolves pass (gap `G-Hit`): anchored when
    /// the shell created its surfaces, so it never runs ahead of the clock the
    /// shell dates its own resolves with.
    animation: AnimationClock,
    /// The document's id, so the frame hook requests the next redraw (D10).
    doc_id: usize,
    events: WindowEventSender,
    clock: Arc<dyn Clock>,
    /// Whether a viewport change has landed since the last presented frame
    /// (D9): Blitz applies the new size in its own handler without touching the
    /// tree, so the frame fact cannot read it from the document.
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
}

impl MbvApplication {
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
        self.proxy.send_event(BlitzShellEvent::RequestRedraw {
            doc_id: self.doc_id,
        });
    }
}

impl ApplicationHandler for MbvApplication {
    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        self.inner.new_events(event_loop, cause);
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

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.can_create_surfaces(event_loop);
        // The shell created the window and resolved it for the first time, so
        // its animation clock has started: anchor ours after it (gap `G-Hit`).
        self.animation.shell_started();
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.proxy_wake_up(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.about_to_wait(event_loop);
        // The loop is idle: a net answer that landed since the last frame needs
        // one more presented frame (D12).
        self.present_load_frame();
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
        self.inner.window_event(event_loop, window_id, event);
        if viewport {
            let _ = self.events.unbounded_send(WindowEvent::Viewport);
        }
        // D10: every presented frame reports its change fact, and the next
        // redraw is requested only while frames are wanted, which paces the
        // loop at vsync. The report itself is never gated: a change no frame
        // has resolved yet is answered by a frame's bump (D12), which a paused
        // window must report too. An idle window presents none: nothing here
        // asks for one, and blitz-shell's poll asks for a redraw on its own
        // when the tree or a net answer gives it work.
        if redraw {
            self.viewport_changed = false;
            self.resolved_early = false;
            let _ = self.events.unbounded_send(WindowEvent::Frame { changed });
            if self.clock.frames_wanted() {
                self.proxy.send_event(BlitzShellEvent::RequestRedraw {
                    doc_id: self.doc_id,
                });
            }
        }
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
    vdom.provide_root_context(Arc::clone(&clock));
    vdom.provide_root_context(EventSlot::new(event_receiver));
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
    let application = DioxusNativeApplication::new(proxy.clone(), event_queue, config);

    let application = MbvApplication {
        inner: application,
        proxy,
        document,
        animation: AnimationClock::default(),
        doc_id,
        events,
        clock,
        viewport_changed: false,
        resolved_early: false,
        loads,
        load_framed: false,
    };
    event_loop
        .run_app(application)
        .expect("the window event loop runs");
}

#[cfg(test)]
mod tests;
