//! Internal trait abstracting Transmission RPC operations.
//!
//! This module provides the [`TransmissionOps`] trait which abstracts the underlying
//! transmission client, enabling mocking in tests.

use transmission_client::{
    Client, ClientError, SessionStats as TransmissionSessionStats, Torrent as TransmissionTorrent,
    TorrentPeers,
};

/// Internal trait that abstracts the transmission client operations.
/// This allows for mocking in tests.
#[cfg_attr(test, mockall::automock)]
#[allow(async_fn_in_trait)]
pub(crate) trait TransmissionOps {
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
