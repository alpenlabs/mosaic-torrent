//! # Mosaic OpenDAL Fuse Adapter
//!
//! ## Usage
//!
//! ```sh,ignore
//! cargo run --release --bin mosaic-opendal-fuse
//! ```

use fuse3::MountOptions;
use fuse3_opendal as _;
use opendal as _;
use thiserror as _;

use mosaic_opendal_fuse::{OpenDALFuseConfiguration, S3OpenDALFuseAdapter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = OpenDALFuseConfiguration::default();
    let adapter = S3OpenDALFuseAdapter::new(config)?;

    let handle = adapter.start_session(MountOptions::default()).await?;
    tokio::select! {
        _ = handle => {},
        _ = tokio::signal::ctrl_c() => {}
    }

    Ok(())
}
