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

use fuse3::{MountOptions, path::Session, raw::MountHandle};
use fuse3_opendal::Filesystem;
use opendal::{Operator, services::S3};
use thiserror::Error;
use tokio as _;

/// Error variants for [`S3OpenDALFuseAdapter`].
#[derive(Error, Debug)]
pub enum Error {
    /// Represents an error when creating the OpenDAL operator.
    #[error("failed to create OpenDAL operator: {0}")]
    OpenDALOperatorInitError(String),

    /// Represents an error when mounting the fuse3 file system.
    #[error("failed to mount fuse3 session: {0}")]
    MountError(String),

    /// Represents a generic I/O error.
    #[error("io error: {0}")]
    IoError(String),
}

/// Configuration for the [`S3OpenDALFuseAdapter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDALFuseConfiguration {
    /// The local directory where to mount the fuse3 file system. If not set explicitly,
    /// a temporary directory is used instead.
    pub mount_directory: String,
    /// The user identifier.
    pub uid: u32,
    /// The group identifier.
    pub gid: u32,
}

impl Default for OpenDALFuseConfiguration {
    fn default() -> Self {
        let mount_directory = env::temp_dir().join("S3OpenDALFuseAdapter");
        Self {
            mount_directory: mount_directory.to_string_lossy().to_string(),
            uid: Default::default(),
            gid: Default::default(),
        }
    }
}

/// A fuse3 file system adapter for the OpenDAL operator.
pub struct S3OpenDALFuseAdapter {
    /// The configuration used to create the fuse3 file system.
    pub config: OpenDALFuseConfiguration,
    filesystem: Filesystem,
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
        let builder = S3::default();
        let operator = Operator::new(builder)
            .map_err(|e| Error::OpenDALOperatorInitError(e.to_string()))?
            .finish();
        Ok(Self::new_with_operator(config, operator))
    }

    /// Returns a new [`S3OpenDALFuseAdapter`] with the specified [`OpenDALFuseConfiguration`] and
    /// a custom [`Operator`] for use in testing.
    #[doc(hidden)]
    pub fn new_with_operator(config: OpenDALFuseConfiguration, operator: Operator) -> Self {
        let filesystem = Filesystem::new(operator, config.uid, config.gid);
        Self { config, filesystem }
    }

    /// Starts a new fuse3 sessions, mounts it, and returns a handle to the mount.
    ///
    /// ## Safety
    ///
    /// The caller **must** remember to call [`MountHandle::unmount`] when the mount is no longer
    /// needed to shutdown the session cleanly and safely.
    pub async fn start_session(self, mount_options: MountOptions) -> Result<MountHandle, Error> {
        fs::create_dir_all(&self.config.mount_directory)
            .map_err(|e| Error::IoError(e.to_string()))?;

        Session::new(mount_options)
            .mount_with_unprivileged(self.filesystem, &self.config.mount_directory)
            .await
            .map_err(|e| Error::MountError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use opendal::services::Memory;

    use super::*;

    /// A short delay so that we don't immediately unmount the fuse3 file system.
    const UNMOUNT_DELAY: Duration = Duration::from_millis(100);

    #[tokio::test]
    async fn adapter_can_start() {
        let config = OpenDALFuseConfiguration::default();
        let operator = Operator::new(Memory::default()).unwrap().finish();
        let adapter = S3OpenDALFuseAdapter::new_with_operator(config, operator);
        let handle = adapter
            .start_session(MountOptions::default())
            .await
            .unwrap();

        tokio::time::sleep(UNMOUNT_DELAY).await;
        handle.unmount().await.unwrap();
    }
}
