//! Type conversion wrappers between transmission_client types and mosaic_torrent_types.
//!
//! These newtype wrappers exist to satisfy the orphan rule since both the source
//! and target types are defined outside this crate.

use mosaic_torrent_types::{Peers, SessionStats, StatsDetails, Torrent};
use transmission_client::{
    SessionStats as TransmissionSessionStats, StatsDetails as TransmissionStatsDetails,
    Torrent as TransmissionTorrent, TorrentPeers,
};

/// Wrapper for converting `TransmissionSessionStats` to `SessionStats`.
#[derive(Debug)]
pub(crate) struct TransmissionSessionStatsWrapper(pub(crate) TransmissionSessionStats);

/// Wrapper for converting `TransmissionStatsDetails` to `StatsDetails`.
#[derive(Debug)]
pub(crate) struct TransmissionStatsDetailsWrapper(pub(crate) TransmissionStatsDetails);

/// Wrapper for converting `TransmissionTorrent` to `Torrent`.
#[derive(Debug)]
pub(crate) struct TransmissionTorrentWrapper(pub(crate) TransmissionTorrent);

/// Wrapper for converting `TorrentPeers` to `Peers`.
#[derive(Debug)]
pub(crate) struct TransmissionTorrentPeersWrapper(pub(crate) TorrentPeers);

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
    use crate::testutil::{make_test_peers, make_test_stats, make_test_torrent};

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
