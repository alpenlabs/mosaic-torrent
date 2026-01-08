//! # Mosaic OpenDAL Fuse Adapter
//!
//! ## Usage
//!
//! ```sh,ignore
//! cargo run --release mosaic-opendal-fuse --mount-path /path/to/mount
//! ```

use std::{fs, path::Path};

use clap::Parser;
use fuse3::raw::MountHandle;
use fuse3_opendal as _;
use opendal::{Operator, services::Memory};
use thiserror as _;
use tokio::{
    net::UnixListener,
    signal::unix::{SignalKind, signal},
    task::JoinHandle,
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use mosaic_opendal_fuse::{OpenDALFuseConfiguration, S3Configuration, S3OpenDALFuseAdapter};
mod cli;
use cli::Cli;

/// Initializes the tracing subscriber.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// Spawns the socket and signals tasks and returns the handles.
async fn spawn_tasks<S: Into<String>>(
    socket_path: S,
) -> Result<(JoinHandle<()>, JoinHandle<()>), Box<dyn std::error::Error>> {
    let socket = spawn_socket_listener(socket_path)?;
    let signals = spawn_signal_listener()?;
    Ok((socket, signals))
}

/// Spawns and returns the socket listener task.
fn spawn_socket_listener<S: Into<String>>(
    socket_path: S,
) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
    let socket_path = socket_path.into();
    let _ = fs::remove_file(&socket_path);

    // Setup a socket that closes connections immediately to expose readiness.
    let listener = UnixListener::bind(&socket_path)?;
    let socket = tokio::spawn(async move {
        info!("S3OpenDalFuseAdapter socket listening on {}", &socket_path);
        loop {
            let _ = listener.accept().await;
        }
    });

    Ok(socket)
}

/// Spawns and returns the signals listener task.
fn spawn_signal_listener() -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
    // Setup unix signals to listen to.
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    let signals = tokio::spawn(async move {
        tokio::select! {
            _ = sigint.recv() => {},
            _ = sigterm.recv() => {},
        }
    });

    Ok(signals)
}

/// Attempts to unmount the FUSE filesystem and clean up the socket.
async fn cleanup<P: AsRef<Path>>(mount_handle: MountHandle, socket_path: P) {
    let _ = fs::remove_file(&socket_path);

    match mount_handle.unmount().await {
        Ok(_) => info!("Unmounted FUSE filesystem successfully"),
        Err(e) => error!("Failed to unmount FUSE filesystem: {}", e),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    init_tracing();

    let cli = Cli::parse();
    let uid = cli.mount_options.uid;
    let gid = cli.mount_options.gid;
    let config = OpenDALFuseConfiguration {
        mount_options: cli.mount_options.into(),
        s3: S3Configuration::from_env(),
    };

    info!("Starting with config: {}", config);

    let adapter = if cli.in_memory {
        let operator = Operator::new(Memory::default())?.finish();
        S3OpenDALFuseAdapter::new_with_operator(config, operator)
    } else {
        S3OpenDALFuseAdapter::new(config)?
    };

    let mut mount_handle = adapter.start_session(&cli.mount_path, uid, gid).await?;
    let handle = &mut mount_handle;

    // If some sockets fail to spawn, we need to clean up the mount point.
    let (_socket, signals) = match spawn_tasks(cli.socket.clone()).await {
        Ok(v) => v,
        Err(_) => {
            cleanup(mount_handle, cli.socket).await;
            return Ok(());
        }
    };

    tokio::select! {
        _ = handle => {},
        _ = signals => cleanup(mount_handle, cli.socket).await,
    }

    Ok(())
}
