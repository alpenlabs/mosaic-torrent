//! # Mosaic Torrent Types
//!
//! This crate defines common types and traits for BitTorrent clients used in the Mosaic project.

use metainfo::{MetainfoBuilder, PieceLength};
use thiserror::Error;

/// Error type for BitTorrent operations.
#[derive(Error, Debug)]
pub enum BitTorrentError {
    /// Network-related errors (connection failures, timeouts, etc.)
    #[error("network error: {0}")]
    Network(String),

    /// Authentication errors
    #[error("authentication required")]
    Unauthorized,

    /// Server returned an error response
    #[error("server error: {0}")]
    ServerError(String),

    /// Invalid torrent file or data
    #[error("invalid torrent: {0}")]
    InvalidTorrent(String),

    /// File system errors (file not found, permission denied, etc.)
    #[error("file system error: {0}")]
    FileSystem(String),

    /// Other unexpected errors
    #[error("unexpected error: {0}")]
    Other(String),
}

/// Create a torrent file from a folder.
/// This is not BitTorrent client specific, so it is not part of the BitTorrent trait.
pub fn create_torrent_file(
    folder: &str,
    output_file: &str,
    tracker_url: Option<&str>,
) -> Result<(), BitTorrentError> {
    let builder = MetainfoBuilder::new()
        .set_piece_length(PieceLength::OptBalanced)
        .set_main_tracker(tracker_url);

    let bytes = builder
        .build(1, folder, |_| {})
        .map_err(|e| BitTorrentError::InvalidTorrent(e.to_string()))?;

    std::fs::write(output_file, bytes).map_err(|e| BitTorrentError::FileSystem(e.to_string()))?;

    Ok(())
}

/// BitTorrent trait defines the common interface for BitTorrent clients.
#[allow(async_fn_in_trait)]
pub trait BitTorrent {
    /// Add a torrent file to Transmission. The torrents starts downloading/seeding immediately.
    /// This can be used to download a torrent, and also to seed a torrent.
    async fn add(&self, torrent_file: &str) -> Result<Torrent, BitTorrentError>;
    /// Stop torrents by their IDs. The IDs should be the torrent hash.
    async fn stop(&self, ids: Vec<String>) -> Result<(), BitTorrentError>;
    /// List all torrents.
    async fn list(&self) -> Result<Vec<Torrent>, BitTorrentError>;
    /// Get the list of peers for a specific torrent by its ID (i32).
    async fn peers(&self, id: i32) -> Result<Peers, BitTorrentError>;
    /// Remove torrents by their IDs (torrent hash). If `delete_local_data` is true, the local data will also be deleted.
    async fn remove(
        &self,
        ids: Vec<String>,
        delete_local_data: bool,
    ) -> Result<(), BitTorrentError>;
    /// Get session statistics.
    async fn stats(&self) -> Result<SessionStats, BitTorrentError>;
}

// The below are mostly copied from Transmission RPC types, as this will be the initial implementation.
// Other implementations are expected to have similar fields.

/// Session statistics.
#[derive(Debug)]
#[allow(missing_docs)] // rationale: these are the same fields as in Transmission RPC
pub struct SessionStats {
    pub active_torrent_count: i32,

    pub cumulative_stats: StatsDetails,

    pub current_stats: StatsDetails,

    pub download_speed: i32,

    pub paused_torrent_count: i32,

    pub torrent_count: i32,

    pub upload_speed: i32,
}

/// Detailed statistics.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct StatsDetails {
    pub downloaded_bytes: i64,

    pub files_added: i64,

    pub seconds_active: i64,

    pub session_count: i64,

    pub uploaded_bytes: i64,
}

/// Torrent information.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct Torrent {
    pub id: i32,

    pub activity_date: i32,

    pub added_date: i32,

    pub bandwidth_priority: i32,

    pub comment: String,

    pub creator: String,

    pub date_created: i32,

    pub download_dir: String,

    pub download_limit: i32,

    pub download_limited: bool,

    pub eta: i64,

    pub eta_idle: i64,

    pub hash_string: String,

    pub have_unchecked: i64,

    pub have_valid: i64,

    pub is_finished: bool,

    pub is_private: bool,

    pub is_stalled: bool,

    pub name: String,

    pub percent_done: f32,

    pub queue_position: i32,

    pub start_date: i32,

    pub status: i32,

    pub torrent_file: String,

    pub total_size: i64,
}

/// Torrent peers information.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct Peers {
    pub id: i32,

    pub peer_limit: i32,

    pub peers_connected: i32,

    pub peers_getting_from_us: i32,

    pub peers_sending_to_us: i32,

    pub max_connected_peers: i32,

    pub webseeds_sending_to_us: i32,
}

#[cfg(test)]
mod tests {
    #[test]
    fn create_torrent() -> Result<(), super::BitTorrentError> {
        std::fs::create_dir_all("target/test_data/create_torrent").unwrap();
        std::fs::write(
            "target/test_data/create_torrent/file.txt",
            "This is a test file.",
        )
        .unwrap();
        super::create_torrent_file(
            "target/test_data/create_torrent",
            "target/test_data/create_torrent/test.torrent",
            None,
        )?;
        assert!(std::path::Path::new("target/test_data/create_torrent/test.torrent").exists());
        std::fs::remove_dir_all("target/test_data/create_torrent").unwrap();
        Ok(())
    }
}
