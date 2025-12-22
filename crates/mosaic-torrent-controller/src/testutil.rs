//! Shared test utilities and fixtures.

use transmission_client::{
    SessionStats as TransmissionSessionStats, StatsDetails as TransmissionStatsDetails,
    Torrent as TransmissionTorrent, TorrentPeers,
};

pub(crate) fn make_test_torrent(id: i32, name: &str, hash: &str) -> TransmissionTorrent {
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

pub(crate) fn make_test_peers(id: i32) -> TorrentPeers {
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

pub(crate) fn make_test_stats() -> TransmissionSessionStats {
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
