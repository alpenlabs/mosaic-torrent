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
use nix as _;
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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The path to mount the FUSE filesystem at.
    #[arg(short = 'p', long)]
    mount_path: String,

    /// The path to listen on for socket connections
    #[arg(short, long, default_value = "/tmp/mosaic_opendal_fuse.sock")]
    socket: String,

    /// Whether to use an in-memory operator instead of an actual S3 operator, for testing
    #[arg(long, hide = true)]
    in_memory: bool,

    /// FUSE mount options
    #[command(flatten)]
    mount: CliMountOptions,
}

/// CLI representation of FUSE mount options.
#[derive(Debug, Clone, Default, clap::Args)]
struct CliMountOptions {
    /// Allow other users to access the mount (maps to allow_other)
    #[arg(long, default_value_t = false)]
    allow_other: bool,

    /// Allow root to access the mount (maps to allow_root)
    #[arg(long, default_value_t = false)]
    allow_root: bool,

    /// Mount read-only (maps to read_only)
    #[arg(long, default_value_t = false)]
    read_only: bool,

    /// Allow mount on a non-empty directory (maps to nonempty)
    #[arg(long, default_value_t = false)]
    nonempty: bool,

    /// Enforce kernel default permissions (maps to default_permissions)
    #[arg(long, default_value_t = false)]
    default_permissions: bool,

    /// Filesystem name (maps to fs_name)
    #[arg(long)]
    fs_name: Option<String>,

    /// User ID to mount as (maps to uid)
    #[arg(long)]
    uid: Option<u32>,

    /// Group ID to mount as (maps to gid)
    #[arg(long)]
    gid: Option<u32>,

    /// Don't apply umask on create (maps to dont_mask)
    #[arg(long, default_value_t = false)]
    dont_mask: bool,

    /// Disable open support (maps to no_open_support)
    #[arg(long, default_value_t = false)]
    no_open_support: bool,

    /// Disable opendir support (maps to no_open_dir_support)
    #[arg(long, default_value_t = false)]
    no_open_dir_support: bool,

    /// Handle killpriv on write/chown/trunc (maps to handle_killpriv)
    #[arg(long, default_value_t = false)]
    handle_killpriv: bool,

    /// Enable write-back cache (maps to write_back)
    #[arg(long, default_value_t = false)]
    write_back: bool,

    /// Force readdir plus (maps to force_readdir_plus)
    #[arg(long, default_value_t = false)]
    force_readdir_plus: bool,

    /// Root inode mode (Linux only) (maps to rootmode)
    #[cfg(target_os = "linux")]
    #[arg(long)]
    rootmode: Option<u32>,

    /// Extra custom FUSE options (comma-separated)
    #[arg(long)]
    custom_options: Option<String>,
}

impl From<CliMountOptions> for fuse3::MountOptions {
    fn from(cli: CliMountOptions) -> Self {
        let mut m = fuse3::MountOptions::default();
        // bool toggles
        m.allow_other(cli.allow_other);
        m.allow_root(cli.allow_root);
        m.read_only(cli.read_only);
        m.nonempty(cli.nonempty);
        m.default_permissions(cli.default_permissions);
        m.dont_mask(cli.dont_mask);
        m.no_open_support(cli.no_open_support);
        m.no_open_dir_support(cli.no_open_dir_support);
        m.handle_killpriv(cli.handle_killpriv);
        m.write_back(cli.write_back);
        m.force_readdir_plus(cli.force_readdir_plus);

        // optional fields
        if let Some(name) = cli.fs_name {
            m.fs_name(name);
        }
        if let Some(uid) = cli.uid {
            m.uid(uid);
        }
        if let Some(gid) = cli.gid {
            m.gid(gid);
        }
        #[cfg(target_os = "linux")]
        if let Some(rm) = cli.rootmode {
            m.rootmode(rm);
        }
        if let Some(opts) = cli.custom_options {
            m.custom_options(opts);
        }
        m
    }
}

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
    let config = OpenDALFuseConfiguration {
        mount_options: cli.mount.into(),
        s3: S3Configuration::from_env(),
        ..Default::default()
    };

    let adapter = if cli.in_memory {
        let operator = Operator::new(Memory::default())?.finish();
        S3OpenDALFuseAdapter::new_with_operator(config, operator)
    } else {
        S3OpenDALFuseAdapter::new(config)?
    };

    let mut mount_handle = adapter.start_session(cli.mount_path).await?;
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
