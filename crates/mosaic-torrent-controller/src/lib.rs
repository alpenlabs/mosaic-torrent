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
//!     let client = TransmissionClient::try_new(None, Some("/path/to/incomplete/dir"), 1).await?;
//!     let torrent = client.add("path/to/output/file.torrent", "/path/to/download/dir").await?;
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

/// TransmissionClient is a BitTorrent client that uses Transmission RPC.
#[allow(missing_debug_implementations)]
pub struct TransmissionClient {
    client: Client,
}

impl TransmissionClient {
    /// Create a new TransmissionClient.
    /// If no RPC URL is provided, it defaults to "http://localhost:9091/transmission/rpc".
    /// This method is async as the session settings are applied on creation.
    /// An incomplete directory can also be specified. If None is provided, it defaults to "/tmp/transmission/incomplete".
    pub async fn try_new(
        rpc_url: Option<&str>,
        incomplete_dir: Option<&str>,
        max_downloads: u32,
    ) -> Result<Self, BitTorrentError> {
        let url = Url::parse(rpc_url.unwrap_or("http://localhost:9091/transmission/rpc"))
            .map_err(|e| BitTorrentError::Other(format!("Invalid RPC URL: {}", e)))?;

        let client = Client::new(url);
        let incomplete_dir = incomplete_dir.unwrap_or("/tmp/transmission/incomplete");
        let session_mutator = SessionMutator {
            incomplete_dir_enabled: Some(true),
            incomplete_dir: Some(incomplete_dir.into()),
            download_queue_enabled: Some(true),
            download_queue_size: Some(max_downloads as i32),
            ..Default::default()
        };

        client
            .session_set(session_mutator)
            .await
            .map_err(map_client_error)?;

        Ok(Self { client })
    }
}

impl BitTorrent for TransmissionClient {
    async fn add(
        &self,
        torrent_file: &str,
        download_dir: &str,
    ) -> Result<Torrent, BitTorrentError> {
        let torrent = self
            .client
            .torrent_add_filename_download_dir(torrent_file, download_dir)
            .await
            .map_err(map_client_error)?
            .ok_or_else(|| BitTorrentError::InvalidTorrent("No torrent returned".into()))?;

        Ok(TransmissionTorrentWrapper(torrent).into())
    }

    async fn stop(&self, ids: Vec<String>) -> Result<(), BitTorrentError> {
        self.client
            .torrent_stop(Some(ids))
            .await
            .map_err(map_client_error)?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<Torrent>, BitTorrentError> {
        let torrents = self
            .client
            .torrents(None)
            .await
            .map_err(map_client_error)?
            .into_iter()
            .map(|t| TransmissionTorrentWrapper(t).into())
            .collect();

        Ok(torrents)
    }

    async fn peers(&self, id: i32) -> Result<Peers, BitTorrentError> {
        let peers_vec = self
            .client
            .torrents_peers(Some(vec![id]))
            .await
            .map_err(map_client_error)?;
        let peers = peers_vec.first().ok_or_else(|| {
            BitTorrentError::InvalidTorrent(format!("No peers found for torrent ID {}", id))
        })?;

        Ok(TransmissionTorrentPeersWrapper(peers.clone()).into())
    }

    async fn remove(
        &self,
        ids: Vec<String>,
        delete_local_data: bool,
    ) -> Result<(), BitTorrentError> {
        self.client
            .torrent_remove(Some(ids), delete_local_data)
            .await
            .map_err(map_client_error)?;
        Ok(())
    }

    async fn stats(&self) -> Result<SessionStats, BitTorrentError> {
        let stats = self
            .client
            .session_stats()
            .await
            .map_err(map_client_error)?;

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
    #[test]
    fn foo() {}
}
