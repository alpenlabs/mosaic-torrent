#![allow(unused_crate_dependencies)]
#![allow(missing_docs)]

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use mosaic_torrent_controller::TransmissionClient;
use mosaic_torrent_types::BitTorrent;

struct ForkingDaemonGuard {
    pidfile: PathBuf,
    pid: i32,
}

impl ForkingDaemonGuard {
    fn start_transmission(pidfile: PathBuf, extra_args: &[&str]) -> io::Result<Self> {
        let mut args: Vec<&str> = Vec::with_capacity(2 + extra_args.len());
        args.push("-x");
        let pidfile_str = pidfile
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "pidfile path not utf-8"))?
            .to_owned();

        let mut cmd = Command::new("transmission-daemon");
        cmd.arg("-x")
            .arg(&pidfile_str)
            .args(extra_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        cmd.spawn()?;

        wait_for_file(&pidfile, Duration::from_secs(3))?;
        let pid = read_pid(&pidfile)?;

        Ok(Self { pidfile, pid })
    }

    fn wait_tcp_ready(&self, host: &str, port: u16, timeout: Duration) -> io::Result<()> {
        use std::net::{TcpStream, ToSocketAddrs};

        let addr = (host, port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no address"))?;

        let start = Instant::now();
        while start.elapsed() < timeout {
            if TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "daemon did not become ready in time",
        ))
    }
}

impl Drop for ForkingDaemonGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            unsafe {
                libc::kill(self.pid as libc::pid_t, libc::SIGTERM);
            }

            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                let alive = unsafe { libc::kill(self.pid as libc::pid_t, 0) } == 0;
                if !alive {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }

            unsafe {
                libc::kill(self.pid as libc::pid_t, libc::SIGKILL);
            }
        }

        let _ = fs::remove_file(&self.pidfile);
    }
}

fn wait_for_file(path: &Path, timeout: Duration) -> io::Result<()> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "pidfile not created",
    ))
}

fn read_pid(path: &Path) -> io::Result<i32> {
    let s = fs::read_to_string(path)?;
    s.trim()
        .parse::<i32>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn init_test_tracing() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter("debug")
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    });
}

/// Happy path integration test: start a Transmission daemon, add a torrent,
#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn integration_test() -> std::io::Result<()> {
    use tokio::time::sleep;
    use tracing::debug;

    init_test_tracing();

    let tmp = tempfile::tempdir()?;
    let pidfile = tmp.path().join("transmission.pid");
    let download_dir = tmp.path().join("complete");
    let incomplete_dir = tmp.path().join("complete");

    let guard = ForkingDaemonGuard::start_transmission(
        pidfile,
        &[
            "-w",
            download_dir.to_str().unwrap(),
            "--incomplete-dir",
            incomplete_dir.to_str().unwrap(),
        ],
    )?;

    guard.wait_tcp_ready("127.0.0.1", 9091, std::time::Duration::from_secs(5))?;

    debug!("Transmission daemon started with PID {}", guard.pid);
    let client = TransmissionClient::try_new(None, 2).await.unwrap();
    let _ = client.add("assets/test_folder.torrent").await.unwrap();
    let torrents = client.list().await.unwrap();
    assert_eq!(torrents.len(), 1);
    let hash = torrents.first().unwrap().hash_string.clone();
    loop {
        sleep(Duration::from_secs(5)).await;
        let binding = client.list().await.unwrap();
        let torrent = binding.first().unwrap();
        let _peers = client.peers(torrent.id).await.unwrap();
        if torrent.percent_done >= 1.0 {
            break;
        }
    }
    client.stop(vec![hash.clone()]).await.unwrap();
    client.remove(vec![hash.clone()], true).await.unwrap();
    let torrents = client.list().await.unwrap();
    assert_eq!(torrents.len(), 0);

    Ok(())
}

/// Test that connecting to a non-existent daemon fails with a network error.
#[tokio::test(flavor = "current_thread")]
async fn integration_test_connection_refused() {
    init_test_tracing();

    // Try to connect to a port where no daemon is running
    let result =
        TransmissionClient::try_new(Some("http://127.0.0.1:19999/transmission/rpc"), 2).await;

    match result {
        Err(mosaic_torrent_types::BitTorrentError::Network(msg)) => {
            assert!(
                msg.contains("Connection refused") || msg.contains("error sending request"),
                "Expected connection refused error, got: {}",
                msg
            );
        }
        Err(other) => panic!("Expected Network error, got: {:?}", other),
        Ok(_) => panic!("Expected connection to fail"),
    }
}

/// Test that an invalid RPC URL is rejected.
#[tokio::test(flavor = "current_thread")]
async fn integration_test_invalid_rpc_url() {
    init_test_tracing();

    let result = TransmissionClient::try_new(Some("not-a-valid-url"), 2).await;

    match result {
        Err(mosaic_torrent_types::BitTorrentError::Other(msg)) => {
            assert!(
                msg.contains("Invalid RPC URL"),
                "Expected invalid URL error, got: {}",
                msg
            );
        }
        Err(other) => panic!("Expected Other error for invalid URL, got: {:?}", other),
        Ok(_) => panic!("Expected invalid URL to be rejected"),
    }
}

/// Test adding a non-existent torrent file fails.
#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn integration_test_add_nonexistent_torrent() -> std::io::Result<()> {
    init_test_tracing();

    let tmp = tempfile::tempdir()?;
    let pidfile = tmp.path().join("transmission.pid");
    let download_dir = tmp.path().join("complete");
    let port = 9092;

    let guard = ForkingDaemonGuard::start_transmission(
        pidfile,
        &[
            "-w",
            download_dir.to_str().unwrap(),
            "-p",
            &port.to_string(),
        ],
    )?;

    guard.wait_tcp_ready("127.0.0.1", port, std::time::Duration::from_secs(5))?;

    let rpc_url = format!("http://127.0.0.1:{}/transmission/rpc", port);
    let client = TransmissionClient::try_new(Some(&rpc_url), 2)
        .await
        .unwrap();

    // Try to add a torrent file that doesn't exist
    let result = client.add("/nonexistent/path/to/fake.torrent").await;

    assert!(result.is_err());
    // The error should indicate invalid torrent or server error
    match result.unwrap_err() {
        mosaic_torrent_types::BitTorrentError::InvalidTorrent(_)
        | mosaic_torrent_types::BitTorrentError::ServerError(_) => {}
        other => panic!("Expected InvalidTorrent or ServerError, got: {:?}", other),
    }

    Ok(())
}

/// Test adding an invalid/corrupt torrent file fails.
#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn integration_test_add_invalid_torrent_content() -> std::io::Result<()> {
    init_test_tracing();

    let tmp = tempfile::tempdir()?;
    let pidfile = tmp.path().join("transmission.pid");
    let download_dir = tmp.path().join("complete");
    let port = 9093;

    // Create an invalid torrent file with garbage content
    let invalid_torrent_path = tmp.path().join("invalid.torrent");
    fs::write(&invalid_torrent_path, "this is not valid bencode data")?;

    let guard = ForkingDaemonGuard::start_transmission(
        pidfile,
        &[
            "-w",
            download_dir.to_str().unwrap(),
            "-p",
            &port.to_string(),
        ],
    )?;

    guard.wait_tcp_ready("127.0.0.1", port, std::time::Duration::from_secs(5))?;

    let rpc_url = format!("http://127.0.0.1:{}/transmission/rpc", port);
    let client = TransmissionClient::try_new(Some(&rpc_url), 2)
        .await
        .unwrap();

    let result = client.add(invalid_torrent_path.to_str().unwrap()).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        mosaic_torrent_types::BitTorrentError::InvalidTorrent(_)
        | mosaic_torrent_types::BitTorrentError::ServerError(_) => {}
        other => panic!(
            "Expected InvalidTorrent or ServerError for corrupt file, got: {:?}",
            other
        ),
    }

    Ok(())
}

/// Test getting peers for a non-existent torrent ID fails.
#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn integration_test_peers_nonexistent_torrent() -> std::io::Result<()> {
    init_test_tracing();

    let tmp = tempfile::tempdir()?;
    let pidfile = tmp.path().join("transmission.pid");
    let download_dir = tmp.path().join("complete");
    let port = 9094;

    let guard = ForkingDaemonGuard::start_transmission(
        pidfile,
        &[
            "-w",
            download_dir.to_str().unwrap(),
            "-p",
            &port.to_string(),
        ],
    )?;

    guard.wait_tcp_ready("127.0.0.1", port, std::time::Duration::from_secs(5))?;

    let rpc_url = format!("http://127.0.0.1:{}/transmission/rpc", port);
    let client = TransmissionClient::try_new(Some(&rpc_url), 2)
        .await
        .unwrap();

    // Try to get peers for a torrent ID that doesn't exist
    let result = client.peers(999999).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        mosaic_torrent_types::BitTorrentError::InvalidTorrent(msg) => {
            assert!(
                msg.contains("No peers found"),
                "Expected 'No peers found' message, got: {}",
                msg
            );
        }
        other => panic!("Expected InvalidTorrent error, got: {:?}", other),
    }

    Ok(())
}

/// Test that stopping a non-existent torrent hash doesn't cause an error
/// (Transmission silently ignores unknown hashes).
#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn integration_test_stop_nonexistent_torrent() -> std::io::Result<()> {
    init_test_tracing();

    let tmp = tempfile::tempdir()?;
    let pidfile = tmp.path().join("transmission.pid");
    let download_dir = tmp.path().join("complete");
    let port = 9095;

    let guard = ForkingDaemonGuard::start_transmission(
        pidfile,
        &[
            "-w",
            download_dir.to_str().unwrap(),
            "-p",
            &port.to_string(),
        ],
    )?;

    guard.wait_tcp_ready("127.0.0.1", port, std::time::Duration::from_secs(5))?;

    let rpc_url = format!("http://127.0.0.1:{}/transmission/rpc", port);
    let client = TransmissionClient::try_new(Some(&rpc_url), 2)
        .await
        .unwrap();

    // Stopping a non-existent hash should succeed (Transmission ignores unknown IDs)
    let result = client.stop(vec!["nonexistenthash123".to_string()]).await;

    // Transmission RPC doesn't error on unknown hashes for stop
    assert!(result.is_ok());

    Ok(())
}

/// Test that removing a non-existent torrent hash doesn't cause an error
/// (Transmission silently ignores unknown hashes).
#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn integration_test_remove_nonexistent_torrent() -> std::io::Result<()> {
    init_test_tracing();

    let tmp = tempfile::tempdir()?;
    let pidfile = tmp.path().join("transmission.pid");
    let download_dir = tmp.path().join("complete");
    let port = 9096;

    let guard = ForkingDaemonGuard::start_transmission(
        pidfile,
        &[
            "-w",
            download_dir.to_str().unwrap(),
            "-p",
            &port.to_string(),
        ],
    )?;

    guard.wait_tcp_ready("127.0.0.1", port, std::time::Duration::from_secs(5))?;

    let rpc_url = format!("http://127.0.0.1:{}/transmission/rpc", port);
    let client = TransmissionClient::try_new(Some(&rpc_url), 2)
        .await
        .unwrap();

    // Removing a non-existent hash should succeed (Transmission ignores unknown IDs)
    let result = client
        .remove(vec!["nonexistenthash456".to_string()], false)
        .await;

    assert!(result.is_ok());

    Ok(())
}

/// Test listing torrents when none exist returns empty list.
#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn integration_test_list_empty() -> std::io::Result<()> {
    init_test_tracing();

    let tmp = tempfile::tempdir()?;
    let pidfile = tmp.path().join("transmission.pid");
    let download_dir = tmp.path().join("complete");
    let port = 9097;

    let guard = ForkingDaemonGuard::start_transmission(
        pidfile,
        &[
            "-w",
            download_dir.to_str().unwrap(),
            "-p",
            &port.to_string(),
        ],
    )?;

    guard.wait_tcp_ready("127.0.0.1", port, std::time::Duration::from_secs(5))?;

    let rpc_url = format!("http://127.0.0.1:{}/transmission/rpc", port);
    let client = TransmissionClient::try_new(Some(&rpc_url), 2)
        .await
        .unwrap();

    let torrents = client.list().await.unwrap();
    assert!(torrents.is_empty(), "Expected empty torrent list");

    Ok(())
}
