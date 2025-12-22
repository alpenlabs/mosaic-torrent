# Torrent Controller

This crate provides a `TransmissionClient` that implements the `mosaic_torrent_types::BitTorrent` trait
from `mosaic_torrent_types`, allowing you to manage torrents through the Transmission daemon. It assumes
the `transmission-daemon` is already running, and reachable via RPC. Via the first argument to
`TransmissionClient::try_new()` you can specify the RPC URL.

## Usage

### Seeding a torrent

```rust,ignore
use mosaic_torrent_controller::TransmissionClient;
use mosaic_torrent_types::{BitTorrent, create_torrent_file};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    create_torrent_file(
        "path/to/folder",
        "path/to/output/file.torrent",
        None,
    )?;
    let client = TransmissionClient::try_new(None, 1).await?;
    let torrent = client.add("path/to/output/file.torrent").await?;
    println!("Added torrent: {:?}", torrent);
    Ok(())
}
```

### Downloading a torrent

```rust,ignore
use mosaic_torrent_controller::TransmissionClient;
use mosaic_torrent_types::BitTorrent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = TransmissionClient::try_new(None, 1).await?;
    let torrent = client.add("path/to/file.torrent").await?;
    println!("Added torrent: {:?}", torrent);
    Ok(())
}
```
