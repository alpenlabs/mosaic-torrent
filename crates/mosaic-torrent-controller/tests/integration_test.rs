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

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn integration_like_test() -> std::io::Result<()> {
    let tmp = tempfile::tempdir()?;
    let pidfile = tmp.path().join("transmission.pid");

    let guard = ForkingDaemonGuard::start_transmission(pidfile, &[])?;

    guard.wait_tcp_ready("127.0.0.1", 9091, std::time::Duration::from_secs(5))?;

    println!("Transmission daemon started with PID {}", guard.pid);
    let client = TransmissionClient::try_new(None, None, 2).await.unwrap();
    client
        .add("assets/test_folder.torrent", tmp.path().to_str().unwrap())
        .await
        .unwrap();
    let torrents = client.list().await.unwrap();
    assert_eq!(torrents.len(), 1);
    let hash = torrents.first().unwrap().hash_string.clone();
    client.stop(vec![hash.clone()]).await.unwrap();
    client.remove(vec![hash.clone()], true).await.unwrap();
    let torrents = client.list().await.unwrap();
    assert_eq!(torrents.len(), 0);

    Ok(())
}
