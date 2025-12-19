// Allow unused dev-dependencies in lib test target
#![cfg_attr(test, allow(unused_crate_dependencies))]

//! # Torrent controller using Transmission RPC.
//!
//! usage:
//!
//! ```rust,ignore
//! use mosaic_torrent_controller::TransmissionClient;
//! use mosaic_torrent_types::{BitTorrent, create_torrent_file};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     create_torrent_file(
//!         "path/to/folder",
//!         "path/to/output/file.torrent",
//!         None,
//!     )?;
//!     let client = TransmissionClient::try_new(None, 1).await?;
//!     let torrent = client.add("path/to/output/file.torrent").await?;
//!     println!("Added torrent: {:?}", torrent);
//!     Ok(())
//! }
//! ```
//!

use transmission_client::{
    Client, ClientError, SessionMutator, SessionStats as TransmissionSessionStats,
    StatsDetails as TransmissionStatsDetails, Torrent as TransmissionTorrent, TorrentPeers,
};
use url::Url;

use mosaic_torrent_types::{
    BitTorrent, BitTorrentError, Peers, SessionStats, StatsDetails, Torrent,
};

/// Internal trait that abstracts the transmission client operations.
/// This allows for mocking in tests.
#[cfg_attr(test, mockall::automock)]
#[allow(async_fn_in_trait, dead_code)]
trait TransmissionOps {
    async fn session_set(&self, mutator: SessionMutator) -> Result<(), ClientError>;
    async fn torrent_add_filename(
        &self,
        filename: &str,
    ) -> Result<Option<TransmissionTorrent>, ClientError>;
    async fn torrent_stop(&self, ids: Option<Vec<String>>) -> Result<(), ClientError>;
    async fn torrents(
        &self,
        ids: Option<Vec<i32>>,
    ) -> Result<Vec<TransmissionTorrent>, ClientError>;
    async fn torrents_peers(&self, ids: Option<Vec<i32>>)
    -> Result<Vec<TorrentPeers>, ClientError>;
    async fn torrent_remove(
        &self,
        ids: Option<Vec<String>>,
        delete_local_data: bool,
    ) -> Result<(), ClientError>;
    async fn session_stats(&self) -> Result<TransmissionSessionStats, ClientError>;
}

impl TransmissionOps for Client {
    async fn session_set(&self, mutator: SessionMutator) -> Result<(), ClientError> {
        Client::session_set(self, mutator).await
    }

    async fn torrent_add_filename(
        &self,
        filename: &str,
    ) -> Result<Option<TransmissionTorrent>, ClientError> {
        Client::torrent_add_filename(self, filename).await
    }

    async fn torrent_stop(&self, ids: Option<Vec<String>>) -> Result<(), ClientError> {
        Client::torrent_stop(self, ids).await
    }

    async fn torrents(
        &self,
        ids: Option<Vec<i32>>,
    ) -> Result<Vec<TransmissionTorrent>, ClientError> {
        Client::torrents(self, ids).await
    }

    async fn torrents_peers(
        &self,
        ids: Option<Vec<i32>>,
    ) -> Result<Vec<TorrentPeers>, ClientError> {
        Client::torrents_peers(self, ids).await
    }

    async fn torrent_remove(
        &self,
        ids: Option<Vec<String>>,
        delete_local_data: bool,
    ) -> Result<(), ClientError> {
        Client::torrent_remove(self, ids, delete_local_data).await
    }

    async fn session_stats(&self) -> Result<TransmissionSessionStats, ClientError> {
        Client::session_stats(self).await
    }
}

/// TransmissionClient is a BitTorrent client that uses Transmission RPC.
#[allow(missing_debug_implementations, private_bounds)]
pub struct TransmissionClient<T: TransmissionOps = Client> {
    client: T,
}

impl TransmissionClient {
    /// Create a new TransmissionClient.
    /// If no RPC URL is provided, it defaults to "http://localhost:9091/transmission/rpc".
    /// This method is async as the session settings are applied on creation.
    /// An incomplete directory can also be specified. If None is provided, it defaults to "/tmp/transmission/incomplete".
    pub async fn try_new(
        rpc_url: Option<&str>,
        max_downloads: u32,
    ) -> Result<Self, BitTorrentError> {
        let url = Url::parse(rpc_url.unwrap_or("http://localhost:9091/transmission/rpc"))
            .map_err(|e| BitTorrentError::Other(format!("Invalid RPC URL: {}", e)))?;

        log::debug!("Connecting to Transmission RPC at {}", url);
        let client = Client::new(url);
        let session_mutator = SessionMutator {
            incomplete_dir_enabled: Some(true),
            download_queue_enabled: Some(true),
            download_queue_size: Some(max_downloads as i32),
            ..Default::default()
        };

        client
            .session_set(session_mutator)
            .await
            .map_err(map_client_error)?;

        log::debug!("Connected to Transmission Daemon");
        Ok(Self { client })
    }
}

#[allow(private_bounds)]
impl<T: TransmissionOps> TransmissionClient<T> {
    /// Create a TransmissionClient with a custom client implementation.
    /// This is primarily useful for testing with mocks.
    #[cfg(test)]
    fn with_client(client: T) -> Self {
        Self { client }
    }
}

#[allow(private_bounds)]
impl<T: TransmissionOps> BitTorrent for TransmissionClient<T> {
    async fn add(&self, torrent_file: &str) -> Result<Torrent, BitTorrentError> {
        log::debug!("Adding torrent from file: {}", torrent_file);
        let torrent = self
            .client
            .torrent_add_filename(torrent_file)
            .await
            .map_err(map_client_error)?
            .ok_or_else(|| BitTorrentError::InvalidTorrent("No torrent returned".into()))?;

        log::debug!("Added {torrent:?}");
        Ok(TransmissionTorrentWrapper(torrent).into())
    }

    async fn stop(&self, ids: Vec<String>) -> Result<(), BitTorrentError> {
        log::debug!("Stopping torrents {ids:?}");
        self.client
            .torrent_stop(Some(ids))
            .await
            .map_err(map_client_error)?;
        log::debug!("Stop command sent");
        Ok(())
    }

    async fn list(&self) -> Result<Vec<Torrent>, BitTorrentError> {
        log::debug!("Listing active torrents");
        let torrents = self
            .client
            .torrents(None)
            .await
            .map_err(map_client_error)?
            .into_iter()
            .map(|t| TransmissionTorrentWrapper(t).into())
            .collect();
        log::debug!("Active torrents: {torrents:?}");

        Ok(torrents)
    }

    async fn peers(&self, id: i32) -> Result<Peers, BitTorrentError> {
        log::debug!("Getting peers for torrent ID {id}");
        let peers_vec = self
            .client
            .torrents_peers(Some(vec![id]))
            .await
            .map_err(map_client_error)?;
        let peers = peers_vec.first().ok_or_else(|| {
            BitTorrentError::InvalidTorrent(format!("No peers found for torrent ID {}", id))
        })?;
        log::debug!("Peers for torrent ID {id}: {peers:?}");

        Ok(TransmissionTorrentPeersWrapper(peers.clone()).into())
    }

    async fn remove(
        &self,
        ids: Vec<String>,
        delete_local_data: bool,
    ) -> Result<(), BitTorrentError> {
        log::debug!("Removing torrents {ids:?}, delete_local_data={delete_local_data}");
        self.client
            .torrent_remove(Some(ids), delete_local_data)
            .await
            .map_err(map_client_error)?;
        log::debug!("Remove command sent");
        Ok(())
    }

    async fn stats(&self) -> Result<SessionStats, BitTorrentError> {
        log::debug!("Getting session statistics");
        let stats = self
            .client
            .session_stats()
            .await
            .map_err(map_client_error)?;
        log::debug!("Session statistics: {stats:?}");

        Ok(TransmissionSessionStatsWrapper(stats).into())
    }
}

// Error conversion helper
fn map_client_error(err: ClientError) -> BitTorrentError {
    match err {
        ClientError::TransmissionUnauthorized => BitTorrentError::Unauthorized,
        ClientError::TransmissionError(msg) => BitTorrentError::ServerError(msg),
        ClientError::NetworkError(e) => BitTorrentError::Network(e.to_string()),
        ClientError::SerdeError(e) => BitTorrentError::Other(e.to_string()),
    }
}

// conversions from transmission_client types to mosaic_torrent_types types

// Newtype wrappers to satisfy the orphan rule since both types are defined outside this crate
#[allow(missing_docs)]
#[derive(Debug)]
pub struct TransmissionSessionStatsWrapper(TransmissionSessionStats);
#[allow(missing_docs)]
#[derive(Debug)]
pub struct TransmissionStatsDetailsWrapper(TransmissionStatsDetails);
#[allow(missing_docs)]
#[derive(Debug)]
pub struct TransmissionTorrentWrapper(TransmissionTorrent);
#[allow(missing_docs)]
#[derive(Debug)]
pub struct TransmissionTorrentPeersWrapper(TorrentPeers);

impl From<TransmissionSessionStatsWrapper> for SessionStats {
    fn from(wrapper: TransmissionSessionStatsWrapper) -> Self {
        let value = wrapper.0;
        Self {
            active_torrent_count: value.active_torrent_count,
            cumulative_stats: TransmissionStatsDetailsWrapper(value.cumulative_stats).into(),
            current_stats: TransmissionStatsDetailsWrapper(value.current_stats).into(),
            download_speed: value.download_speed,
            paused_torrent_count: value.paused_torrent_count,
            torrent_count: value.torrent_count,
            upload_speed: value.upload_speed,
        }
    }
}

impl From<TransmissionStatsDetailsWrapper> for StatsDetails {
    fn from(wrapper: TransmissionStatsDetailsWrapper) -> Self {
        let value = wrapper.0;
        Self {
            downloaded_bytes: value.downloaded_bytes,
            files_added: value.files_added,
            seconds_active: value.seconds_active,
            session_count: value.session_count,
            uploaded_bytes: value.uploaded_bytes,
        }
    }
}

impl From<TransmissionTorrentWrapper> for Torrent {
    fn from(wrapper: TransmissionTorrentWrapper) -> Self {
        let value = wrapper.0;
        Self {
            id: value.id,
            activity_date: value.activity_date,
            added_date: value.added_date,
            bandwidth_priority: value.bandwidth_priority,
            comment: value.comment,
            creator: value.creator,
            date_created: value.date_created,
            download_dir: value.download_dir,
            download_limit: value.download_limit,
            download_limited: value.download_limited,
            eta: value.eta,
            eta_idle: value.eta_idle,
            hash_string: value.hash_string,
            have_unchecked: value.have_unchecked,
            have_valid: value.have_valid,
            is_finished: value.is_finished,
            is_private: value.is_private,
            is_stalled: value.is_stalled,
            name: value.name,
            percent_done: value.percent_done,
            queue_position: value.queue_position,
            start_date: value.start_date,
            status: value.status,
            torrent_file: value.torrent_file,
            total_size: value.total_size,
        }
    }
}

impl From<TransmissionTorrentPeersWrapper> for Peers {
    fn from(wrapper: TransmissionTorrentPeersWrapper) -> Self {
        let value = wrapper.0;
        Self {
            id: value.id,
            peer_limit: value.peer_limit,
            peers_connected: value.peers_connected,
            peers_getting_from_us: value.peers_getting_from_us,
            peers_sending_to_us: value.peers_sending_to_us,
            max_connected_peers: value.max_connected_peers,
            webseeds_sending_to_us: value.webseeds_sending_to_us,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_torrent(id: i32, name: &str, hash: &str) -> TransmissionTorrent {
        TransmissionTorrent {
            id,
            activity_date: 0,
            added_date: 0,
            bandwidth_priority: 0,
            comment: String::new(),
            corrupt_ever: 0,
            creator: String::new(),
            date_created: 0,
            desired_available: 0,
            done_date: 0,
            download_dir: "/downloads".to_string(),
            download_limit: 0,
            download_limited: false,
            downloaded_ever: 0,
            edit_date: 0,
            error: 0,
            error_string: String::new(),
            eta: 0,
            eta_idle: 0,
            hash_string: hash.to_string(),
            have_unchecked: 0,
            have_valid: 0,
            honors_session_limits: true,
            is_finished: false,
            is_private: false,
            is_stalled: false,
            left_until_done: 0,
            magnet_link: String::new(),
            manual_announce_time: 0,
            metadata_percent_complete: 1.0,
            name: name.to_string(),
            percent_done: 0.5,
            piece_count: 100,
            piece_size: 1024,
            pieces: String::new(),
            primary_mime_type: String::new(),
            queue_position: 0,
            rate_download: 0,
            rate_upload: 0,
            recheck_progress: 0.0,
            seconds_downloading: 0,
            seconds_seeding: 0,
            seed_idle_limit: 0,
            seed_idle_mode: 0,
            seed_ratio_limit: 0.0,
            seed_ratio_mode: 0,
            size_when_done: 1000,
            start_date: 0,
            status: 4,
            torrent_file: "/path/to/torrent".to_string(),
            total_size: 1000,
            upload_limit: 0,
            upload_limited: false,
            upload_ratio: 0.0,
            uploaded_ever: 0,
        }
    }

    fn make_test_peers(id: i32) -> TorrentPeers {
        TorrentPeers {
            id,
            peer_limit: 100,
            peers_connected: 5,
            peers_getting_from_us: 2,
            peers_sending_to_us: 3,
            max_connected_peers: 50,
            webseeds_sending_to_us: 0,
        }
    }

    fn make_test_stats() -> TransmissionSessionStats {
        TransmissionSessionStats {
            active_torrent_count: 1,
            cumulative_stats: TransmissionStatsDetails {
                downloaded_bytes: 1000,
                files_added: 5,
                seconds_active: 3600,
                session_count: 10,
                uploaded_bytes: 500,
            },
            current_stats: TransmissionStatsDetails {
                downloaded_bytes: 100,
                files_added: 1,
                seconds_active: 600,
                session_count: 1,
                uploaded_bytes: 50,
            },
            download_speed: 1000,
            paused_torrent_count: 0,
            torrent_count: 1,
            upload_speed: 500,
        }
    }

    #[tokio::test]
    async fn test_add_torrent_success() {
        let mut mock = MockTransmissionOps::new();
        let test_torrent = make_test_torrent(1, "test_torrent", "abc123");

        mock.expect_torrent_add_filename()
            .withf(|filename| filename == "/path/to/file.torrent")
            .returning(move |_| Ok(Some(make_test_torrent(1, "test_torrent", "abc123"))));

        let client = TransmissionClient::with_client(mock);
        let result = client.add("/path/to/file.torrent").await;

        assert!(result.is_ok());
        let torrent = result.unwrap();
        assert_eq!(torrent.id, test_torrent.id);
        assert_eq!(torrent.name, test_torrent.name);
        assert_eq!(torrent.hash_string, test_torrent.hash_string);
    }

    #[tokio::test]
    async fn test_add_torrent_returns_none() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrent_add_filename().returning(|_| Ok(None));

        let client = TransmissionClient::with_client(mock);
        let result = client.add("/path/to/file.torrent").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BitTorrentError::InvalidTorrent(msg) => {
                assert!(msg.contains("No torrent returned"));
            }
            _ => panic!("Expected InvalidTorrent error"),
        }
    }

    #[tokio::test]
    async fn test_add_torrent_unauthorized() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrent_add_filename()
            .returning(|_| Err(ClientError::TransmissionUnauthorized));

        let client = TransmissionClient::with_client(mock);
        let result = client.add("/path/to/file.torrent").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BitTorrentError::Unauthorized => {}
            _ => panic!("Expected Unauthorized error"),
        }
    }

    #[tokio::test]
    async fn test_add_torrent_server_error() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrent_add_filename()
            .returning(|_| Err(ClientError::TransmissionError("Server error".to_string())));

        let client = TransmissionClient::with_client(mock);
        let result = client.add("/path/to/file.torrent").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BitTorrentError::ServerError(msg) => {
                assert_eq!(msg, "Server error");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[tokio::test]
    async fn test_stop_torrent_success() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrent_stop()
            .withf(|ids| ids == &Some(vec!["abc123".to_string()]))
            .returning(|_| Ok(()));

        let client = TransmissionClient::with_client(mock);
        let result = client.stop(vec!["abc123".to_string()]).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stop_torrent_error() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrent_stop()
            .returning(|_| Err(ClientError::TransmissionError("Failed to stop".to_string())));

        let client = TransmissionClient::with_client(mock);
        let result = client.stop(vec!["abc123".to_string()]).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BitTorrentError::ServerError(msg) => {
                assert_eq!(msg, "Failed to stop");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[tokio::test]
    async fn test_list_torrents_success() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrents()
            .withf(|ids| ids.is_none())
            .returning(|_| {
                Ok(vec![
                    make_test_torrent(1, "torrent1", "hash1"),
                    make_test_torrent(2, "torrent2", "hash2"),
                ])
            });

        let client = TransmissionClient::with_client(mock);
        let result = client.list().await;

        assert!(result.is_ok());
        let torrents = result.unwrap();
        assert_eq!(torrents.len(), 2);
        assert_eq!(torrents[0].id, 1);
        assert_eq!(torrents[0].name, "torrent1");
        assert_eq!(torrents[1].id, 2);
        assert_eq!(torrents[1].name, "torrent2");
    }

    #[tokio::test]
    async fn test_list_torrents_empty() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrents().returning(|_| Ok(vec![]));

        let client = TransmissionClient::with_client(mock);
        let result = client.list().await;

        assert!(result.is_ok());
        let torrents = result.unwrap();
        assert!(torrents.is_empty());
    }

    #[tokio::test]
    async fn test_list_torrents_error() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrents()
            .returning(|_| Err(ClientError::TransmissionUnauthorized));

        let client = TransmissionClient::with_client(mock);
        let result = client.list().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BitTorrentError::Unauthorized => {}
            _ => panic!("Expected Unauthorized error"),
        }
    }

    #[tokio::test]
    async fn test_peers_success() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrents_peers()
            .withf(|ids| ids == &Some(vec![1]))
            .returning(|_| Ok(vec![make_test_peers(1)]));

        let client = TransmissionClient::with_client(mock);
        let result = client.peers(1).await;

        assert!(result.is_ok());
        let peers = result.unwrap();
        assert_eq!(peers.id, 1);
        assert_eq!(peers.peers_connected, 5);
        assert_eq!(peers.peers_getting_from_us, 2);
        assert_eq!(peers.peers_sending_to_us, 3);
    }

    #[tokio::test]
    async fn test_peers_not_found() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrents_peers().returning(|_| Ok(vec![]));

        let client = TransmissionClient::with_client(mock);
        let result = client.peers(999).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BitTorrentError::InvalidTorrent(msg) => {
                assert!(msg.contains("No peers found for torrent ID 999"));
            }
            _ => panic!("Expected InvalidTorrent error"),
        }
    }

    #[tokio::test]
    async fn test_peers_error() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrents_peers()
            .returning(|_| Err(ClientError::TransmissionError("Peers error".to_string())));

        let client = TransmissionClient::with_client(mock);
        let result = client.peers(1).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BitTorrentError::ServerError(msg) => {
                assert_eq!(msg, "Peers error");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[tokio::test]
    async fn test_remove_torrent_success() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrent_remove()
            .withf(|ids, delete_data| ids == &Some(vec!["hash1".to_string()]) && *delete_data)
            .returning(|_, _| Ok(()));

        let client = TransmissionClient::with_client(mock);
        let result = client.remove(vec!["hash1".to_string()], true).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_remove_torrent_without_delete() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrent_remove()
            .withf(|ids, delete_data| ids == &Some(vec!["hash1".to_string()]) && !*delete_data)
            .returning(|_, _| Ok(()));

        let client = TransmissionClient::with_client(mock);
        let result = client.remove(vec!["hash1".to_string()], false).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_remove_torrent_error() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_torrent_remove()
            .returning(|_, _| Err(ClientError::TransmissionError("Remove failed".to_string())));

        let client = TransmissionClient::with_client(mock);
        let result = client.remove(vec!["hash1".to_string()], true).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BitTorrentError::ServerError(msg) => {
                assert_eq!(msg, "Remove failed");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[tokio::test]
    async fn test_stats_success() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_session_stats()
            .returning(|| Ok(make_test_stats()));

        let client = TransmissionClient::with_client(mock);
        let result = client.stats().await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.active_torrent_count, 1);
        assert_eq!(stats.download_speed, 1000);
        assert_eq!(stats.upload_speed, 500);
        assert_eq!(stats.torrent_count, 1);
        assert_eq!(stats.cumulative_stats.downloaded_bytes, 1000);
        assert_eq!(stats.current_stats.downloaded_bytes, 100);
    }

    #[tokio::test]
    async fn test_stats_error() {
        let mut mock = MockTransmissionOps::new();

        mock.expect_session_stats()
            .returning(|| Err(ClientError::TransmissionUnauthorized));

        let client = TransmissionClient::with_client(mock);
        let result = client.stats().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BitTorrentError::Unauthorized => {}
            _ => panic!("Expected Unauthorized error"),
        }
    }

    #[test]
    fn test_error_mapping_unauthorized() {
        let err = map_client_error(ClientError::TransmissionUnauthorized);
        assert!(matches!(err, BitTorrentError::Unauthorized));
    }

    #[test]
    fn test_error_mapping_server_error() {
        let err = map_client_error(ClientError::TransmissionError("test error".to_string()));
        match err {
            BitTorrentError::ServerError(msg) => assert_eq!(msg, "test error"),
            _ => panic!("Expected ServerError"),
        }
    }

    #[test]
    fn test_torrent_conversion() {
        let transmission_torrent = make_test_torrent(42, "My Torrent", "deadbeef");
        let torrent: Torrent = TransmissionTorrentWrapper(transmission_torrent).into();

        assert_eq!(torrent.id, 42);
        assert_eq!(torrent.name, "My Torrent");
        assert_eq!(torrent.hash_string, "deadbeef");
        assert_eq!(torrent.percent_done, 0.5);
        assert_eq!(torrent.download_dir, "/downloads");
    }

    #[test]
    fn test_peers_conversion() {
        let transmission_peers = make_test_peers(10);
        let peers: Peers = TransmissionTorrentPeersWrapper(transmission_peers).into();

        assert_eq!(peers.id, 10);
        assert_eq!(peers.peer_limit, 100);
        assert_eq!(peers.peers_connected, 5);
        assert_eq!(peers.peers_getting_from_us, 2);
        assert_eq!(peers.peers_sending_to_us, 3);
    }

    #[test]
    fn test_stats_conversion() {
        let transmission_stats = make_test_stats();
        let stats: SessionStats = TransmissionSessionStatsWrapper(transmission_stats).into();

        assert_eq!(stats.active_torrent_count, 1);
        assert_eq!(stats.download_speed, 1000);
        assert_eq!(stats.upload_speed, 500);
        assert_eq!(stats.cumulative_stats.downloaded_bytes, 1000);
        assert_eq!(stats.cumulative_stats.session_count, 10);
        assert_eq!(stats.current_stats.downloaded_bytes, 100);
    }
}
