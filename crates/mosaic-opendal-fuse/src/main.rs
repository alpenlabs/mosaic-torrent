//! # Mosaic OpenDAL Fuse Adapter
//!
//! ## Usage
//!
//! ```sh,ignore
//! cargo run --release --bin mosaic-opendal-fuse
//! ```

use fuse3 as _;
use fuse3_opendal as _;
use libc as _;
use opendal as _;
use thiserror as _;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use mosaic_opendal_fuse::{OpenDALFuseConfiguration, S3OpenDALFuseAdapter};

/// Initializes the tracing subscriber.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = OpenDALFuseConfiguration::default();
    let adapter = S3OpenDALFuseAdapter::new(config)?;

    let mut mount_handle = adapter.start_session().await?;
    let handle = &mut mount_handle;

    tokio::select! {
        _ = handle => {},
        _ = tokio::signal::ctrl_c() => {
            match mount_handle.unmount().await {
                Ok(_) => info!("Unmounted FUSE filesystem successfully"),
                Err(e) => error!("Failed to unmount FUSE filesystem: {}", e),
            }
        }
    }

    Ok(())
}
