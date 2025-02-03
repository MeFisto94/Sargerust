//! A library for reading MPQ archives

#![allow(clippy::unreadable_literal)]

mod archive;
mod chain;
mod compression;
mod crypt;

pub use crate::archive::{Archive, File};
pub use crate::chain::Chain;
