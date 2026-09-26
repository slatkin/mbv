//! Single socket-adaptation type for daemon control and remote player transports.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::os::unix::net::UnixStream;
#[derive(Debug)]
pub enum SocketStream {
    Unix(UnixStream),
    Tcp(TcpStream),
}

impl SocketStream {
    pub fn try_clone(&self) -> io::Result<Self> {
        match self {
            Self::Unix(stream) => stream.try_clone().map(Self::Unix),
            Self::Tcp(stream) => stream.try_clone().map(Self::Tcp),
        }
    }

    /// Shuts down the underlying socket for both reads and writes (#233).
    /// Unlike dropping a clone -- which only closes *that* clone's fd
    /// duplicate -- `shutdown` acts on the shared underlying socket in the
    /// kernel, so it unblocks a concurrent blocking `read()` on any other
    /// clone of the same connection immediately.
    pub fn shutdown(&self) -> io::Result<()> {
        match self {
            Self::Unix(stream) => stream.shutdown(Shutdown::Both),
            Self::Tcp(stream) => stream.shutdown(Shutdown::Both),
        }
    }
}

impl Read for SocketStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Unix(stream) => stream.read(buf),
            Self::Tcp(stream) => stream.read(buf),
        }
    }
}

impl Write for SocketStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Unix(stream) => stream.write(buf),
            Self::Tcp(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Unix(stream) => stream.flush(),
            Self::Tcp(stream) => stream.flush(),
        }
    }
}
