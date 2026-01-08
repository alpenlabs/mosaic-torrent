use clap::{Args, Parser};
use nix::unistd::{Gid, Uid};

/// Top-level CLI struct for the binary.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct Cli {
    /// The path to mount the FUSE filesystem at.
    #[arg(short = 'p', long)]
    pub mount_path: String,

    /// FUSE mount options
    #[command(flatten)]
    pub mount_options: CliMountOptions,

    /// The path to listen on for socket connections
    #[arg(short, long, default_value = "/tmp/mosaic_opendal_fuse.sock")]
    pub socket: String,

    /// Whether to use an in-memory operator instead of an actual S3 operator, for testing
    #[arg(long, hide = true)]
    pub in_memory: bool,
}

/// CLI representation of FUSE mount options.
#[derive(Debug, Clone, Default, Args)]
pub(crate) struct CliMountOptions {
    /// Allow other users to access the mount.
    #[arg(long, default_value_t = false)]
    pub allow_other: bool,

    /// Allow root to access the mount.
    #[arg(long, default_value_t = false)]
    pub allow_root: bool,

    /// Mount read-only.
    #[arg(long, default_value_t = false)]
    pub read_only: bool,

    /// Allow mount on a non-empty directory.
    #[arg(long, default_value_t = false)]
    pub nonempty: bool,

    /// Enforce kernel default permissions.
    #[arg(long, default_value_t = false)]
    pub default_permissions: bool,

    /// Filesystem name (maps to fs_name)
    #[arg(long)]
    pub fs_name: Option<String>,

    /// User ID to mount as. Defaults to current user.
    #[arg(long, default_value_t = default_uid())]
    pub uid: u32,

    /// Group ID to mount as. Defaults to current user's primary group ID.
    #[arg(long, default_value_t = default_gid())]
    pub gid: u32,

    /// Don't apply umask on create
    #[arg(long, default_value_t = false)]
    pub dont_mask: bool,

    /// Disable open support.
    #[arg(long, default_value_t = false)]
    pub no_open_support: bool,

    /// Disable opendir support.
    #[arg(long, default_value_t = false)]
    pub no_open_dir_support: bool,

    /// Handle killpriv on write/chown/trunc.
    #[arg(long, default_value_t = false)]
    pub handle_killpriv: bool,

    /// Enable write-back cache.
    #[arg(long, default_value_t = false)]
    pub write_back: bool,

    /// Force readdir plus.
    #[arg(long, default_value_t = false)]
    pub force_readdir_plus: bool,

    /// Root inode mode (Linux only).
    #[cfg(target_os = "linux")]
    #[arg(long)]
    pub rootmode: Option<u32>,

    /// Extra custom FUSE options.
    #[arg(long)]
    pub custom_options: Option<String>,
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
        m.uid(cli.uid);
        m.gid(cli.gid);
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

fn default_uid() -> u32 {
    Uid::current().as_raw()
}

fn default_gid() -> u32 {
    Gid::current().as_raw()
}
