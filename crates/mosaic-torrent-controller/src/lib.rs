// Allow unused dev-dependencies in lib test target
#![cfg_attr(test, allow(unused_crate_dependencies))]

//! # Torrent controller using Transmission RPC.
//!
//! This crate provides a [`TransmissionClient`] that implements the [`mosaic_torrent_types::BitTorrent`] trait
//! from `mosaic_torrent_types`, allowing you to manage torrents through the Transmission daemon.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use mosaic_torrent_controller::TransmissionClient;
//! use mosaic_torrent_types::{BitTorrent, create_torrent_file};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     create_torrent_file(
//!         "path/to/folder",
//!         "path/to/output/file.torrent",
//!         None,
//!     )?;
//!     let client = TransmissionClient::try_new("http://localhost:9091/transmission/rpc", 1).await?;
//!     let torrent = client.add("path/to/output/file.torrent").await?;
//!     println!("Added torrent: {:?}", torrent);
//!     Ok(())
//! }
//! ```

mod client;
mod conversions;
mod ops;

#[cfg(test)]
mod testutil;

pub use client::TransmissionClient;
