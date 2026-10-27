//! Handshake (`hello/`) corpus: one fixture per negotiated `CtrlHello`
//! shape — the Local control-credential hello, the credential-free packaged
//! daemon reply, service-setup administration, Pin swap, owner actions, and
//! the audio-only daemon (design D4).

use super::*;

/// Fixed identity literals for every fixture (design note N2): the corpus
/// pins wire shapes, not this binary's build identity, so the builders never
/// call `CtrlHello::current()`.
const APP_VERSION: &str = "1.2.3";
const BUILD_ID: &str = "4f2a1c9";
const CONTROL_TOKEN: &str = "conformance-control-token";
const SWAP_TOKEN: &str = "conformance-swap-token";

/// One negotiated `CtrlHello` shape in the corpus. The exhaustive
/// shape-name match keeps every shape named, and the row dispatch below
/// gives every shape a builder and a fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HelloShape {
    /// The minimal client hello: only the capabilities every peer must
    /// advertise.
    Base,
    /// A Local client hello carrying the mbv Control credential.
    ControlCredential,
    /// The packaged daemon's reply hello: no Control credential is
    /// configured, so `control-auth` is absent (issue #523's boundary).
    CredentialFreePackaged,
    /// A client hello restricted to Service-setup administration.
    ServiceSetupAdmin,
    /// A client the Owner may replace across a Pin swap: swap token, not
    /// pinned.
    PinSwap,
    /// A Pin-swap-capable client running in a pinwin panel.
    PinnedSurface,
    /// A connection restricted to one Owner action.
    OwnerAction,
    /// A daemon reply hello for an Owner configured audio-only.
    AudioOnlyDaemon,
}

impl HelloShape {
    /// Every negotiated hello shape; `rows` gives each one a fixture.
    const ALL: [HelloShape; 8] = [
        HelloShape::Base,
        HelloShape::ControlCredential,
        HelloShape::CredentialFreePackaged,
        HelloShape::ServiceSetupAdmin,
        HelloShape::PinSwap,
        HelloShape::PinnedSurface,
        HelloShape::OwnerAction,
        HelloShape::AudioOnlyDaemon,
    ];

    /// Exhaustive shape-name match: a new hello shape fails this build until
    /// its arm, builder, and fixture row exist.
    fn name(self) -> &'static str {
        match self {
            HelloShape::Base => "base",
            HelloShape::ControlCredential => "control-credential",
            HelloShape::CredentialFreePackaged => "credential-free-packaged",
            HelloShape::ServiceSetupAdmin => "service-setup-admin",
            HelloShape::PinSwap => "pin-swap",
            HelloShape::PinnedSurface => "pinned-surface",
            HelloShape::OwnerAction => "owner-action",
            HelloShape::AudioOnlyDaemon => "audio-only-daemon",
        }
    }
}

fn hello(capabilities: Vec<String>) -> CtrlHello {
    CtrlHello {
        protocol_version: CTRL_PROTOCOL_VERSION,
        app_version: APP_VERSION.to_owned(),
        build_id: BUILD_ID.to_owned(),
        capabilities,
        control_token: None,
        swap_token: None,
    }
}

/// The three capabilities every peer must advertise.
fn required_capabilities() -> Vec<String> {
    vec![
        CTRL_CAP_QUEUE_STATE.to_owned(),
        CTRL_CAP_START_INDEX.to_owned(),
        CTRL_CAP_STATUS_ONLY.to_owned(),
    ]
}

/// The daemon reply hello: every capability `CtrlHello::current()` builds
/// for an owner plus `owner-action`, in the order the daemon assembles them.
/// `control_auth` decides whether `control-auth` stays in the list.
fn daemon_capabilities(control_auth: bool) -> Vec<String> {
    let mut capabilities = required_capabilities();
    capabilities.push(CTRL_CAP_LIFECYCLE_SHUTDOWN.to_owned());
    capabilities.push(CTRL_CAP_UNIFIED_QUEUE.to_owned());
    if control_auth {
        capabilities.push(CTRL_CAP_CONTROL_AUTH.to_owned());
    }
    capabilities.push(CTRL_CAP_ABS_QUEUE.to_owned());
    capabilities.push(CTRL_CAP_ABS_PROGRESS.to_owned());
    capabilities.push(CTRL_CAP_ABS_BOOK_QUEUE.to_owned());
    capabilities.push(CTRL_CAP_ABS_BOOK_PROGRESS.to_owned());
    capabilities.push(CTRL_CAP_OWNER_QUEUE_SOURCE_UPDATE.to_owned());
    capabilities.push(CTRL_CAP_ANSWERED_QUEUE_OPS.to_owned());
    capabilities.push(CTRL_CAP_QUEUE_SHUFFLE.to_owned());
    capabilities.push(CTRL_CAP_OWNER_ACTION.to_owned());
    capabilities
}

/// The canonical `CtrlHello` for one shape.
fn build(shape: HelloShape) -> CtrlHello {
    match shape {
        HelloShape::Base => hello(required_capabilities()),
        HelloShape::ControlCredential => {
            let mut value = hello(required_capabilities());
            value.capabilities.push(CTRL_CAP_CONTROL_AUTH.to_owned());
            value.control_token = Some(CONTROL_TOKEN.to_owned());
            value
        }
        HelloShape::CredentialFreePackaged => hello(daemon_capabilities(false)),
        HelloShape::ServiceSetupAdmin => {
            let mut value = hello(required_capabilities());
            value
                .capabilities
                .push(CTRL_CAP_SERVICE_SETUP_ADMIN.to_owned());
            value.control_token = Some(CONTROL_TOKEN.to_owned());
            value
        }
        HelloShape::PinSwap => {
            let mut value = hello(required_capabilities());
            value.capabilities.push(CTRL_CAP_CONTROL_AUTH.to_owned());
            value.capabilities.push(CTRL_CAP_PIN_SWAP.to_owned());
            value.control_token = Some(CONTROL_TOKEN.to_owned());
            value.swap_token = Some(SWAP_TOKEN.to_owned());
            value
        }
        HelloShape::PinnedSurface => {
            let mut value = hello(required_capabilities());
            value.capabilities.push(CTRL_CAP_CONTROL_AUTH.to_owned());
            value.capabilities.push(CTRL_CAP_PIN_SWAP.to_owned());
            value.capabilities.push(CTRL_CAP_PINNED_SURFACE.to_owned());
            value.control_token = Some(CONTROL_TOKEN.to_owned());
            value.swap_token = Some(SWAP_TOKEN.to_owned());
            value
        }
        HelloShape::OwnerAction => {
            let mut value = hello(required_capabilities());
            value.capabilities.push(CTRL_CAP_CONTROL_AUTH.to_owned());
            value.capabilities.push(CTRL_CAP_OWNER_ACTION.to_owned());
            value.control_token = Some(CONTROL_TOKEN.to_owned());
            value
        }
        HelloShape::AudioOnlyDaemon => {
            let mut value = hello(daemon_capabilities(false));
            value.capabilities.push(CTRL_CAP_AUDIO_ONLY.to_owned());
            value
        }
    }
}

/// The `hello/` rows of the fixture table.
pub(crate) fn rows(rows: &mut Vec<FixtureRow>) {
    for shape in HelloShape::ALL {
        let value = build(shape);
        let name = shape.name();
        rows.push(FixtureRow {
            path: format!("hello/{name}.json"),
            direction: Direction::RoundTrip,
            schema: Some(schema::Root::Hello),
            value: fixture_value(value),
        });
    }
}
