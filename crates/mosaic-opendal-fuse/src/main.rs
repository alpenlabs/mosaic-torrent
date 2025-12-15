//! # Mosaic OpenDAL Fuse Adapter
//!
//! ## Usage
//!
//! ```sh,ignore
//! cargo run --release --bin mosaic-opendal-fuse
//! ```

use std::fs;

use clap::Parser;
use fuse3 as _;
use fuse3_opendal as _;
use libc as _;
use opendal::{self as _, Operator, services::Memory};
use thiserror as _;
use tokio::{
    net::UnixListener,
    signal::unix::{SignalKind, signal},
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use mosaic_opendal_fuse::{OpenDALFuseConfiguration, S3OpenDALFuseAdapter};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The path to mount the FUSE filesystem at. If not specified, a temporary directory is used
    #[arg(short = 'p', long)]
    mount_path: Option<String>,

    /// The path to listen on for socket connections
    #[arg(short, long, default_value = "/tmp/mosaic_opendal_fuse.sock")]
    socket: String,

    /// Whether to use an in-memory operator instead of an actual S3 operator, for testing
    #[arg(long, hide = true)]
    in_memory: bool,
}

/// Initializes the tracing subscriber.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let cli = Cli::parse();
    let mut config = OpenDALFuseConfiguration::default();
    cli.mount_path.map(|path| config.mount_directory = path);

    let adapter = if cli.in_memory {
        let operator = Operator::new(Memory::default())?.finish();
        S3OpenDALFuseAdapter::new_with_operator(config, operator)
    } else {
        S3OpenDALFuseAdapter::new(config)?
    };

    let mut mount_handle = adapter.start_session().await?;
    let handle = &mut mount_handle;

    // Setup a socket that closes connections immediately to expose readiness.
    let _ = fs::remove_file(&cli.socket);

    let listener = UnixListener::bind(&cli.socket)?;
    tokio::spawn(async move {
        info!("S3OpenDalFuseAdapter socket listening on {}", &cli.socket);
        loop {
            let _ = listener.accept().await;
        }
    });

    // Setup unix signals to listen to.
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    let signals = tokio::spawn(async move {
        tokio::select! {
            _ = sigint.recv() => {},
            _ = sigterm.recv() => {},
        }
    });

    tokio::select! {
        _ = handle => {},
        _ = signals => {
            match mount_handle.unmount().await {
                Ok(_) => info!("Unmounted FUSE filesystem successfully"),
                Err(e) => error!("Failed to unmount FUSE filesystem: {}", e),
            }
        }
    }

    Ok(())
}
