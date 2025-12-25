//! Transmission RPC client implementation.

use tracing::debug;
use transmission_client::{Client, ClientError, SessionMutator};
use url::Url;

use mosaic_torrent_types::{BitTorrent, BitTorrentError, Peers, SessionStats, Torrent};

use crate::conversions::{
    TransmissionSessionStatsWrapper, TransmissionTorrentPeersWrapper, TransmissionTorrentWrapper,
};
use crate::ops::TransmissionOps;

#[cfg(test)]
mod tests;

/// TransmissionClient is a BitTorrent client that uses Transmission RPC.
#[allow(missing_debug_implementations, private_bounds)]
pub struct TransmissionClient<T: TransmissionOps = Client> {
    client: T,
}

impl TransmissionClient {
    /// Create a new TransmissionClient.
    ///
    /// This method is async as the session settings are applied on creation.
    pub async fn try_new(rpc_url: &str, max_downloads: u32) -> Result<Self, BitTorrentError> {
        let url = Url::parse(rpc_url)
            .map_err(|e| BitTorrentError::Other(format!("Invalid RPC URL: {}", e)))?;

        debug!("Connecting to Transmission RPC at {}", url);
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

        debug!("Connected to Transmission Daemon");
        Ok(Self { client })
    }
}

#[allow(private_bounds)]
impl<T: TransmissionOps> TransmissionClient<T> {
    /// Create a TransmissionClient with a custom client implementation.
    /// This is primarily useful for testing with mocks.
    #[cfg(test)]
    pub(crate) fn with_client(client: T) -> Self {
        Self { client }
    }
}

#[allow(private_bounds)]
impl<T: TransmissionOps> BitTorrent for TransmissionClient<T> {
    async fn add(&self, torrent_file: &str) -> Result<Torrent, BitTorrentError> {
        debug!("Adding torrent from file: {}", torrent_file);
        let torrent = self
            .client
            .torrent_add_filename(torrent_file)
            .await
            .map_err(map_client_error)?
            .ok_or_else(|| BitTorrentError::InvalidTorrent("No torrent returned".into()))?;

        debug!("Added {torrent:?}");
        Ok(TransmissionTorrentWrapper(torrent).into())
    }

    async fn stop(&self, ids: Vec<String>) -> Result<(), BitTorrentError> {
        debug!("Stopping torrents {ids:?}");
        self.client
            .torrent_stop(Some(ids))
            .await
            .map_err(map_client_error)?;
        debug!("Stop command sent");
        Ok(())
    }

    async fn list(&self) -> Result<Vec<Torrent>, BitTorrentError> {
        debug!("Listing active torrents");
        let torrents = self
            .client
            .torrents(None)
            .await
            .map_err(map_client_error)?
            .into_iter()
            .map(|t| TransmissionTorrentWrapper(t).into())
            .collect();
        debug!("Active torrents: {torrents:?}");

        Ok(torrents)
    }

    async fn peers(&self, id: i32) -> Result<Peers, BitTorrentError> {
        debug!("Getting peers for torrent ID {id}");
        let peers_vec = self
            .client
            .torrents_peers(Some(vec![id]))
            .await
            .map_err(map_client_error)?;
        let peers = peers_vec.first().ok_or_else(|| {
            BitTorrentError::InvalidTorrent(format!("No peers found for torrent ID {}", id))
        })?;
        debug!("Peers for torrent ID {id}: {peers:?}");

        Ok(TransmissionTorrentPeersWrapper(peers.clone()).into())
    }

    async fn remove(
        &self,
        ids: Vec<String>,
        delete_local_data: bool,
    ) -> Result<(), BitTorrentError> {
        debug!("Removing torrents {ids:?}, delete_local_data={delete_local_data}");
        self.client
            .torrent_remove(Some(ids), delete_local_data)
            .await
            .map_err(map_client_error)?;
        debug!("Remove command sent");
        Ok(())
    }

    async fn stats(&self) -> Result<SessionStats, BitTorrentError> {
        debug!("Getting session statistics");
        let stats = self
            .client
            .session_stats()
            .await
            .map_err(map_client_error)?;
        debug!("Session statistics: {stats:?}");

        Ok(TransmissionSessionStatsWrapper(stats).into())
    }
}

/// Maps transmission client errors to BitTorrent errors.
fn map_client_error(err: ClientError) -> BitTorrentError {
    match err {
        ClientError::TransmissionUnauthorized => BitTorrentError::Unauthorized,
        ClientError::TransmissionError(msg) => BitTorrentError::ServerError(msg),
        ClientError::NetworkError(e) => BitTorrentError::Network(e.to_string()),
        ClientError::SerdeError(e) => BitTorrentError::Other(e.to_string()),
    }
}
