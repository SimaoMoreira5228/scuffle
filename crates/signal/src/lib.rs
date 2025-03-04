//! A crate designed to provide a more user friendly interface to
//! `tokio::signal`.
//!
//! ## Why do we need this?
//!
//! The `tokio::signal` module provides a way for us to wait for a signal to be
//! received in a non-blocking way. This crate extends that with a more helpful
//! interface allowing the ability to listen to multiple signals concurrently.
//!
//! ## Example
//!
//! ```rust
//! # #[cfg(unix)]
//! # {
//! use scuffle_signal::SignalHandler;
//! use tokio::signal::unix::SignalKind;
//!
//! # tokio_test::block_on(async {
//! let mut handler = SignalHandler::new()
//!     .with_signal(SignalKind::interrupt())
//!     .with_signal(SignalKind::terminate());
//!
//! # // Safety: This is a test, and we control the process.
//! # unsafe {
//! #    libc::raise(SignalKind::interrupt().as_raw_value());
//! # }
//! // Wait for a signal to be received
//! let signal = handler.await;
//!
//! // Handle the signal
//! let interrupt = SignalKind::interrupt();
//! let terminate = SignalKind::terminate();
//! match signal {
//!     interrupt => {
//!         // Handle SIGINT
//!         println!("received SIGINT");
//!     },
//!     terminate => {
//!         // Handle SIGTERM
//!         println!("received SIGTERM");
//!     },
//! }
//! # });
//! # }
//! ```
//!
//! ## Status
//!
//! This crate is currently under development and is not yet stable.
//!
//! Unit tests are not yet fully implemented. Use at your own risk.
//!
//! ## License
//!
//! This project is licensed under the [MIT](./LICENSE.MIT) or
//! [Apache-2.0](./LICENSE.Apache-2.0) license. You can choose between one of
//! them if you use this work.
//!
//! `SPDX-License-Identifier: MIT OR Apache-2.0`
#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]

use std::pin::Pin;
use std::task::{Context, Poll};

#[cfg(unix)]
use tokio::signal::unix;
#[cfg(unix)]
pub use tokio::signal::unix::SignalKind as UnixSignalKind;
#[cfg(windows)]
use tokio::signal::windows;

#[cfg(feature = "bootstrap")]
mod bootstrap;

#[cfg(feature = "bootstrap")]
pub use bootstrap::{SignalConfig, SignalSvc};

/// The type of signal to listen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    /// Represents the interrupt signal, which is `SIGINT` on Unix and `Ctrl-C` on Windows.
    Interrupt,
    /// Represents the terminate signal, which is `SIGTERM` on Unix and `Ctrl-Close` on Windows.
    Terminate,
    /// Represents a Windows-specific signal kind, as defined in `WindowsSignalKind`.
    #[cfg(windows)]
    Windows(WindowsSignalKind),
    /// Represents a Unix-specific signal kind, wrapping `tokio::signal::unix::SignalKind`.
    #[cfg(unix)]
    Unix(UnixSignalKind),
}

#[cfg(unix)]
impl From<UnixSignalKind> for SignalKind {
    fn from(value: UnixSignalKind) -> Self {
        match value {
            kind if kind == UnixSignalKind::interrupt() => Self::Interrupt,
            kind if kind == UnixSignalKind::terminate() => Self::Terminate,
            kind => Self::Unix(kind),
        }
    }
}

#[cfg(unix)]
impl PartialEq<UnixSignalKind> for SignalKind {
    fn eq(&self, other: &UnixSignalKind) -> bool {
        match self {
            Self::Interrupt => other == &UnixSignalKind::interrupt(),
            Self::Terminate => other == &UnixSignalKind::terminate(),
            Self::Unix(kind) => kind == other,
        }
    }
}

/// Represents Windows-specific signal kinds.
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsSignalKind {
    /// Represents the `Ctrl-Break` signal.
    CtrlBreak,
    /// Represents the `Ctrl-C` signal.
    CtrlC,
    /// Represents the `Ctrl-Close` signal.
    CtrlClose,
    /// Represents the `Ctrl-Logoff` signal.
    CtrlLogoff,
    /// Represents the `Ctrl-Shutdown` signal.
    CtrlShutdown,
}

#[cfg(windows)]
impl From<WindowsSignalKind> for SignalKind {
    fn from(value: WindowsSignalKind) -> Self {
        match value {
            WindowsSignalKind::CtrlC => Self::Interrupt,
            WindowsSignalKind::CtrlClose => Self::Terminate,
            WindowsSignalKind::CtrlBreak => Self::Windows(value),
            WindowsSignalKind::CtrlLogoff => Self::Windows(value),
            WindowsSignalKind::CtrlShutdown => Self::Windows(value),
        }
    }
}

#[cfg(windows)]
impl PartialEq<WindowsSignalKind> for SignalKind {
    fn eq(&self, other: &WindowsSignalKind) -> bool {
        match self {
            Self::Interrupt => other == &WindowsSignalKind::CtrlC,
            Self::Terminate => other == &WindowsSignalKind::CtrlClose,
            Self::Windows(kind) => kind == other,
        }
    }
}

#[cfg(windows)]
#[derive(Debug)]
enum WindowsSignalValue {
    CtrlBreak(tokio::signal::windows::CtrlBreak),
    CtrlC(tokio::signal::windows::CtrlC),
    CtrlClose(tokio::signal::windows::CtrlClose),
    CtrlLogoff(tokio::signal::windows::CtrlLogoff),
    CtrlShutdown(tokio::signal::windows::CtrlShutdown),
}

#[cfg(windows)]
impl WindowsSignalValue {
    fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<()>> {
        match self {
            Self::CtrlBreak(signal) => signal.poll_recv(cx),
            Self::CtrlC(signal) => signal.poll_recv(cx),
            Self::CtrlClose(signal) => signal.poll_recv(cx),
            Self::CtrlLogoff(signal) => signal.poll_recv(cx),
            Self::CtrlShutdown(signal) => signal.poll_recv(cx),
        }
    }
}

#[cfg(unix)]
type Signal = unix::Signal;

#[cfg(windows)]
type Signal = WindowsSignalValue;

#[cfg(unix)]
type OsSignalKind = UnixSignalKind;
#[cfg(windows)]
type OsSignalKind = WindowsSignalKind;

impl SignalKind {
    #[cfg(unix)]
    fn listen(&self) -> Result<Signal, std::io::Error> {
        match self {
            Self::Interrupt => tokio::signal::unix::signal(UnixSignalKind::interrupt()),
            Self::Terminate => tokio::signal::unix::signal(UnixSignalKind::terminate()),
            Self::Unix(kind) => tokio::signal::unix::signal(*kind),
        }
    }

    #[cfg(windows)]
    fn listen(&self) -> Result<Signal, std::io::Error> {
        match self {
            // https://learn.microsoft.com/en-us/windows/console/ctrl-c-and-ctrl-break-signals
            Self::Interrupt | Self::Windows(WindowsSignalKind::CtrlC) => {
                Ok(WindowsSignalValue::CtrlC(tokio::signal::windows::ctrl_c()?))
            }
            // https://learn.microsoft.com/en-us/windows/console/ctrl-close-signal
            Self::Terminate | Self::Windows(WindowsSignalKind::CtrlClose) => {
                Ok(WindowsSignalValue::CtrlClose(tokio::signal::windows::ctrl_close()?))
            }
            Self::Windows(WindowsSignalKind::CtrlBreak) => {
                Ok(WindowsSignalValue::CtrlBreak(tokio::signal::windows::ctrl_break()?))
            }
            Self::Windows(WindowsSignalKind::CtrlLogoff) => {
                Ok(WindowsSignalValue::CtrlLogoff(tokio::signal::windows::ctrl_logoff()?))
            }
            Self::Windows(WindowsSignalKind::CtrlShutdown) => {
                Ok(WindowsSignalValue::CtrlShutdown(tokio::signal::windows::ctrl_shutdown()?))
            }
        }
    }

    #[cfg(unix)]
    fn into_os(&self) -> OsSignalKind {
        match self {
            Self::Interrupt => UnixSignalKind::interrupt(),
            Self::Terminate => UnixSignalKind::terminate(),
            Self::Unix(kind) => *kind,
        }
    }

    #[cfg(windows)]
    fn into_os(&self) -> OsSignalKind {
        match self {
            Self::Interrupt => WindowsSignalKind::CtrlC,
            Self::Terminate => WindowsSignalKind::CtrlClose,
            Self::Windows(kind) => *kind,
        }
    }
}

/// A handler for listening to multiple signals, and providing a future for
/// receiving them.
///
/// This is useful for applications that need to listen for multiple signals,
/// and want to react to them in a non-blocking way. Typically you would need to
/// use a tokio::select{} to listen for multiple signals, but this provides a
/// more ergonomic interface for doing so.
///
/// After a signal is received you can poll the handler again to wait for
/// another signal. Dropping the handle will cancel the signal subscription
///
/// # Example
///
/// ```rust
/// use scuffle_signal::SignalHandler;
/// use tokio::signal::unix::SignalKind;
///
/// # tokio_test::block_on(async {
/// let mut handler = SignalHandler::new()
///     .with_signal(SignalKind::interrupt())
///     .with_signal(SignalKind::terminate());
///
/// # // Safety: This is a test, and we control the process.
/// # unsafe {
/// #    libc::raise(SignalKind::interrupt().as_raw_value());
/// # }
/// // Wait for a signal to be received
/// let signal = handler.await;
///
/// // Handle the signal
/// let interrupt = SignalKind::interrupt();
/// let terminate = SignalKind::terminate();
/// match signal {
///     interrupt => {
///         // Handle SIGINT
///         println!("received SIGINT");
///     },
///     terminate => {
///         // Handle SIGTERM
///         println!("received SIGTERM");
///     },
/// }
/// # });
/// ```
#[derive(Debug)]
#[must_use = "signal handlers must be used to wait for signals"]
pub struct SignalHandler {
    signals: Vec<(SignalKind, Signal)>,
}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalHandler {
    /// Create a new `SignalHandler` with no signals.
    pub const fn new() -> Self {
        Self { signals: Vec::new() }
    }

    /// Create a new `SignalHandler` with the given signals.
    pub fn with_signals<T: Into<SignalKind>>(signals: impl IntoIterator<Item = T>) -> Self {
        let mut handler = Self::new();

        for signal in signals {
            handler = handler.with_signal(signal.into());
        }

        handler
    }

    /// Add a signal to the handler.
    ///
    /// If the signal is already in the handler, it will not be added again.
    pub fn with_signal(mut self, kind: impl Into<SignalKind>) -> Self {
        let kind = kind.into();
        if self.signals.iter().any(|(k, _)| k == &kind) {
            return self;
        }

        let signal = kind.listen().expect("failed to create signal");

        self.signals.push((kind, signal));

        self
    }

    /// Add a signal to the handler.
    ///
    /// If the signal is already in the handler, it will not be added again.
    pub fn add_signal(&mut self, kind: impl Into<SignalKind>) -> &mut Self {
        let kind = kind.into();
        if self.signals.iter().any(|(k, _)| k == &kind) {
            return self;
        }

        let signal = kind.listen().expect("failed to create signal");

        self.signals.push((kind, signal));

        self
    }

    /// Wait for a signal to be received.
    /// This is equivilant to calling (&mut handler).await, but is more
    /// ergonomic if you want to not take ownership of the handler.
    pub async fn recv(&mut self) -> SignalKind {
        self.await
    }

    /// Poll for a signal to be received.
    /// Does not require pinning the handler.
    pub fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<SignalKind> {
        for (kind, signal) in self.signals.iter_mut() {
            if signal.poll_recv(cx).is_ready() {
                return Poll::Ready(*kind);
            }
        }

        Poll::Pending
    }
}

impl std::future::Future for SignalHandler {
    type Output = SignalKind;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_recv(cx)
    }
}

#[cfg(test)]
#[cfg_attr(all(coverage_nightly, test), coverage(off))]
mod tests {
    use std::time::Duration;

    use scuffle_future_ext::FutureExt;

    use super::*;

    #[cfg(unix)]
    pub fn raise_signal(kind: SignalKind) {
        // Safety: This is a test, and we control the process.
        unsafe {
            libc::raise(kind.into_os().as_raw_value());
        }
    }

    #[cfg(windows)]
    pub fn raise_signal(kind: SignalKind) {
        // Safety: This is a test, and we control the process.
        unsafe {
            // only accepts CtrlC and CtrlBreak
            use winapi::um::wincon::GenerateConsoleCtrlEvent;

            match kind {
                SignalKind::Interrupt | SignalKind::Windows(WindowsSignalKind::CtrlC) => {
                    GenerateConsoleCtrlEvent(winapi::um::wincon::CTRL_C_EVENT, 0)
                }
                SignalKind::Windows(WindowsSignalKind::CtrlBreak) => {
                    GenerateConsoleCtrlEvent(winapi::um::wincon::CTRL_BREAK_EVENT, 0)
                }
                _ => panic!("unsupported signal"),
            };
        }
    }

    #[cfg(all(not(valgrind), unix))] // test is time-sensitive
    #[tokio::test]
    async fn signal_handler() {
        let mut handler = SignalHandler::with_signals([UnixSignalKind::user_defined1()])
            .with_signal(UnixSignalKind::user_defined2())
            .with_signal(UnixSignalKind::user_defined1());

        raise_signal(SignalKind::Unix(UnixSignalKind::user_defined1()));

        let recv = (&mut handler).with_timeout(Duration::from_millis(5)).await.unwrap();

        assert_eq!(recv, SignalKind::Unix(UnixSignalKind::user_defined1()), "expected SIGUSR1");

        // We already received the signal, so polling again should return Poll::Pending
        let recv = (&mut handler).with_timeout(Duration::from_millis(5)).await;

        assert!(recv.is_err(), "expected timeout");

        raise_signal(SignalKind::Unix(UnixSignalKind::user_defined2()));

        // We should be able to receive the signal again
        let recv = (&mut handler).with_timeout(Duration::from_millis(5)).await.unwrap();

        assert_eq!(recv, UnixSignalKind::user_defined2(), "expected SIGUSR2");
    }

    #[cfg(all(not(valgrind), windows))]
    #[tokio::test]
    async fn signal_handler() {
        let mut handler = SignalHandler::with_signals([WindowsSignalKind::CtrlC])
            .with_signal(WindowsSignalKind::CtrlBreak)
            .with_signal(WindowsSignalKind::CtrlC);

        raise_signal(SignalKind::Windows(WindowsSignalKind::CtrlC));

        let recv = (&mut handler).with_timeout(Duration::from_millis(5)).await.unwrap();

        assert_eq!(recv, WindowsSignalKind::CtrlC, "expected CtrlC");

        let recv = (&mut handler).with_timeout(Duration::from_millis(5)).await;
        assert!(recv.is_err(), "expected timeout");

        raise_signal(SignalKind::Windows(WindowsSignalKind::CtrlBreak));

        let recv = (&mut handler).with_timeout(Duration::from_millis(5)).await.unwrap();

        assert_eq!(recv, WindowsSignalKind::CtrlBreak, "expected CtrlBreak");
    }

    #[cfg(all(not(valgrind), unix))] // test is time-sensitive
    #[tokio::test]
    async fn add_signal() {
        let mut handler = SignalHandler::new();

        handler
            .add_signal(UnixSignalKind::user_defined1())
            .add_signal(UnixSignalKind::user_defined2())
            .add_signal(UnixSignalKind::user_defined2());

        raise_signal(SignalKind::Unix(UnixSignalKind::user_defined1()));

        let recv = handler.recv().with_timeout(Duration::from_millis(5)).await.unwrap();

        assert_eq!(recv, UnixSignalKind::user_defined1(), "expected SIGUSR1");

        raise_signal(SignalKind::Unix(UnixSignalKind::user_defined2()));

        let recv = handler.recv().with_timeout(Duration::from_millis(5)).await.unwrap();

        assert_eq!(recv, UnixSignalKind::user_defined2(), "expected SIGUSR2");
    }

    #[cfg(all(not(valgrind), windows))]
    #[tokio::test]
    async fn add_signal() {
        let mut handler = SignalHandler::new();

        handler
            .add_signal(WindowsSignalKind::CtrlC)
            .add_signal(WindowsSignalKind::CtrlBreak)
            .add_signal(WindowsSignalKind::CtrlC);

        raise_signal(SignalKind::Windows(WindowsSignalKind::CtrlC));

        let recv = handler.recv().with_timeout(Duration::from_millis(5)).await.unwrap();

        assert_eq!(recv, WindowsSignalKind::CtrlC, "expected CtrlC");

        raise_signal(SignalKind::Windows(WindowsSignalKind::CtrlBreak));

        let recv = handler.recv().with_timeout(Duration::from_millis(5)).await.unwrap();

        assert_eq!(recv, WindowsSignalKind::CtrlBreak, "expected CtrlBreak");
    }

    #[cfg(not(valgrind))] // test is time-sensitive
    #[tokio::test]
    async fn no_signals() {
        let mut handler = SignalHandler::default();

        // Expected to timeout
        assert!(handler.recv().with_timeout(Duration::from_millis(50)).await.is_err());
    }
}
