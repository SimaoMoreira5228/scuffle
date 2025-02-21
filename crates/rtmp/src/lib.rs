//! A crate for handling RTMP server connections.
//!
//! ## Status
//!
//! This crate is currently under development and is not yet stable.
//!
//! Unit tests are not yet fully implemented. Use at your own risk.
//!
//! ## License
//!
//! This project is licensed under the [MIT](./LICENSE.MIT) or [Apache-2.0](./LICENSE.Apache-2.0) license.
//! You can choose between one of them if you use this work.
//!
//! `SPDX-License-Identifier: MIT OR Apache-2.0`
#![deny(missing_docs)]
#![deny(unsafe_code)]

mod channels;
mod chunk;
mod handshake;
mod macros;
mod messages;
mod netconnection;
mod netstream;
mod protocol_control_messages;
mod session;
mod user_control_messages;

pub use channels::{ChannelData, DataConsumer, DataProducer, PublishConsumer, PublishProducer, PublishRequest, UniqueID};
pub use session::{Session, SessionError};

#[cfg(test)]
mod tests;
