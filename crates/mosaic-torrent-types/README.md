# Mosaic Torrent Types

This crate defines common types and traits for BitTorrent clients used in the Mosaic project.
The `BitTorrent` trait defines the following operations:

```rust
pub trait BitTorrent {
    /// Add a torrent file to Transmission. The torrents starts downloading/seeding immediately.
    /// This can be used to download a torrent, and also to seed a torrent.
    async fn add(&self, torrent_file: &str) -> Result<Torrent, BitTorrentError>;
    /// Stop torrents by their IDs. The IDs should be the torrent hash.
    async fn stop(&self, ids: Vec<String>) -> Result<(), BitTorrentError>;
    /// List all torrents.
    async fn list(&self) -> Result<Vec<Torrent>, BitTorrentError>;
    /// Get the list of peers for a specific torrent by its ID (i32).
    async fn peers(&self, id: i32) -> Result<Peers, BitTorrentError>;
    /// Remove torrents by their IDs (torrent hash). If `delete_local_data` is true, the local data will also be deleted.
    async fn remove(
        &self,
        ids: Vec<String>,
        delete_local_data: bool,
    ) -> Result<(), BitTorrentError>;
    /// Get session statistics.
    async fn stats(&self) -> Result<SessionStats, BitTorrentError>;
}
```

A default implementation is given in the `mosaic-torrent-controller` crate
for the `Transmission` BitTorrent client. Other implementations should be easy to add if required.
