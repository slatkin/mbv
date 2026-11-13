//! The launch copy (D6 of `blitz-queue-panel-spike`): the body of
//! `launch_cfg_with_props` from `dioxus-native` 0.8.0-alpha.3, with this
//! crate's net provider, no navigation provider, and this crate's window
//! attributes. The copy starts no tokio runtime, because the `net` feature is
//! off (D13).

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use blitz_shell::{BlitzShellProxy, WindowConfig, create_default_event_loop};
use blitz_traits::net::NetProvider;
use dioxus_native::prelude::*;
use dioxus_native::winit::platform::wayland::WindowAttributesWayland;
use dioxus_native::winit::window::WindowAttributes;
use dioxus_native::{
    DioxusDocument, DioxusNativeApplication, DioxusNativeWindowRenderer, DocumentConfig,
    RendererOptions,
};
use mbv_gui::bridge::sections::GuiSectionBatch;
use mbv_gui::core_thread::CoreInput;

use crate::art_net::MbvNetProvider;

/// The Wayland app id of the window (D14). It differs from the GUI's id, so
/// the two windows do not share a desktop entry.
const APP_ID: &str = "io.github.slatkin.mbv-native";

/// The section-batch receiver, shared by the root context. The root component
/// takes it once, and later takes see `None`.
#[derive(Clone, Debug)]
pub struct BatchSlot(Arc<Mutex<Option<UnboundedReceiver<GuiSectionBatch>>>>);

impl BatchSlot {
    /// A slot that holds `batches` until the first `take`.
    #[must_use]
    pub fn new(batches: UnboundedReceiver<GuiSectionBatch>) -> Self {
        Self(Arc::new(Mutex::new(Some(batches))))
    }

    /// The receiver, for the first caller only.
    #[must_use]
    pub fn take(&self) -> Option<UnboundedReceiver<GuiSectionBatch>> {
        self.0.lock().ok()?.take()
    }
}

/// Open the window and run its event loop until the window closes. `app` is
/// the root component, `core` sends art requests and close inputs to the GUI
/// core thread, and `batches` carries the section batches the core sends.
pub fn launch(
    app: fn() -> Element,
    core: Sender<CoreInput>,
    batches: UnboundedReceiver<GuiSectionBatch>,
) {
    let event_loop = create_default_event_loop();
    let winit_proxy = event_loop.create_proxy();
    let (proxy, event_queue) = BlitzShellProxy::new(winit_proxy);

    let net_provider: Arc<dyn NetProvider> = Arc::new(MbvNetProvider::new(core));
    let vdom = VirtualDom::new(app);
    vdom.provide_root_context(Arc::clone(&net_provider));
    vdom.provide_root_context(BatchSlot::new(batches));

    let doc = DioxusDocument::new(
        vdom,
        DocumentConfig {
            net_provider: Some(net_provider),
            navigation_provider: None,
            font_ctx: None,
            ..Default::default()
        },
    );
    let renderer = DioxusNativeWindowRenderer::with_options(RendererOptions {
        base_color: None,
        alpha_mode: None,
        features: None,
        limits: None,
    });
    let attributes = WindowAttributes::default()
        .with_title("mbv")
        .with_platform_attributes(Box::new(
            WindowAttributesWayland::default().with_name(APP_ID, "mbv-native"),
        ));
    let config = WindowConfig::with_attributes(Box::new(doc) as _, renderer, attributes);
    let application = DioxusNativeApplication::new(proxy, event_queue, config);
    event_loop
        .run_app(application)
        .expect("the window event loop runs");
}
