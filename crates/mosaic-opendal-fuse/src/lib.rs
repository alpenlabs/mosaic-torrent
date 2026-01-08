//! # Mosaic OpenDAL Fuse Adapter
//!
//! ## Example
//!
//! ```rust,ignore
//! use fuse3::MountOptions;
//!
//! use mosaic_opendal_fuse::{OpenDALFuseConfiguration, S3OpenDALFuseAdapter};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!    let config = OpenDALFuseConfiguration::default();
//!    let adapter = S3OpenDALFuseAdapter::new(config)?;
//!    let handle = adapter.start_session(MountOptions::default()).await;
//!    handle.unmount().await?;
//!    Ok(())
//! }
//! ```

use std::{env, fmt, fs};

use clap as _;
use dotenvy as _;
use fuse3::{MountOptions, path::Session, raw::MountHandle};
use fuse3_opendal::Filesystem;
use nix as _;
use opendal::{Operator, services::S3};
use thiserror::Error;
use tokio as _;
use tracing::{error, info, instrument};
use tracing_subscriber as _;

/// Error variants for [`S3OpenDALFuseAdapter`].
#[derive(Error, Debug)]
pub enum Error {
    /// Represents an error when creating the OpenDAL operator.
    #[error("failed to create OpenDAL operator: {0}")]
    OpenDALOperatorInit(String),

    /// Represents an error when mounting the fuse3 file system.
    #[error("failed to mount fuse3 session: {0}")]
    Mount(String),

    /// Represents a generic I/O error.
    #[error("io: {0}")]
    Io(String),
}

/// Configuration for the S3 service.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct S3Configuration {
    /// The root directory for S3.
    pub root: String,
    /// The name of the bucket to use.
    pub bucket: String,
    /// The name of the region. Set to `auto` to use the default region, if supported by your provider.
    pub region: String,
    /// The endpoint to use.
    pub endpoint: String,
    /// The access key.
    pub access_key: String,
    /// The secret key.
    pub secret_key: String,
}

impl S3Configuration {
    /// Tries to read the configuration from the environment.
    pub fn from_env() -> Self {
        Self {
            root: env::var("OPENDAL_S3_ROOT").unwrap_or_default(),
            bucket: env::var("OPENDAL_S3_BUCKET").unwrap_or_default(),
            region: env::var("OPENDAL_S3_REGION").unwrap_or_default(),
            endpoint: env::var("OPENDAL_S3_ENDPOINT").unwrap_or_default(),
            access_key: env::var("OPENDAL_S3_ACCESS_KEY_ID").unwrap_or_default(),
            secret_key: env::var("OPENDAL_S3_SECRET_ACCESS_KEY").unwrap_or_default(),
        }
    }
}

impl fmt::Debug for S3Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let access_key_set = !self.access_key.is_empty();
        let secret_key_set = !self.secret_key.is_empty();

        // Never print credentials.
        write!(
            f,
            "S3(root=\"{}\", bucket=\"{}\", region=\"{}\", endpoint=\"{}\", access_key=<{}>, secret_key=<{}>)",
            self.root,
            self.bucket,
            self.region,
            self.endpoint,
            if access_key_set {
                "set"
            } else {
                "unset ⚠️"
            },
            if secret_key_set {
                "set"
            } else {
                "unset ⚠️"
            },
        )
    }
}

/// Configuration for the [`S3OpenDALFuseAdapter`].
#[derive(Default, Clone, PartialEq, Eq)]
pub struct OpenDALFuseConfiguration {
    /// The options for mounting the fuse3 file system.
    pub mount_options: MountOptions,
    /// The config for the S3 service.
    pub s3: S3Configuration,
}

impl fmt::Debug for OpenDALFuseConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OpenDALFuse(mount_options={:?}, s3={:?})",
            self.mount_options, self.s3
        )
    }
}

/// A fuse3 file system adapter for the OpenDAL operator.
pub struct S3OpenDALFuseAdapter {
    /// The configuration used to create the fuse3 file system.
    pub config: OpenDALFuseConfiguration,
    operator: Operator,
}

impl fmt::Debug for S3OpenDALFuseAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3OpenDALFuseAdapter")
            .field("config", &self.config)
            .field("filesystem", &"...")
            .finish()
    }
}

impl S3OpenDALFuseAdapter {
    /// Returns a new [`S3OpenDALFuseAdapter`] with the specified [`OpenDALFuseConfiguration`]. Configuration
    /// for the OpenDAL operator is read from the environment.
    pub fn new(config: OpenDALFuseConfiguration) -> Result<Self, Error> {
        info!("Creating OpenDAL operator...");
        let builder = S3::default()
            .root(&config.s3.root)
            .bucket(&config.s3.bucket)
            .region(&config.s3.region)
            .endpoint(&config.s3.endpoint)
            .access_key_id(&config.s3.access_key)
            .secret_access_key(&config.s3.secret_key);

        let operator = Operator::new(builder)
            .map_err(|e| {
                error!("Failed to create OpenDAL operator: {}", e);
                Error::OpenDALOperatorInit(e.to_string())
            })?
            .finish();
        info!("OpenDAL operator created successfully");
        Ok(Self::new_with_operator(config, operator))
    }

    /// Returns a new [`S3OpenDALFuseAdapter`] with the specified [`OpenDALFuseConfiguration`] and
    /// a custom [`Operator`]. Not meant to be called directly outside of testing, prefer
    /// [`S3OpenDALFuseAdapter::new`] instead.
    #[doc(hidden)]
    pub fn new_with_operator(config: OpenDALFuseConfiguration, operator: Operator) -> Self {
        Self { config, operator }
    }

    /// Starts a new fuse3 sessions, mounts it, and returns a handle to the mount.
    ///
    /// ## Safety
    ///
    /// The caller **must** remember to call [`MountHandle::unmount`] when the mount is no longer
    /// needed to shutdown the session cleanly and safely.
    #[instrument(skip(self), fields(mount_dir = %mount_directory))]
    pub async fn start_session<S: Into<String> + fmt::Display + fmt::Debug>(
        self,
        mount_directory: S,
        uid: u32,
        gid: u32,
    ) -> Result<MountHandle, Error> {
        let mount_directory = mount_directory.into();
        info!("Creating mount directory at {}", mount_directory);
        fs::create_dir_all(&mount_directory).map_err(|e| {
            error!("Failed to create mount directory: {}", e);
            Error::Io(e.to_string())
        })?;

        let filesystem = Filesystem::new(self.operator, uid, gid);

        info!("Mounting FUSE filesystem...");
        let handle = Session::new(self.config.mount_options)
            .mount_with_unprivileged(filesystem, &mount_directory)
            .await
            .map_err(|e| {
                error!("Failed to mount FUSE filesystem: {}", e);
                Error::Mount(e.to_string())
            })?;
        info!("FUSE filesystem mounted successfully");

        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use opendal::services::Memory;

    use super::*;

    /// A short delay so that we don't immediately unmount the fuse3 file system.
    const UNMOUNT_DELAY: Duration = Duration::from_millis(100);
    /// The directory to mount to when running tests.
    const TEST_MOUNT_DIR: &str = "/tmp/mosaic-opendal-fuse";

    #[tokio::test]
    async fn adapter_can_start() {
        let config = OpenDALFuseConfiguration::default();
        let operator = Operator::new(Memory::default()).unwrap().finish();
        let adapter = S3OpenDALFuseAdapter::new_with_operator(config, operator);
        let handle = adapter.start_session(TEST_MOUNT_DIR, 0, 0).await.unwrap();

        tokio::time::sleep(UNMOUNT_DELAY).await;
        handle.unmount().await.unwrap();
    }
}
