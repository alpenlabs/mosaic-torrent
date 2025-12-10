//! # Mosaic Torrent Types
//!
//! This crate defines common types and traits for BitTorrent clients used in the Mosaic project.

use bip_metainfo::{MetainfoBuilder, PieceLength};

/// Create a torrent file from a folder.
/// This is not BitTorrent client specific, so it is not part of the BitTorrent trait.
pub fn create_torrent_file(folder: &str, output_file: &str, tracker_url: Option<&str>) {
    let builder = MetainfoBuilder::new()
        .set_piece_length(PieceLength::OptBalanced)
        .set_main_tracker(tracker_url);
    let bytes = builder.build(1, folder, |_| {}).unwrap();
    std::fs::write(output_file, bytes).unwrap();
}

#[allow(async_fn_in_trait)]
/// BitTorrent trait defines the common interface for BitTorrent clients.
pub trait BitTorrent {
    /// Add a torrent file to Transmission. The torrents starts downloading/seeding immediately.
    /// This can be used to download a torrent, and also to seed a torrent.
    async fn add(&self, torrent_file: &str, download_dir: &str) -> Torrent;
    /// Stop torrents by their IDs. The IDs should be the torrent hash.
    async fn stop(&self, ids: Vec<String>);
    /// List all torrents.
    async fn list(&self) -> Vec<Torrent>;
    /// Remove torrents by their IDs (torrent hash). If `delete_local_data` is true, the local data will also be deleted.
    async fn remove(&self, ids: Vec<String>, delete_local_data: bool);
    /// Get session statistics.
    async fn stats(&self) -> SessionStats;
}

// The below are copied from Transmission RPC types, as this will be the initial implementation.
// Other implementations are expected to be similar.

#[derive(Debug)]
#[allow(missing_docs)] // rationale: these are the same fields as in Transmission RPC
/// Session statistics.
pub struct SessionStats {
    pub active_torrent_count: i32,
    pub cumulative_stats: StatsDetails,
    pub current_stats: StatsDetails,
    pub download_speed: i32,
    pub paused_torrent_count: i32,
    pub torrent_count: i32,
    pub upload_speed: i32,
}

#[derive(Debug)]
#[allow(missing_docs)]
/// Detailed statistics.
pub struct StatsDetails {
    pub downloaded_bytes: i64,
    pub files_added: i64,
    pub seconds_active: i64,
    pub session_count: i64,
    pub uploaded_bytes: i64,
}

#[derive(Debug)]
#[allow(missing_docs)]
/// Torrent information.
pub struct Torrent {
    pub id: i32,
    pub activity_date: i32,
    pub added_date: i32,
    pub bandwidth_priority: i32,
    pub comment: String,
    pub corrupt_ever: i64,
    pub creator: String,
    pub date_created: i32,
    pub desired_available: i64,
    pub done_date: i32,
    pub download_dir: String,
    pub download_limit: i32,
    pub download_limited: bool,
    pub downloaded_ever: i64,
    pub edit_date: i32,
    pub error: i32,
    pub error_string: String,
    pub eta: i64,
    pub eta_idle: i64,
    pub hash_string: String,
    pub have_unchecked: i64,
    pub have_valid: i64,
    pub honors_session_limits: bool,
    pub is_finished: bool,
    pub is_private: bool,
    pub is_stalled: bool,
    pub left_until_done: i64,
    pub magnet_link: String,
    pub manual_announce_time: i32,
    pub metadata_percent_complete: f32,
    pub name: String,
    pub percent_done: f32,
    pub piece_count: i64,
    pub piece_size: i64,
    pub pieces: String,
    pub primary_mime_type: String,
    pub queue_position: i32,
    pub rate_download: i32,
    pub rate_upload: i32,
    pub recheck_progress: f32,
    pub seconds_downloading: i32,
    pub seconds_seeding: i32,
    pub seed_idle_limit: i32,
    pub seed_idle_mode: i32,
    pub seed_ratio_limit: f32,
    pub seed_ratio_mode: i32,
    pub size_when_done: i64,
    pub start_date: i32,
    pub status: i32,
    pub torrent_file: String,
    pub total_size: i64,
    pub upload_limit: i32,
    pub upload_limited: bool,
    pub upload_ratio: f32,
    pub uploaded_ever: i64,
}

#[cfg(test)]
mod tests {
    #[test]
    fn create_torrent() {
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
        );
        assert!(std::path::Path::new("target/test_data/create_torrent/test.torrent").exists());
        std::fs::remove_dir_all("target/test_data/create_torrent").unwrap();
    }
}
