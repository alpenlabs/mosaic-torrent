//! Tests for the TransmissionClient.

use mosaic_torrent_types::{BitTorrent, BitTorrentError};
use transmission_client::ClientError;

use super::{map_client_error, TransmissionClient};
use crate::ops::MockTransmissionOps;
use crate::testutil::{make_test_peers, make_test_stats, make_test_torrent};

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
