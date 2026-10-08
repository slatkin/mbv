//! Shared HTTP, TLS-agent, socket, and retry primitives.
//!
//! The native-TLS agent builder, per-Service request logging, path encoding, hard
//! bounds, and the reconnect backoff shared by the websocket loops live here. It
//! carries traffic for every Service and owns no Service logic itself.

use rand::RngExt;
use std::fmt;

pub mod bounded;
#[cfg(any(test, feature = "test"))]
pub mod mock_http;
pub mod stream;
#[cfg(test)]
mod tests;

/// Characters outside RFC 3986's unreserved set (`ALPHA / DIGIT / "-" / "." /
/// "_" / "~"`), which must be percent-encoded before going into a URL path
/// segment. ureq 3.x builds requests via `http::Uri` and rejects invalid
/// characters outright rather than encoding them (unlike ureq 2.x's more
/// lenient URL builder), so any server-returned ID or user-entered string
/// interpolated into a path must be encoded explicitly.
const PATH_SEGMENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Percent-encode a single URL path segment (an ID, name, or search term),
/// not a full path -- do not pass a string containing `/`.
#[must_use]
pub fn encode_path_segment(value: &str) -> percent_encoding::PercentEncode<'_> {
    percent_encoding::utf8_percent_encode(value, PATH_SEGMENT)
}

/// Which websocket reconnect loop is waiting for its next attempt.
#[derive(Clone, Copy, Debug)]
pub enum ReconnectTarget {
    Ws,
    AudiobookshelfSocket,
    Other(&'static str),
}

/// Sleep out one reconnect attempt: exponential backoff with 0–1s jitter,
/// doubling `backoff_secs` up to 60s. Shared by the Emby and Audiobookshelf
/// websocket reconnect loops so the incantation lives in one place.
pub fn reconnect_backoff_sleep(backoff_secs: &mut u64, target: ReconnectTarget) {
    let jitter: f64 = rand::rng().random_range(0.0..1.0);
    let delay = std::time::Duration::from_secs_f64(
        f64::from(u32::try_from(*backoff_secs).unwrap_or(u32::MAX)) + jitter,
    );
    match target {
        ReconnectTarget::Ws => tracing::info!(
            name: "net.reconnect.scheduled",
            target: "ws",
            delay_secs = delay.as_secs_f64(),
            backoff_secs = *backoff_secs,
            "reconnecting"
        ),
        ReconnectTarget::AudiobookshelfSocket => tracing::info!(
            name: "net.reconnect.scheduled",
            target: "audiobookshelf_socket",
            delay_secs = delay.as_secs_f64(),
            backoff_secs = *backoff_secs,
            "reconnecting"
        ),
        ReconnectTarget::Other(target) => tracing::info!(
            name: "net.reconnect.scheduled",
            target: "net",
            source_target = target,
            delay_secs = delay.as_secs_f64(),
            backoff_secs = *backoff_secs,
            "reconnecting"
        ),
    }
    std::thread::sleep(delay);
    *backoff_secs = (*backoff_secs * 2).min(60);
}

/// Which Service's traffic an agent carries. Naming the service at agent
/// build time is what puts `service=` on every HTTP log line.
#[derive(Clone, Copy, Debug)]
pub enum HttpService {
    Emby,
    Audiobookshelf,
    Feeds,
}

impl HttpService {
    /// The lowercase service name as it appears in log fields
    /// (`service=emby`, `service=audiobookshelf`, `service=feeds`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Emby => "emby",
            Self::Audiobookshelf => "audiobookshelf",
            Self::Feeds => "feeds",
        }
    }
}

impl fmt::Display for HttpService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The per-request logging middleware every agent runs: the round-trip
/// happens inside an `http.request` span (`service`, `http.request.method`
/// and `url.path` -- path only, never the query string), and the outcome is
/// logged with status and duration.
struct HttpRequestLogging(HttpService);

impl ureq::middleware::Middleware for HttpRequestLogging {
    fn handle(
        &self,
        request: ureq::http::Request<ureq::SendBody>,
        next: ureq::middleware::MiddlewareNext<'_>,
    ) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
        let service = self.0;
        let method = request.method().as_str();
        let path = request.uri().path().to_owned();
        let span = tracing::info_span!(
            target: "http",
            "http.request",
            service = %service.as_str(),
            http.request.method = method,
            url.path = %path,
            http.response.status_code = tracing::field::Empty,
            duration_ms = tracing::field::Empty,
        );
        let _entered = span.enter();
        let start = std::time::Instant::now();
        let result = next.handle(request);
        let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        // `when there is a response`: a status-code error still carries
        // one, a transport failure does not.
        let status_code: Option<u64> = match &result {
            Ok(response) => Some(u64::from(response.status().as_u16())),
            Err(ureq::Error::StatusCode(status)) => Some(u64::from(*status)),
            Err(_) => None,
        };
        if let Some(status) = status_code {
            span.record("http.response.status_code", status);
        }
        span.record("duration_ms", duration_ms);
        match &result {
            Ok(_) => tracing::debug!(
                name: "http.request.done", target: "http",
                "request done"
            ),
            Err(error) => tracing::warn!(
                name: "http.request.failed", target: "http",
                error = %error,
                "request failed"
            ),
        }
        result
    }
}

/// Agent configuration shared by every provider client: the native-tls
/// provider (ureq 3 no longer auto-enables it from the feature flag), the
/// timeouts, and the HTTP logging middleware for `service`. Shared by
/// `native_tls_agent` and `MockHttp::agent_for`, so tests go through the
/// production configuration.
#[must_use]
pub fn agent_config(
    service: HttpService,
    connect_timeout: Option<std::time::Duration>,
    global_timeout: Option<std::time::Duration>,
) -> ureq::config::Config {
    ureq::Agent::config_builder()
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .build(),
        )
        .timeout_connect(connect_timeout)
        .timeout_global(global_timeout)
        .middleware(HttpRequestLogging(service))
        .build()
}

/// Build an agent using the explicit native-tls provider, carrying `service`
/// so every request it makes is logged under it. Shared by the Emby,
/// Audiobookshelf and Feeds clients and the TUI image fetchers.
#[must_use]
pub fn native_tls_agent(
    service: HttpService,
    connect_timeout: Option<std::time::Duration>,
    global_timeout: Option<std::time::Duration>,
) -> ureq::Agent {
    agent_config(service, connect_timeout, global_timeout).into()
}
