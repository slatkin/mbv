//! Headless tests of the daemon-lost recovery dialog (D16): the contracts of
//! `DaemonLostOverlay.test.ts` that the window's port keeps — the dialog
//! paints the diagnostics the frame carries and the recovery choices the
//! core projects, a frame with nothing playing paints no title line, and the
//! affirmative choice paints the accent text token on the recessed fill. The
//! press-forward half of the former suite's last test is the input layer's
//! contract (`window/input.rs` tests own it); the region half is the paint
//! test here. The core channel is a mock pair; no test opens a window or
//! sleeps.

use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::traversal::TreeTraverser;
use blitz_dom::{Document as _, local_name};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::DaemonLostOverlay;
use crate::bridge::frame::overlay::{GuiConfirmButton, GuiConfirmButtonTone};
use crate::bridge::layout::{DaemonLostTargetId, RegionTarget};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::layout::region_set::last_reported_targets;
use crate::window::layout::{LayoutEpoch, use_layout_reporter};

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    last_playing_title: Option<String>,
    daemon_log_path: String,
    restart_error: Option<String>,
    buttons: Vec<GuiConfirmButton>,
}

/// The root: provides the reporter's contexts and paints the dialog.
fn daemon_lost_app(props: FixtureProps) -> Element {
    provide_context(CoreLink::new(props.sender));
    provide_context(use_hook(LayoutEpoch::new));
    let reporter = use_layout_reporter();
    provide_context(reporter);
    rsx! {
        DaemonLostOverlay {
            last_playing_title: props.last_playing_title,
            daemon_log_path: props.daemon_log_path,
            restart_error: props.restart_error,
            buttons: props.buttons,
        }
    }
}

struct Fixture {
    doc: DioxusDocument,
    receiver: Receiver<CoreInput>,
}

/// The dialog the core projects: a lost Owner that was playing something,
/// with the three recovery choices the TUI's hint line offers.
fn fixture_props(sender: Sender<CoreInput>) -> FixtureProps {
    FixtureProps {
        sender,
        last_playing_title: Some("Track One".to_owned()),
        daemon_log_path: "/home/user/.local/state/mbv/local-daemon.log".to_owned(),
        restart_error: None,
        buttons: vec![
            GuiConfirmButton {
                label: "Restart and resume".to_owned(),
                keys: "R".to_owned(),
                tone: GuiConfirmButtonTone::Affirmative,
            },
            GuiConfirmButton {
                label: "Restart, don't resume".to_owned(),
                keys: "S".to_owned(),
                tone: GuiConfirmButtonTone::Affirmative,
            },
            GuiConfirmButton {
                label: "Quit".to_owned(),
                keys: "Q".to_owned(),
                tone: GuiConfirmButtonTone::Cancel,
            },
        ],
    }
}

impl Fixture {
    /// The whole text the document paints.
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

    /// The `style` attribute of the node the selector names.
    fn style_of(&self, selector: &str) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")?;
        let node = dom.get_node(id)?;
        node.attr(local_name!("style")).map(str::to_owned)
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed.
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }
}

fn built_with(adjust: impl FnOnce(&mut FixtureProps)) -> Fixture {
    let (sender, receiver) = channel();
    let mut props = fixture_props(sender);
    adjust(&mut props);
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(daemon_lost_app, props),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let mut fixture = Fixture { doc, receiver };
    fixture.poll_until_idle();
    fixture
}

fn built() -> Fixture {
    built_with(|_| {})
}

/// Contract (task 4.2): the dialog paints its diagnostics and its recovery
/// choices, and the choices report their `daemonLost` regions in the core's
/// order.
#[test]
fn the_dialog_paints_its_diagnostics_and_its_recovery_choices() {
    let mut fixture = built_with(|props| {
        props.restart_error = Some("attach refused".to_owned());
    });
    let text = fixture.text();
    assert!(text.contains("The local daemon connection was lost unexpectedly."));
    assert!(text.contains("Was playing: Track One"));
    assert!(text.contains("Daemon log: /home/user/.local/state/mbv/local-daemon.log"));
    assert!(text.contains("Restart failed: attach refused"));
    assert!(text.contains("Restart and resume"));
    assert!(text.contains("Restart, don't resume"));
    assert!(text.contains("Quit"));
    fixture.poll_until_idle();
    assert_eq!(
        last_reported_targets(&fixture.receiver),
        vec![
            RegionTarget::DaemonLost(DaemonLostTargetId::Button(0)),
            RegionTarget::DaemonLost(DaemonLostTargetId::Button(1)),
            RegionTarget::DaemonLost(DaemonLostTargetId::Button(2)),
        ]
    );
}

/// Contract (task 4.2): a frame with nothing playing paints no title line.
#[test]
fn a_frame_with_nothing_playing_paints_no_title_line() {
    let fixture = built_with(|props| {
        props.last_playing_title = None;
    });
    assert!(!fixture.text().contains("Was playing:"));
}

/// Contract (task 4.2): the affirmative choice paints the accent text token
/// on the recessed fill — never the picked row's fill token (the
/// dark-on-green contrast bug this guards).
#[test]
fn the_affirmative_choice_paints_the_accent_text_token_on_the_recessed_fill() {
    let fixture = built();
    let style = fixture
        .style_of("[data-daemon-lost-button=\"0\"]")
        .expect("the affirmative choice paints");
    assert!(style.contains("color: var(--canvas-accent)"), "{style}");
    assert!(style.contains("background: var(--canvas-well)"), "{style}");
}
