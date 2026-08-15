//! Single socket-adaptation type for every mbv transport: the daemon control
//! channel, the shared-data service, and the remote player. One enum instead
//! of four parallel adapters (previously traits `CtrlStream`/`SharedStream`
//! and enums `ControlStream`/`MaybeTls`).

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub(crate) enum SocketStream {
    Unix(UnixStream),
    Tcp(TcpStream),
    Tls(native_tls::TlsStream<TcpStream>),
}

impl SocketStream {
    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        match self {
            Self::Unix(stream) => stream.try_clone().map(Self::Unix),
            Self::Tcp(stream) => stream.try_clone().map(Self::Tcp),
            // ponytail: TLS cannot clone without a second handshake; ctrl
            // paths never carry TLS so this arm is unreachable in practice.
            // Upgrade: wrap the TlsStream in Arc if a clone is ever needed.
            Self::Tls(_) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "TLS streams cannot be cloned",
            )),
        }
    }

    /// Shuts down the underlying socket for both reads and writes (#233).
    /// Unlike dropping a clone -- which only closes *that* clone's fd
    /// duplicate -- `shutdown` acts on the shared underlying socket in the
    /// kernel, so it unblocks a concurrent blocking `read()` on any other
    /// clone of the same connection immediately.
    pub(crate) fn shutdown(&self) -> io::Result<()> {
        match self {
            Self::Unix(stream) => stream.shutdown(Shutdown::Both),
            Self::Tcp(stream) => stream.shutdown(Shutdown::Both),
            Self::Tls(stream) => stream.get_ref().shutdown(Shutdown::Both),
        }
    }

    pub(crate) fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            Self::Unix(stream) => stream.set_read_timeout(timeout),
            Self::Tcp(stream) => stream.set_read_timeout(timeout),
            Self::Tls(stream) => stream.get_ref().set_read_timeout(timeout),
        }
    }

    pub(crate) fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            Self::Unix(stream) => stream.set_write_timeout(timeout),
            Self::Tcp(stream) => stream.set_write_timeout(timeout),
            Self::Tls(stream) => stream.get_ref().set_write_timeout(timeout),
        }
    }

    pub(crate) fn set_nonblocking(&self) -> io::Result<()> {
        match self {
            Self::Unix(stream) => stream.set_nonblocking(true),
            Self::Tcp(stream) => stream.set_nonblocking(true),
            Self::Tls(stream) => stream.get_ref().set_nonblocking(true),
        }
    }
}

impl Read for SocketStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Unix(stream) => stream.read(buf),
            Self::Tcp(stream) => stream.read(buf),
            Self::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for SocketStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Unix(stream) => stream.write(buf),
            Self::Tcp(stream) => stream.write(buf),
            Self::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Unix(stream) => stream.flush(),
            Self::Tcp(stream) => stream.flush(),
            Self::Tls(stream) => stream.flush(),
        }
    }
}
