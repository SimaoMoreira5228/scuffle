//! A crate for encoding and decoding H.264 video headers.
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
#![deny(unsafe_code)]

mod config;
mod sps;

pub use self::config::{AVCDecoderConfigurationRecord, AvccExtendedConfig};
pub use self::sps::{ColorConfig, Sps, SpsExtended};

#[cfg(test)]
mod tests;
