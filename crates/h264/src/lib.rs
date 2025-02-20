// TODO: #![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::missing_const_for_fn)]

mod config;
mod sps;

pub use self::config::{AVCDecoderConfigurationRecord, AvccExtendedConfig};
pub use self::sps::{ColorConfig, Sps, SpsExtended};

#[cfg(test)]
mod tests;
