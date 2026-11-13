//! The launch copy (D5 of `replace-tauri-with-blitz`): the body of
//! `launch_cfg_with_props` from `dioxus-native` 0.8.0-alpha.3, with this
//! crate's window attributes, the `BlitzShellProxy` as a root context, the
//! window event handler wrapped for `WindowEvent::Viewport` (D9), and no
//! tokio runtime. The pinned crate source is the authority for the copy; the
//! spike's `crates/mbv-native/src/launch.rs` at commit `2a0510d2a` is a
//! reference.

use std::sync::Arc;
use std::sync::mpsc::Sender;

use blitz_dom::Document as _;
use blitz_shell::{BlitzShellEvent, BlitzShellProxy, WindowConfig, create_default_event_loop};
use blitz_traits::net::NetProvider;
use dioxus_native::prelude::*;
use dioxus_native::winit::application::ApplicationHandler;
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
use crate::window::net::MbvNetProvider;

/// The Wayland app id of the window (D5). It is the GUI's current id, so the
/// desktop entry still matches.
const APP_ID: &str = "io.github.slatkin.mbv-gui";

/// The dioxus-native application with its window event handler wrapped (D5):
/// every event delegates, and the window events the window half consumes
/// report after the shell has handled them (D9, D10).
struct MbvApplication {
    inner: DioxusNativeApplication,
    proxy: BlitzShellProxy,
    /// The document's id, so the frame hook requests the next redraw (D10).
    doc_id: usize,
    events: WindowEventSender,
    clock: Arc<dyn Clock>,
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
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.proxy_wake_up(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.about_to_wait(event_loop);
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
        let redraw = matches!(event, WinitWindowEvent::RedrawRequested);
        self.inner.window_event(event_loop, window_id, event);
        if viewport {
            let _ = self.events.unbounded_send(WindowEvent::Viewport);
        }
        // D10: while frames are wanted, every presented frame reports and
        // requests the next redraw, which paces the loop at vsync.
        if redraw && self.clock.frames_wanted() {
            let _ = self.events.unbounded_send(WindowEvent::Frame);
            self.proxy.send_event(BlitzShellEvent::RequestRedraw {
                doc_id: self.doc_id,
            });
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
    let net_provider: Arc<dyn NetProvider> =
        Arc::new(MbvNetProvider::new(core.clone(), events.clone()));
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
    let renderer = DioxusNativeWindowRenderer::with_options(RendererOptions {
        base_color: None,
        alpha_mode: None,
        features: None,
        limits: None,
    });
    let attributes = WindowAttributes::default()
        .with_title("mbv")
        .with_platform_attributes(Box::new(
            WindowAttributesWayland::default().with_name(APP_ID, "mbv"),
        ));
    let config = WindowConfig::with_attributes(Box::new(doc) as _, renderer, attributes);
    let application = DioxusNativeApplication::new(proxy.clone(), event_queue, config);

    let application = MbvApplication {
        inner: application,
        proxy,
        doc_id,
        events,
        clock,
    };
    event_loop
        .run_app(application)
        .expect("the window event loop runs");
}
