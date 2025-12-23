//! Integration test for TransmissionClient with a chained sequence: add -> list -> peers -> stop -> remove.
//! Requires a running Transmission RPC and environment configuration:
//! - TRANSMISSION_RPC_URL (default: http://localhost:9091/transmission/rpc)
//! - TRANSMISSION_INCOMPLETE_DIR (default: /tmp/transmission/incomplete)
//! - TRANSMISSION_MAX_DOWNLOADS (default: 1)
//!
//! This test creates a local "documentation.txt" with "Lorem ipsum..." and builds a .torrent for it.

#![allow(unused_crate_dependencies)]

use std::{env, fs, path::PathBuf};

use mosaic_torrent_controller::TransmissionClient;
use mosaic_torrent_types::{BitTorrent, create_torrent_file};

fn rpc_url() -> String {
    env::var("TRANSMISSION_RPC_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9091/transmission/rpc".into())
}

fn max_downloads() -> u32 {
    env::var("TRANSMISSION_MAX_DOWNLOADS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1)
}

/// Prepare a temporary test workspace with a sample file and a generated torrent.
/// Returns (folder_path, torrent_file_path, download_dir_path).
fn prepare_test_workspace() -> (PathBuf, PathBuf) {
    let base = PathBuf::from("/testdata");
    let folder = base.join("sample");
    let torrent_file = base.join("sample.torrent");

    // Create directories
    fs::create_dir_all(&folder).unwrap();

    // Write sample documentation.txt
    fs::write(
        folder.join("documentation.txt"),
        "Lorem ipsum dolor sit amet...",
    )
    .unwrap();

    // Build a .torrent for the folder
    create_torrent_file(
        folder.to_string_lossy().as_ref(),
        torrent_file.to_string_lossy().as_ref(),
        None
    )
    .expect("failed to create sample torrent");

    (folder, torrent_file)
}

#[tokio::test]
#[test_log::test]
async fn transmission_controller_chained_flow() {
    // Arrange client
    let client = TransmissionClient::try_new(Some(rpc_url().as_str()), max_downloads())
        .await
        .expect("failed to initialize TransmissionClient");

    // Arrange test data
    let (_folder, torrent_file) = prepare_test_workspace();

    // 1. Add torrent
    let added = client
        .add(torrent_file.to_string_lossy().as_ref())
        .await
        .expect("failed to add torrent");

    // 2. List torrents, find our torrent by hash or name
    let list = client.list().await.expect("failed to list torrents");
    let t = list
        .iter()
        .find(|t| t.id == added.id || t.hash_string == added.hash_string)
        .expect("added torrent not found in list");

    // 3. Peers for our torrent id (must exist)
    let _peers = client.peers(t.id).await.expect("failed to fetch peers");

    // 4. Stop our torrent by hash
    client
        .stop(vec![added.hash_string.clone()])
        .await
        .expect("failed to stop torrent");

    // 5. Remove our torrent by hash (no local data deletion)
    client
        .remove(vec![added.hash_string.clone()], false)
        .await
        .expect("failed to remove torrent");

    // 6. Ensure our torrent is gone
    let final_list = client.list().await.expect("failed to list torrents");
    let still_present = final_list
        .iter()
        .any(|t| t.id == added.id || t.hash_string == added.hash_string);
    assert!(!still_present, "torrent was not removed");
}
