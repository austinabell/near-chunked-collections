//! This crate aims to provide chunked storage collections for NEAR smart contracts.
//!
//! The benefit of this is more efficient gas usage in cases where the cost savings from performing
//! fewer overall reads is greater than the potentially increased number of bytes written per
//! element.

#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![deny(dead_code, unused_mut)]
#![warn(missing_docs)]

pub mod vec;
pub use vec::ChunkedVector;
