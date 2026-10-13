//! The IPC socket session of the child-process backend (design D3 and D7 of
//! `mpv-child-process`).
//!
//! One session owns one non-blocking `UnixStream` to the `mpv` child. The
//! playback thread drives it, as it drives the libmpv handle today. `request`
//! sends a command and waits for its reply, and `next_event` pops the queued
//! events in the order that mpv sent them. No reader thread runs.

use std::collections::VecDeque;
use std::fmt;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::mpv_error::MpvError;
use crate::mpv_event::{MpvEvent, MpvFailure};

/// Bytes read from the socket per call.
const READ_CHUNK: usize = 4096;

/// Where the socket stands. The end event waits behind the lines that arrived
/// before end-of-file, so it is queued only after those lines are delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Link {
    Open,
    /// End-of-file was read. Complete lines may still be buffered.
    PeerClosed,
    /// The end event is queued. The session reads no more.
    Ended,
}

/// The IPC socket of one `mpv` child process.
#[derive(Debug)]
pub(crate) struct IpcSession {
    stream: UnixStream,
    /// Bytes read from the socket that do not yet end in a newline.
    received: Vec<u8>,
    events: VecDeque<MpvEvent>,
    link: Link,
    /// Set when a `quit` command is sent. The end of the socket then reads as
    /// a shutdown, not as a crash (scenario S8).
    quit_requested: bool,
    next_request_id: u64,
}

impl IpcSession {
    /// Wraps a connected socket and sets it non-blocking, so no read holds the
    /// playback thread.
    pub(crate) fn from_stream(stream: UnixStream) -> Result<Self, MpvError> {
        stream.set_nonblocking(true).map_err(MpvError::Io)?;
        Ok(Self {
            stream,
            received: Vec::new(),
            events: VecDeque::new(),
            link: Link::Open,
            quit_requested: false,
            next_request_id: 1,
        })
    }

    /// Sends `command` and waits up to `timeout` for its reply, and returns the
    /// reply's `data`. Events that arrive before the reply stay queued, in
    /// order, for `next_event`.
    pub(crate) fn request(
        &mut self,
        command: &[&str],
        timeout: Duration,
    ) -> Result<Value, MpvError> {
        let id = self.write_command(command)?;
        let started = Instant::now();
        loop {
            if let Some(data) = self.pump(Some(id))? {
                return Ok(data);
            }
            if self.link != Link::Open {
                return Err(MpvError::Closed);
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(MpvError::Timeout);
            }
            self.wait_readable(remaining)?;
        }
    }

    /// Returns the next event. When none is queued, reads what the socket holds
    /// now. A read or decode fault is delivered as a failure event.
    pub(crate) fn next_event(&mut self) -> Option<MpvEvent> {
        if self.events.is_empty()
            && let Err(error) = self.pump(None)
        {
            self.events
                .push_back(MpvEvent::Failed(MpvFailure::Other(error.to_string())));
        }
        self.events.pop_front()
    }

    /// Writes one command line and returns its request id.
    fn write_command(&mut self, command: &[&str]) -> Result<u64, MpvError> {
        let id = self.next_request_id;
        self.next_request_id += 1;
        let mut line = json!({"command": command, "request_id": id}).to_string();
        line.push('\n');
        self.stream
            .write_all(line.as_bytes())
            .map_err(MpvError::Io)?;
        if command.first() == Some(&"quit") {
            self.quit_requested = true;
        }
        Ok(id)
    }

    /// Reads what the socket holds, then delivers each complete line in order.
    /// Returns the data of the `awaiting` reply when that line arrives. The
    /// lines after it stay buffered for the next call.
    fn pump(&mut self, awaiting: Option<u64>) -> Result<Option<Value>, MpvError> {
        self.read_available()?;
        while let Some(line) = self.take_line() {
            if let Some(data) = self.accept_line(&line, awaiting)? {
                return Ok(Some(data));
            }
        }
        if self.link == Link::PeerClosed {
            let end = if self.quit_requested {
                MpvEvent::Shutdown
            } else {
                MpvEvent::Failed(MpvFailure::ProcessExited)
            };
            self.events.push_back(end);
            self.link = Link::Ended;
        }
        Ok(None)
    }

    /// Reads until the socket would block or the peer is gone.
    fn read_available(&mut self) -> Result<(), MpvError> {
        if self.link != Link::Open {
            return Ok(());
        }
        let mut chunk = [0_u8; READ_CHUNK];
        loop {
            let count = match self.stream.read(&mut chunk) {
                Ok(count) => count,
                Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                // A peer that closes with input unread resets the socket. The
                // reset is the same close (S8).
                Err(error) if error.kind() == ErrorKind::ConnectionReset => 0,
                Err(error) => return Err(MpvError::Io(error)),
            };
            if count == 0 {
                self.link = Link::PeerClosed;
                return Ok(());
            }
            self.received.extend_from_slice(&chunk[..count]);
        }
    }

    /// Removes the first complete line from the buffer, without its newline.
    fn take_line(&mut self) -> Option<String> {
        let end = self.received.iter().position(|&byte| byte == b'\n')?;
        let mut line: Vec<u8> = self.received.drain(..=end).collect();
        line.pop();
        Some(String::from_utf8_lossy(&line).into_owned())
    }

    /// Handles one line. The reply to `awaiting` yields its data. Any other line
    /// is decoded as an event and queued.
    fn accept_line(
        &mut self,
        line: &str,
        awaiting: Option<u64>,
    ) -> Result<Option<Value>, MpvError> {
        let value: Value = serde_json::from_str(line).map_err(decode_error)?;
        if let Some(id) = awaiting
            && value.get("request_id").and_then(Value::as_u64) == Some(id)
        {
            return reply_data(value).map(Some);
        }
        let event = MpvEvent::from_ipc_line(line).map_err(decode_error)?;
        self.events.push_back(event);
        Ok(None)
    }

    /// Blocks until the socket is readable or `timeout` passes. An interrupted
    /// wait returns early, and the caller checks the clock again.
    fn wait_readable(&self, timeout: Duration) -> Result<(), MpvError> {
        let mut pfd = libc::pollfd {
            fd: self.stream.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let millis = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
        // SAFETY: `pfd` is a valid, writable pollfd for the duration of this
        // call, and its fd is the open socket that `self.stream` owns.
        let rc = unsafe { libc::poll(&raw mut pfd, 1, millis) };
        if rc < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != ErrorKind::Interrupted {
                return Err(MpvError::Io(error));
            }
        }
        Ok(())
    }
}

/// The data of a reply, or the error that mpv gave for the command.
fn reply_data(mut reply: Value) -> Result<Value, MpvError> {
    let error = reply
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("no error field");
    if error != "success" {
        return Err(MpvError::Reply(error.to_owned()));
    }
    Ok(reply.get_mut("data").map_or(Value::Null, Value::take))
}

fn decode_error(error: impl fmt::Display) -> MpvError {
    MpvError::Decode(error.to_string())
}

#[cfg(test)]
mod tests;
