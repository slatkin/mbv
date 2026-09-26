//! Test-only in-memory HTTP transport for ureq.
//!
//! Replaces loopback `TcpListener` stubs: requests are recorded verbatim (the
//! same wire bytes ureq would put on a socket) and responses are served from a
//! script, so client code (headers, paths, bodies, failure classification) is
//! exercised without a real server, per the AGENTS.md mocks-only test policy.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ureq::config::Config;
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, LazyBuffers, NextTimeout, Transport,
};
use ureq::{Agent, Error};

#[derive(Debug)]
enum Scripted {
    /// Raw HTTP response bytes.
    Respond(String),
    /// Respond after a delay, long past the caller's bound.
    Delayed(Duration, String),
    /// Fail the connection with the given io error kind.
    Fail(io::ErrorKind),
    /// Never respond within `hard_bound`; let the caller's bound time out.
    Stall(Duration),
}

#[derive(Debug)]
struct Shared {
    script: Mutex<VecDeque<Scripted>>,
    requests: Mutex<Vec<String>>,
}

/// Handle to the scripted responses and recorded requests of one mock agent.
#[derive(Clone, Debug)]
pub struct MockHttp {
    shared: Arc<Shared>,
}

impl Default for MockHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl MockHttp {
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                script: Mutex::new(VecDeque::new()),
                requests: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Queue a response with the given status and JSON/text body.
    pub fn respond(&self, status: u16, body: &str) {
        self.push(Scripted::Respond(Self::response(status, body)));
    }

    fn response(status: u16, body: &str) -> String {
        let reason = if status == 200 { "OK" } else { "Error" };
        format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    /// Queue a response that arrives only after `duration`, long after the
    /// caller's hard bound has fired (the abandoned worker still receives it).
    pub fn delayed(&self, duration: Duration, body: &str) {
        self.push(Scripted::Delayed(duration, Self::response(200, body)));
    }

    /// Queue a connection-level failure (refused / dropped / stalled socket).
    pub fn fail(&self, kind: io::ErrorKind) {
        self.push(Scripted::Fail(kind));
    }

    /// Queue a response that never arrives in time: the transport stalls past
    /// the caller's hard bound, which synthesizes the timed-out failure.
    pub fn stall(&self, duration: Duration) {
        self.push(Scripted::Stall(duration));
    }

    /// The full wire text of every request transmitted so far, in order.
    ///
    /// # Panics
    ///
    /// Panics if the `requests` mutex is poisoned: a previous holder panicked
    /// while holding the lock.
    #[must_use]
    pub fn requests(&self) -> Vec<String> {
        self.shared.requests.lock().unwrap().clone()
    }

    /// Number of requests transmitted so far, without cloning them.
    ///
    /// # Panics
    ///
    /// Panics if the `requests` mutex is poisoned: a previous holder panicked
    /// while holding the lock.
    #[must_use]
    pub fn request_count(&self) -> usize {
        self.shared.requests.lock().unwrap().len()
    }

    fn push(&self, scripted: Scripted) {
        self.shared.script.lock().unwrap().push_back(scripted);
    }

    /// Build a ureq agent whose HTTP round-trips run entirely in memory.
    #[must_use]
    pub fn agent(&self) -> Agent {
        let connector = MockConnector {
            shared: Arc::clone(&self.shared),
        };
        Agent::with_parts(Config::default(), connector, DefaultResolver::default())
    }
}

impl Shared {
    fn next_response(&self) -> Scripted {
        self.script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Scripted::Stall(Duration::from_secs(3600)))
    }
}

#[derive(Debug)]
struct MockConnector {
    shared: Arc<Shared>,
}

impl<In: Transport> Connector<In> for MockConnector {
    type Out = MockTransport;

    fn connect(
        &self,
        _details: &ConnectionDetails,
        _chained: Option<In>,
    ) -> Result<Option<Self::Out>, Error> {
        Ok(Some(MockTransport {
            buffers: LazyBuffers::new(64 * 1024, 64 * 1024),
            shared: Arc::clone(&self.shared),
            received: String::new(),
            request_logged: false,
            response_served: false,
        }))
    }
}

#[derive(Debug)]
struct MockTransport {
    buffers: LazyBuffers,
    shared: Arc<Shared>,
    /// Accumulated wire segments of the outgoing request.
    received: String,
    request_logged: bool,
    response_served: bool,
}

/// True once `wire` holds the full request: headers plus `Content-Length` body.
fn request_complete(wire: &str) -> bool {
    let Some(end) = wire.find("\r\n\r\n") else {
        return false;
    };
    let content_length = wire[..end]
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .and_then(|value| value.parse::<usize>().ok())
        })
        .unwrap_or(0);
    wire.len() >= end + 4 + content_length
}

impl Transport for MockTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, _timeout: NextTimeout) -> Result<(), Error> {
        self.received
            .push_str(&String::from_utf8_lossy(&self.buffers.output()[..amount]));
        if !self.request_logged && request_complete(&self.received) {
            self.request_logged = true;
            self.shared
                .requests
                .lock()
                .unwrap()
                .push(self.received.clone());
        }
        Ok(())
    }

    fn await_input(&mut self, _timeout: NextTimeout) -> Result<bool, Error> {
        if self.response_served {
            // Connection: close — there is nothing after the response body.
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "mock: closed",
            )));
        }
        match self.shared.next_response() {
            Scripted::Respond(raw) => {
                self.response_served = true;
                self.serve(&raw);
                Ok(true)
            }
            Scripted::Delayed(duration, raw) => {
                self.response_served = true;
                std::thread::sleep(duration);
                self.serve(&raw);
                Ok(true)
            }
            Scripted::Fail(kind) => {
                self.response_served = true;
                Err(Error::Io(io::Error::new(kind, "mock: connection failed")))
            }
            Scripted::Stall(duration) => {
                self.response_served = true;
                std::thread::sleep(duration);
                Err(Error::Io(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "mock: no response",
                )))
            }
        }
    }

    fn is_open(&mut self) -> bool {
        !self.response_served
    }
}

impl MockTransport {
    fn serve(&mut self, raw: &str) {
        let bytes = raw.as_bytes();
        let input = self.buffers.input_append_buf();
        input[..bytes.len()].copy_from_slice(bytes);
        self.buffers.input_appended(bytes.len());
    }
}
