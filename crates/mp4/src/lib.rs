// TODO: #![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::missing_const_for_fn)]

mod boxes;

pub mod codec;

pub use boxes::{header, types, BoxType, DynBox};

#[cfg(test)]
mod tests;
